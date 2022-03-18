use std::path::PathBuf;
use std::process::Command;

fn executable() -> PathBuf {
    let executable = PathBuf::from(env!("VM_CLIENT_EXECUTABLE"));
    if !executable.exists() {
        panic!("{} does not exist", &executable.display());
    }
    executable
}

fn default_image() -> PathBuf {
    let image = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("images")
        .join("Pharo9.0-64bit")
        .join("Pharo9.0-64bit.image");

    if !image.exists() {
        panic!("{} does not exist", &image.display());
    }
    image
}

fn minimal_image() -> PathBuf {
    let image = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("images")
        .join("Pharo9.0-64bit-minimal")
        .join("Pharo9.0-64bit-minimal.image");

    if !image.exists() {
        panic!("{} does not exist", &image.display());
    }
    image
}

#[test]
pub fn minimal_add() {
    let executable = executable();
    let output = Command::new(&executable)
        .current_dir(executable.parent().unwrap())
        .arg(minimal_image())
        .arg("eval")
        .arg("2+2")
        .output()
        .unwrap();

    // extract the raw bytes that we captured and interpret them as a string
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_eq!(stdout.trim(), "4");
}

#[test]
pub fn minimal_worker_add() {
    let executable = executable();
    let output = Command::new(&executable)
        .current_dir(executable.parent().unwrap())
        .arg("--worker")
        .arg(minimal_image())
        .arg("eval")
        .arg("2+2")
        .output()
        .unwrap();

    // extract the raw bytes that we captured and interpret them as a string
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert_eq!(stdout.trim(), "4");
    assert_eq!(stderr.trim(), "");
}

#[test]
pub fn minimal_worker_is_in_worker() {
    let executable = executable();
    let output = Command::new(&executable)
        .current_dir(executable.parent().unwrap())
        .arg("--worker")
        .arg(minimal_image())
        .arg("eval")
        .arg("Smalltalk vm isRunningInWorkerThread")
        .output()
        .unwrap();

    // extract the raw bytes that we captured and interpret them as a string
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert_eq!(stdout.trim(), "true");
    assert_eq!(stderr.trim(), "");
}

#[test]
pub fn default_add() {
    let executable = executable();
    let output = Command::new(&executable)
        .current_dir(executable.parent().unwrap())
        .arg(default_image())
        .arg("eval")
        .arg("2+2")
        .output()
        .unwrap();

    // extract the raw bytes that we captured and interpret them as a string
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_eq!(stdout.trim(), "4");
}

#[test]
pub fn default_worker_add() {
    let executable = executable();
    let output = Command::new(&executable)
        .current_dir(executable.parent().unwrap())
        .arg("--worker")
        .arg(default_image())
        .arg("eval")
        .arg("2+2")
        .output()
        .unwrap();

    // extract the raw bytes that we captured and interpret them as a string
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_eq!(stdout.trim(), "4");
}

#[test]
pub fn default_worker_is_in_worker() {
    let executable = executable();
    let output = Command::new(&executable)
        .current_dir(executable.parent().unwrap())
        .arg("--worker")
        .arg(default_image())
        .arg("eval")
        .arg("Smalltalk vm isRunningInWorkerThread")
        .output()
        .unwrap();

    // extract the raw bytes that we captured and interpret them as a string
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_eq!(stdout.trim(), "true");
}
