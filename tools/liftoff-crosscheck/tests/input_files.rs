//! Every committed `xcheck` input file (topologies, frames, calibration)
//! parses and meets the format's own rules (design section 5).

use liftoff_crosscheck::xcheck::{self, Profile, XCheck, ZposDef};
use std::fs;
use std::path::{Path, PathBuf};

const DIRS: [&str; 3] = ["topologies", "frames", "calibration"];

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return; // a directory that does not exist yet has no files
    };
    for entry in entries {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e == "xcheck") {
            out.push(path);
        }
    }
}

fn files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut out = Vec::new();
    for d in DIRS {
        collect(&root.join(d), &mut out);
    }
    out.sort();
    out
}

fn name(path: &Path) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b.iter().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                *c == b'-'
            } else {
                c.is_ascii_digit()
            }
        })
        && (1..=12).contains(&s[5..7].parse::<u32>().unwrap_or(0))
        && (1..=31).contains(&s[8..10].parse::<u32>().unwrap_or(0))
}

fn read_file(path: &Path) -> Result<XCheck, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("unreadable: {e}"))?;
    xcheck::read(&text).map_err(|e| e.to_string())
}

fn check_origin(x: &XCheck) -> Result<(), String> {
    // The reader only yields a kind of observed, source or synthetic.
    if is_date(&x.origin.date) {
        Ok(())
    } else {
        Err(format!("origin date {:?} is not YYYY-MM-DD", x.origin.date))
    }
}

fn check_zpos_now(x: &XCheck) -> Result<(), String> {
    for p in &x.planes {
        if matches!(p.zpos, ZposDef::Range { .. }) && p.zpos_now.is_none() {
            return Err(format!("plane {} has a zpos range without zpos_now", p.id));
        }
    }
    Ok(())
}

fn check_imx95(x: &XCheck) -> Result<(), String> {
    if x.profile != Profile::Imx95Dpu {
        return Ok(());
    }
    if !x.groups.is_empty() {
        return Err("profile imx95dpu with explicit group lines".into());
    }
    for p in &x.planes {
        if p.scaling.is_some() || p.max_source.is_some() {
            return Err(format!(
                "profile imx95dpu with explicit scaling/max_source on plane {}",
                p.id
            ));
        }
    }
    Ok(())
}

fn check_round_trip(x: &XCheck) -> Result<(), String> {
    let text = xcheck::write(x).map_err(|e| format!("writer refused: {e}"))?;
    let back = xcheck::read(&text).map_err(|e| format!("re-read of written text: {e}"))?;
    if &back == x {
        Ok(())
    } else {
        Err("write then read gives a different value".into())
    }
}

/// Runs `check` on every parsed file and reports all failures at once.
fn run(check: impl Fn(&XCheck) -> Result<(), String>) {
    let mut failures = Vec::new();
    for path in files() {
        if let Err(e) = read_file(&path).and_then(|x| check(&x)) {
            failures.push(format!("{}: {e}", name(&path)));
        }
    }
    assert!(
        failures.is_empty(),
        "{} file(s) failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn input_files_exist() {
    let found = files();
    assert!(
        found.iter().any(|p| name(p).starts_with("topologies")),
        "no topologies/*.xcheck found"
    );
}

#[test]
fn reader_accepts_every_file() {
    run(|_| Ok(()));
}

#[test]
fn origin_has_kind_and_date() {
    run(check_origin);
}

#[test]
fn range_zpos_has_zpos_now() {
    run(check_zpos_now);
}

#[test]
fn imx95_profile_has_no_explicit_limits() {
    run(check_imx95);
}

#[test]
fn write_read_round_trip() {
    run(check_round_trip);
}
