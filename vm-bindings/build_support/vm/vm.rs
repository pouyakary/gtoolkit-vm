#[cfg(all(feature = "jit", target_os = "ios"))]
compile_error!("JIT is not supported by iOS");

use std::rc::Rc;

use anyhow::{anyhow, Result};
use semver::Version;
use serde::Serialize;

use crate::*;

#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct VirtualMachine {
    #[serde(skip)]
    builder: Rc<dyn Builder>,
    #[serde(skip)]
    vmmaker: VMMaker,
    build_info: BuildInfo,
    config: ConfigTemplate,
    core: Core,
    plugins: Vec<Plugin>,
}

impl VirtualMachine {
    pub(crate) fn builder() -> Result<Rc<dyn Builder>> {
        for_target_triplet(std::env::var("TARGET").unwrap().as_str())
    }

    fn build_info(builder: Rc<dyn Builder>) -> Result<BuildInfo> {
        BuildInfo::new(builder)
    }

    fn size_of_int(_os: &TargetOS, arch: &ArchBits) -> usize {
        match arch {
            ArchBits::Bit32 => 4,
            ArchBits::Bit64 => 4,
        }
    }

    fn size_of_long(os: &TargetOS, arch: &ArchBits) -> usize {
        match arch {
            ArchBits::Bit32 => 4,
            ArchBits::Bit64 => {
                match os.family() {
                    FamilyOS::Unix | FamilyOS::Other => 8,
                    FamilyOS::Apple => 8,
                    // An int and a long are 32-bit values on 64-bit Windows operating systems.
                    // https://learn.microsoft.com/en-us/cpp/build/common-visual-cpp-64-bit-migration-issues?redirectedfrom=MSDN&view=msvc-170
                    FamilyOS::Windows => 4,
                }
            }
        }
    }

    fn size_of_long_long(_os: &TargetOS, arch: &ArchBits) -> usize {
        match arch {
            ArchBits::Bit32 => 8,
            ArchBits::Bit64 => 8,
        }
    }

    fn size_of_void_pointer(_os: &TargetOS, arch: &ArchBits) -> usize {
        match arch {
            ArchBits::Bit32 => 4,
            ArchBits::Bit64 => 8,
        }
    }

    /// Sets up the core configuration of the vm such as its name, the size of basic types, version and the build timestamp
    fn config(builder: Rc<dyn Builder>, info: &BuildInfo) -> Result<ConfigTemplate> {
        let mut config = ConfigTemplate::new(builder.clone());
        let arch_bits = builder.arch_bits();
        let target_os = builder.target();

        let size_of_int = Self::size_of_int(&target_os, &arch_bits);
        let size_of_long = Self::size_of_long(&target_os, &arch_bits);
        let size_of_long_long = Self::size_of_long_long(&target_os, &arch_bits);
        let size_of_void_p = Self::size_of_void_pointer(&target_os, &arch_bits);
        let squeak_int64_type = if size_of_long == 8 {
            "long"
        } else {
            if size_of_long_long == 8 {
                "long long"
            } else {
                return Err(anyhow!("Could not find a 64bit integer type"));
            }
        };

        let os_type = match builder.target_family() {
            FamilyOS::Apple => "Mac OS",
            FamilyOS::Unix | FamilyOS::Other => {
                if builder.target().is_android() {
                    "android"
                } else {
                    "unix"
                }
            }
            FamilyOS::Windows => "Win32",
        };

        let target_os = builder.target().os().as_str();

        config
            .var(Config::VM_NAME("Pharo".to_string()))
            .var(Config::DEFAULT_IMAGE_NAME("Pharo.image".to_string()))
            .var(Config::OS_TYPE(os_type.to_string()))
            .var(Config::VM_TARGET(std::env::var("CARGO_CFG_TARGET_OS")?))
            .var(Config::VM_TARGET_OS(target_os.to_string()))
            .var(Config::VM_TARGET_CPU(std::env::var(
                "CARGO_CFG_TARGET_ARCH",
            )?))
            .var(Config::SIZEOF_INT(size_of_int))
            .var(Config::SIZEOF_LONG(size_of_long))
            .var(Config::SIZEOF_LONG_LONG(size_of_long_long))
            .var(Config::SIZEOF_VOID_P(size_of_void_p))
            .var(Config::SQUEAK_INT64_TYPEDEF(squeak_int64_type.to_string()))
            .var(Config::VERSION_MAJOR(info.version_major()))
            .var(Config::VERSION_MINOR(info.version_minor()))
            .var(Config::VERSION_PATCH(info.version_patch()))
            .var(Config::PharoVM_VERSION_STRING(
                info.version()
                    .map(|version| format!("{}", version))
                    .unwrap_or_else(|| "Unknown version".to_string()),
            ))
            .var(Config::BUILT_FROM(info.to_string()))
            .var(Config::ALWAYS_INTERACTIVE(false));
        Ok(config)
    }

    /// Return the name of the interpreter that should be generated.
    /// This basically boils down to choosing between faster JIT-enabled or
    /// slower stack interpreter
    fn interpreter(_builder: Rc<dyn Builder>) -> &'static str {
        if cfg!(feature = "jit") {
            "CoInterpreterWithProcessSwitchTelemetry"
        } else {
            "StackVM"
        }
    }

    fn cogit_compiler(_builder: Rc<dyn Builder>) -> &'static str {
        "StackToRegisterMappingCogitWithProcessSwitchTelemetry"
    }

    fn interpreter_sources() -> Vec<&'static str> {
        if cfg!(feature = "jit") {
            vec![
                // generated interpreter sources
                "{generated}/vm/src/cogit.c",
                #[cfg(not(feature = "gnuisation"))]
                "{generated}/vm/src/cointerp.c",
                #[cfg(feature = "gnuisation")]
                "{generated}/vm/src/gcc3x-cointerp.c",
            ]
        } else {
            vec!["{generated}/vm/src/interp.c"]
        }
    }

    fn sources(target: &TargetOS, build_info: &BuildInfo) -> Vec<&'static str> {
        let mut sources = Self::interpreter_sources();
        sources.extend([
            // support sources
            "{crate}/extra/debug.c",
            "{sources}/src/utils.c",
            "{sources}/src/errorCode.c",
            "{sources}/src/nullDisplay.c",
            "{sources}/src/externalPrimitives.c",
            //"{sources}/src/client.c",
            "{crate}/extra/client.c",
            "{sources}/src/pathUtilities.c",
            "{sources}/src/parameters/parameterVector.c",
            "{sources}/src/parameters/parameters.c",
            "{sources}/src/fileDialogCommon.c",
            "{sources}/src/stringUtilities.c",
            "{sources}/src/imageAccess.c",
            "{sources}/src/semaphores/platformSemaphore.c",
            "{sources}/extracted/vm/src/common/heartbeat.c",
            // Common sources
            "{sources}/extracted/vm/src/common/sqHeapMap.c",
            "{sources}/extracted/vm/src/common/sqVirtualMachine.c",
            "{sources}/extracted/vm/src/common/sqNamedPrims.c",
            "{sources}/extracted/vm/src/common/sqExternalSemaphores.c",
            "{sources}/extracted/vm/src/common/sqTicker.c",
            // Re-exports of private VM functions
            "{crate}/extra/sqExport.c",
            "{crate}/extra/exported.c",
        ]);

        match target.family() {
            FamilyOS::Apple => {
                sources.extend([
                    // Platform sources
                    "{sources}/extracted/vm/src/osx/aioOSX.c",
                    "{sources}/src/debugUnix.c",
                    "{sources}/src/utilsMac.mm",
                    "{sources}/src/parameters/parameters.m",
                    // Virtual Memory functions
                    "{sources}/src/memoryUnix.c",
                ])
            }
            FamilyOS::Unix | FamilyOS::Other => {
                sources.extend([
                    // Platform sources
                    "{sources}/extracted/vm/src/unix/aio.c",
                    "{sources}/src/debugUnix.c",
                    // Support sources
                    "{sources}/src/fileDialogUnix.c",
                    // Virtual Memory functions
                    "{sources}/src/memoryUnix.c",
                ])
            }
            FamilyOS::Windows => {
                sources.extend([
                    // Platform sources
                    "{sources}/extracted/vm/src/win/sqWin32SpurAlloc.c",
                    "{sources}/extracted/vm/src/win/aioWin.c",
                    // Support sources
                    "{sources}/src/fileDialogWin32.c",
                    "{crate}/extra/getpagesizeWin32.c",
                ]);

                let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
                let setjmp = match target_arch.as_str() {
                    "x86_64" => "{crate}/extra/setjmp-Windows-wrapper-X64.asm",
                    "aarch64" => "{crate}/extra/setjmp-Windows-wrapper-ARM64.asm",
                    _ => panic!("Unsupported arch: {}", &target_arch),
                };

                sources.extend([setjmp]);

                // Choose debugging sources based on the VM Version. Starting with v9.0.8
                // the sources moved
                let version = build_info.version().unwrap();
                if version < &Version::new(9, 0, 8) {
                    sources.extend(["{sources}/src/debugWin.c"]);
                } else {
                    sources.extend([
                        "{sources}/src/win/winDebug.c",
                        "{sources}/src/win/winDebugMenu.c",
                        "{sources}/src/win/winDebugWindow.c",
                    ]);
                };
            }
        }

        sources
    }

    /// Return a list of include directories for a given build target platform
    fn includes(target: &TargetOS) -> Vec<String> {
        let mut includes = [
            "{crate}/extra".to_owned(),
            "{crate}/extra/include".to_owned(),
            "{crate}/extra/include/pharovm".to_owned(),
            "{sources}/extracted/vm/include/common".to_owned(),
            "{sources}/include".to_owned(),
            "{sources}/include/pharovm".to_owned(),
            "{generated}/vm/include".to_owned(),
        ]
        .to_vec();

        match target.family() {
            FamilyOS::Apple => {
                includes.push("{sources}/extracted/vm/include/osx".to_owned());
                includes.push("{sources}/extracted/vm/include/unix".to_owned());
            }
            FamilyOS::Unix | FamilyOS::Other => {
                includes.push("{sources}/extracted/vm/include/unix".to_owned());
            }
            FamilyOS::Windows => {
                includes.push("{crate}/extra/extracted/vm/include/win".to_owned());
                includes.push("{sources}/extracted/vm/include/win".to_owned());
                includes.push(format!(
                    "{{ output }}/{}/{}/include",
                    WindowsBuilder::pthreads_name(),
                    WindowsBuilder::vcpkg_triplet()
                ));
            }
        }
        includes
    }

    fn core(builder: Rc<dyn Builder>, build_info: &BuildInfo) -> Core {
        let mut core = Core::new("PharoVMCore", builder.clone());
        core.sources(Self::sources(&core.target(), build_info));
        core.includes(Self::includes(&core.target()));

        core.define_for_header("dirent.h", "HAVE_DIRENT_H");
        core.define_for_header("ndir.h", "HAVE_NDIR_H");
        core.define_for_header("sys/ndir.h", "HAVE_SYS_NDIR_H");
        core.define_for_header("sys/dir.h", "HAVE_SYS_DIR_H");
        core.define_for_header("features.h", "HAVE_FEATURES_H");
        core.define_for_header("unistd.h", "HAVE_UNISTD_H");
        core.define_for_header("sys/filio.h", "HAVE_SYS_FILIO_H");
        core.define_for_header("sys/time.h", "HAVE_SYS_TIME_H");

        // android does not support execinfo.h
        if !core.target().is_android() {
            core.define_for_header("execinfo.h", "HAVE_EXECINFO_H");
        }
        core.define_for_header("dlfcn.h", "HAVE_DLFCN_H");

        core.flag("-Wno-error=implicit-function-declaration");
        core.flag("-Wno-implicit-function-declaration");
        core.flag("-Wno-absolute-value");
        core.flag("-Wno-shift-count-overflow");
        core.flag("-Wno-int-conversion");
        core.flag("-Wno-macro-redefined");
        core.flag("-Wno-unused-value");
        core.flag("-Wno-pointer-to-int-cast");
        core.flag("-Wno-non-literal-null-conversion");
        core.flag("-Wno-conditional-type-mismatch");
        core.flag("-Wno-compare-distinct-pointer-types");
        core.flag("-Wno-incompatible-function-pointer-types");
        core.flag("-Wno-pointer-sign");
        core.flag("-Wno-unused-command-line-argument");
        core.flag("-Wno-undef-prefix");

        #[cfg(feature = "immutability")]
        core.define("IMMUTABILITY", "1");
        #[cfg(feature = "inline_memory_accessors")]
        core.define("USE_INLINE_MEMORY_ACCESSORS", "1");

        #[cfg(feature = "jit")]
        {
            core.define("COGVM", "1");
            core.define("COGMTVM", "0");
        }

        core.define("PharoVM", "1");
        core.define("ASYNC_FFI_QUEUE", "1");

        match core.arch_bits() {
            ArchBits::Bit32 => {
                core.define("ARCH", "32");
            }
            ArchBits::Bit64 => {
                core.define("ARCH", "64");
            }
        }
        core.define("VM_LABEL(foo)", "0");
        core.define("SOURCE_PATH_SIZE", "40");

        core.define("PHARO_VM_IN_WORKER_THREAD", "1");

        // let's never build pharo-vm in debug mode because it enables assertions
        // and the whole thing becomes too slow
        core.define("NDEBUG", None);
        core.define("DEBUGVM", "0");

        if core.target().is_unix() {
            core.define("LSB_FIRST", "1");
            core.define("UNIX", "1");
            core.define("HAVE_TM_GMTOFF", None);

            // Android has its own pthread implementation
            if !core.target().is_android() {
                core.dependency(Dependency::Library("pthread".to_string(), vec![]));
            }
            if core.target().is_android() {
                core.dependency(Dependency::Library("m".to_string(), vec![]));
            }
        }

        if core.target().is_apple() {
            core.define("OSX", "1");
            if core.target().is_macos() {
                core.dependency(Dependency::SystemLibrary("AppKit".to_string()));
                // On Apple Silicon machines the code zone is read-only, and requires special operations.
                // Enabling READ_ONLY_CODE_ZONE makes the VM use pthread_jit_write_protect_np()
                // which is only available on MacOS. We should not enable it on other apple device.
                #[cfg(target_arch = "aarch64")]
                core.define("READ_ONLY_CODE_ZONE", "1");
            }
            core.dependency(Dependency::SystemLibrary("Foundation".to_string()));
        }

        if core.target().is_windows() {
            core.define("WIN", "1");
            core.define("WIN32", "1");
            core.dependency(Dependency::SystemLibrary("User32".to_string()));
            core.dependency(Dependency::SystemLibrary("Ws2_32".to_string()));
            core.dependency(Dependency::SystemLibrary("DbgHelp".to_string()));
            core.dependency(Dependency::SystemLibrary("Ole32".to_string()));
            core.dependency(Dependency::SystemLibrary("Shell32".to_string()));
            core.dependency(Dependency::Library(
                WindowsBuilder::pthreads_lib_name().to_string(),
                vec![WindowsBuilder::pthreads_lib()],
            ));
        }

        #[cfg(feature = "ffi")]
        core.add_feature(ffi_feature(&core));
        #[cfg(feature = "threaded_ffi")]
        core.add_feature(threaded_ffi_feature(&core));
        core
    }

    fn plugins(core: &Core) -> Vec<Plugin> {
        [
            #[cfg(feature = "b2d_plugin")]
            b2d_plugin(&core),
            #[cfg(feature = "bit_blt_plugin")]
            bit_blt_plugin(&core),
            #[cfg(feature = "dsa_primitives_plugin")]
            dsa_primitives_plugin(&core),
            #[cfg(feature = "file_plugin")]
            file_plugin(&core),
            #[cfg(feature = "file_attributes_plugin")]
            file_attributes_plugin(&core),
            #[cfg(feature = "float_array_plugin")]
            float_array_plugin(&core),
            #[cfg(feature = "jpeg_read_writer2_plugin")]
            jpeg_read_writer2_plugin(&core),
            #[cfg(feature = "jpeg_reader_plugin")]
            jpeg_reader_plugin(&core),
            #[cfg(feature = "large_integers_plugin")]
            large_integers_plugin(&core),
            #[cfg(feature = "locale_plugin")]
            locale_plugin(&core),
            #[cfg(feature = "misc_primitive_plugin")]
            misc_primitive_plugin(&core),
            #[cfg(feature = "socket_plugin")]
            socket_plugin(&core),
            #[cfg(feature = "squeak_ssl_plugin")]
            squeak_ssl_plugin(&core),
            #[cfg(feature = "surface_plugin")]
            surface_plugin(&core),
            #[cfg(all(feature = "unix_os_process_plugin", target_family = "unix"))]
            unix_os_process_plugin(&core),
            #[cfg(feature = "uuid_plugin")]
            uuid_plugin(&core),
        ]
        .to_vec()
        .into_iter()
        .filter_map(|each| each)
        .collect()
    }

    fn vmmaker(builder: Rc<dyn Builder>, interpreter: &str, compiler: &str) -> Result<VMMaker> {
        let vmmaker = VMMaker::prepare(builder)?;
        vmmaker.generate_sources(interpreter, compiler)?;
        Ok(vmmaker)
    }

    pub fn new() -> Result<Self> {
        let builder = Self::builder()?;
        builder.prepare_environment();
        let build_info = Self::build_info(builder.clone())?;
        let config = Self::config(builder.clone(), &build_info)?;
        let vmmaker = Self::vmmaker(
            builder.clone(),
            Self::interpreter(builder.clone()),
            Self::cogit_compiler(builder.clone()),
        )?;
        let core = Self::core(builder.clone(), &build_info);
        let plugins = Self::plugins(&core);

        Ok(Self {
            builder,
            vmmaker,
            build_info,
            config,
            core,
            plugins,
        })
    }

    pub fn get_core(&self) -> &Core {
        &self.core
    }

    pub fn compile(&self) {
        self.config.render();
        self.core.compile();
        for plugin in &self.plugins {
            plugin.compile();
        }
        self.builder.link_libraries();
        self.builder.generate_bindings();
    }
}
