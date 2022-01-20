use crate::{Builder, BuilderTarget};
use std::fmt;
use std::fmt::{Debug, Formatter};
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Default, Clone)]
pub struct LinuxBuilder;

impl Debug for LinuxBuilder {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.print_directories(f)
    }
}

impl Builder for LinuxBuilder {
    fn target(&self) -> BuilderTarget {
        BuilderTarget::Linux
    }

    fn compile_sources(&self) {}

    fn platform_include_directory(&self) -> PathBuf {
        self.squeak_include_directory().join("unix")
    }

    fn link_libraries(&self) {
        println!("cargo:rustc-link-lib=PharoVMCore");
    }

    fn boxed(self) -> Rc<dyn Builder> {
        Rc::new(self)
    }
}
