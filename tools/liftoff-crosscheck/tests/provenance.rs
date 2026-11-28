//! Provenance of the fixtures: each names its source, commit and date, and no code reads the
//! soon-to-be-deleted observed-topology documents.
#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn header(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .take_while(|l| l.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

fn field<'a>(header: &'a [String], key: &str) -> Option<&'a str> {
    let prefix = format!("# {key}:");
    header
        .iter()
        .find_map(|l| l.strip_prefix(&prefix))
        .map(str::trim)
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
}

fn is_commit(s: &str) -> bool {
    (7..=40).contains(&s.len()) && s.bytes().all(|c| c.is_ascii_hexdigit())
}

fn fixture(name: &str) -> PathBuf {
    let p = crate_dir().join("fixtures").join(name);
    assert!(p.is_file(), "missing fixture {name}");
    p
}

#[test]
fn probe_fixtures_name_source_commit_and_date() {
    // Sources after the provenance cleanup (a8e92dd and the platform-capabilities move).
    let sources = [
        ("bcm2711-cm4", "docs/platform-capabilities.md"),
        ("bcm2712", "docs/platform-capabilities.md"),
        ("rk3588", "tests/data/rk3588/topology-2026-11-28.md"),
        ("stm32mp257", "docs/platform-capabilities.md"),
    ];
    for (board, want) in sources {
        let h = header(&fixture(&format!("{board}.probe.txt")));
        let source = field(&h, "source").unwrap_or_else(|| panic!("{board}: no source"));
        assert_eq!(source, want, "{board}: source");
        let commit = field(&h, "commit").unwrap_or_else(|| panic!("{board}: no commit"));
        assert!(is_commit(commit), "{board}: bad commit {commit:?}");
        let date = field(&h, "observed").unwrap_or_else(|| panic!("{board}: no date"));
        assert!(is_date(date), "{board}: bad date {date:?}");
        assert_eq!(field(&h, "origin"), Some("observed"), "{board}: origin");
    }
}

#[test]
fn imx95_synthetic_fixture_names_source_and_commit() {
    let h = header(&fixture("imx95-synthetic.xcheck"));
    let source = field(&h, "source").expect("no source");
    assert!(!source.is_empty());
    let commit = field(&h, "commit").expect("no commit");
    assert!(is_commit(commit), "bad commit {commit:?}");
    assert!(
        field(&h, "synthetic").is_some(),
        "synthetic fixture must say so"
    );
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            collect(&p, out);
        } else {
            out.push(p);
        }
    }
}

#[test]
fn code_never_names_the_observed_directory() {
    let needle = ["docs/", "observed"].concat();
    let mut files = Vec::new();
    for d in ["src", "tests"] {
        collect(&crate_dir().join(d), &mut files);
    }
    assert!(!files.is_empty());
    for f in files {
        let text = fs::read_to_string(&f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        assert!(!text.contains(&needle), "{} names {needle}", f.display());
    }
}
