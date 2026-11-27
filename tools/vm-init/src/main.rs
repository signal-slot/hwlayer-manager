//! PID 1 of the test VM: mounts the pseudo filesystems, loads the kernel
//! modules listed in `/modules.list`, runs every executable in `/tests` and
//! restarts the machine, which makes QEMU exit.
//!
//! If `/commands.list` exists, its lines are run instead of `/tests`, one
//! command line per line (words separated by white space, no quoting):
//!
//! - `cmd args...`: runs it and waits; it passes if it exits with 0.
//! - `& cmd args...`: starts it in the background; it is killed after the
//!   last line.
//! - `wait <path>`: waits up to 10 s for `<path>` to exist.
//! - `#` starts a comment line.
//!
//! The result is reported on the console as `VM_TEST_RESULT=<0|1>`.

#![forbid(unsafe_code)]

use std::ffi::{CStr, CString};
use std::fs;
use std::path::Path;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use rustix::mount::{mount, MountFlags};
use rustix::system::{finit_module, reboot, RebootCommand};

fn setup() -> Result<(), String> {
    for (source, target, kind) in [
        ("proc", "/proc", "proc"),
        ("sysfs", "/sys", "sysfs"),
        ("devtmpfs", "/dev", "devtmpfs"),
        ("debugfs", "/sys/kernel/debug", "debugfs"),
        ("tmpfs", "/tmp", "tmpfs"),
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

/// Runs the lines of `/commands.list`; see the module documentation.
fn run_commands(list: &str) -> Result<bool, String> {
    let mut passed = true;
    let mut background: Vec<(String, Child)> = Vec::new();
    for line in list.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        println!("--- command: {line}");
        let (detach, words) = match line.strip_prefix('&') {
            Some(rest) => (true, rest),
            None => (false, line),
        };
        let words: Vec<&str> = words.split_whitespace().collect();
        let Some((program, args)) = words.split_first() else {
            return Err(format!("empty command: {line}"));
        };
        if *program == "wait" && !detach {
            let [path] = args else {
                return Err(format!("wait needs one path: {line}"));
            };
            let deadline = Instant::now() + Duration::from_secs(10);
            while !Path::new(path).exists() {
                if Instant::now() > deadline {
                    println!("--- timeout waiting for {path}");
                    passed = false;
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            continue;
        }
        let mut command = Command::new(program);
        command.args(args);
        if detach {
            let child = command.spawn().map_err(|e| format!("start {line}: {e}"))?;
            background.push((line.to_string(), child));
            continue;
        }
        let status = command.status().map_err(|e| format!("run {line}: {e}"))?;
        println!("--- exit {}: {line}", status.code().unwrap_or(-1));
        passed &= status.success();
    }
    for (line, mut child) in background {
        if let Ok(None) = child.try_wait() {
            let _ = child.kill();
        }
        let status = child.wait().map_err(|e| format!("wait {line}: {e}"))?;
        println!("--- background ended ({status}): {line}");
    }
    Ok(passed)
}

fn main() {
    let run = || match fs::read_to_string("/commands.list") {
        Ok(list) => run_commands(&list),
        Err(_) => run_tests(),
    };
    let passed = match setup().and_then(|()| run()) {
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
