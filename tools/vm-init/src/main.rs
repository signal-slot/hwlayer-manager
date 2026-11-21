//! PID 1 of the test VM: mounts the pseudo filesystems, loads the kernel
//! modules listed in `/modules.list`, runs every executable in `/tests` and
//! restarts the machine, which makes QEMU exit.
//!
//! The result is reported on the console as `VM_TEST_RESULT=<0|1>`.

#![forbid(unsafe_code)]

use std::ffi::{CStr, CString};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use rustix::mount::{mount, MountFlags};
use rustix::system::{finit_module, reboot, RebootCommand};

fn setup() -> Result<(), String> {
    for (source, target, kind) in [
        ("proc", "/proc", "proc"),
        ("sysfs", "/sys", "sysfs"),
        ("devtmpfs", "/dev", "devtmpfs"),
        ("debugfs", "/sys/kernel/debug", "debugfs"),
    ] {
        fs::create_dir_all(target).map_err(|e| format!("mkdir {target}: {e}"))?;
        mount(source, target, kind, MountFlags::empty(), None::<&CStr>)
            .map_err(|e| format!("mount {target}: {e}"))?;
    }

    let list = fs::read_to_string("/modules.list").map_err(|e| format!("modules.list: {e}"))?;
    for line in list.lines().filter(|l| !l.trim().is_empty()) {
        let (path, params) = line.split_once(' ').unwrap_or((line, ""));
        let file = fs::File::open(path).map_err(|e| format!("open {path}: {e}"))?;
        let params = CString::new(params).map_err(|e| e.to_string())?;
        finit_module(&file, &params, 0).map_err(|e| format!("load {path}: {e}"))?;
    }

    // The device node appears asynchronously after the module loads.
    let deadline = Instant::now() + Duration::from_secs(5);
    while !Path::new("/dev/dri/card0").exists() {
        if Instant::now() > deadline {
            return Err("no /dev/dri/card0 after loading the modules".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

fn run_tests() -> Result<bool, String> {
    let mut tests: Vec<_> = fs::read_dir("/tests")
        .map_err(|e| format!("/tests: {e}"))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .collect();
    tests.sort();
    let mut passed = !tests.is_empty();
    for test in tests {
        println!("--- {}", test.display());
        let status = Command::new(&test)
            .args(["--ignored", "--test-threads=1"])
            .status()
            .map_err(|e| format!("run {}: {e}", test.display()))?;
        passed &= status.success();
    }
    Ok(passed)
}

fn main() {
    let passed = match setup().and_then(|()| run_tests()) {
        Ok(passed) => passed,
        Err(message) => {
            println!("vm-init: {message}");
            false
        }
    };
    println!("VM_TEST_RESULT={}", if passed { 0 } else { 1 });
    rustix::fs::sync();
    // With `-no-reboot`, a restart ends the QEMU process.
    let _ = reboot(RebootCommand::Restart);
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}
