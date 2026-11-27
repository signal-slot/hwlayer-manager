//! Command line of the cross-check.
//!
//! ```text
//! liftoff-crosscheck convert <probe.txt> [--allow-missing-formats]
//! liftoff-crosscheck run <file.xcheck> --driver <path> [--trace]
//! liftoff-crosscheck sweep <file.xcheck> --driver <path> [--out DIR] [--samples N] [--seed N]
//! ```
//!
//! Not in CI; recorded or synthetic topologies give no hardware claim, and
//! every result line carries the origin of its input.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use liftoff_crosscheck::compare::{
    self, origin_word, setting_word, DriverSource, FrameReport, LiftoffSource,
};
use liftoff_crosscheck::model::Setting;
use liftoff_crosscheck::probe;
use liftoff_crosscheck::sweep::{self, Bounds, GEN_VERSION};
use liftoff_crosscheck::xcheck::{self, OriginKind, Profile, XCheck};

const USAGE: &str = "\
usage: liftoff-crosscheck convert <probe.txt> [--allow-missing-formats]
       liftoff-crosscheck run <file.xcheck> --driver <path> [--trace]
       liftoff-crosscheck sweep <file.xcheck> --driver <path> [--out DIR] [--samples N] [--seed N]

The driver is the last line printed by tools/liftoff-crosscheck/build-liftoff.sh.
Not in CI. No hardware claim: results describe the planners on the input topology.";

/// Findings printed per class before the rest goes to the files only.
const PRINT_PER_CLASS: usize = 20;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match real_main(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("liftoff-crosscheck: {e}");
            ExitCode::FAILURE
        }
    }
}

struct Opts {
    positional: Vec<String>,
    driver: Option<PathBuf>,
    out: Option<PathBuf>,
    samples: Option<u32>,
    seed: Option<u64>,
    allow_missing: bool,
    trace: bool,
}

fn parse(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        positional: Vec::new(),
        driver: None,
        out: None,
        samples: None,
        seed: None,
        allow_missing: false,
        trace: false,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = |name: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match a.as_str() {
            "--driver" => o.driver = Some(value("--driver")?.into()),
            "--out" => o.out = Some(value("--out")?.into()),
            "--samples" => {
                let v = value("--samples")?;
                o.samples = Some(v.parse().map_err(|_| format!("bad --samples {v:?}"))?);
            }
            "--seed" => {
                let v = value("--seed")?;
                let n = match v.strip_prefix("0x") {
                    Some(h) => u64::from_str_radix(h, 16),
                    None => v.parse(),
                };
                o.seed = Some(n.map_err(|_| format!("bad --seed {v:?}"))?);
            }
            "--allow-missing-formats" => o.allow_missing = true,
            "--trace" => o.trace = true,
            "-h" | "--help" => return Err(USAGE.into()),
            s if s.starts_with("--") => return Err(format!("unknown option {s}\n{USAGE}")),
            _ => o.positional.push(a.clone()),
        }
    }
    Ok(o)
}

fn real_main(args: &[String]) -> Result<(), String> {
    let Some((cmd, rest)) = args.split_first() else {
        return Err(USAGE.into());
    };
    let o = parse(rest)?;
    let [input] = o.positional.as_slice() else {
        return Err(format!("{cmd}: exactly one input file\n{USAGE}"));
    };
    let input = Path::new(input);
    match cmd.as_str() {
        "convert" => convert(input, o.allow_missing),
        "run" => {
            let driver = o.driver.ok_or("run needs --driver <path>")?;
            run(input, &driver, o.trace)
        }
        "sweep" => {
            let driver = o.driver.ok_or("sweep needs --driver <path>")?;
            let bounds = Bounds {
                seed: o.seed.unwrap_or(sweep::DEFAULT_SEED),
                samples: o.samples.unwrap_or(sweep::DEFAULT_SAMPLES),
            };
            let out = match o.out {
                Some(d) => d,
                None => std::env::var_os("TMPDIR")
                    .map(|t| PathBuf::from(t).join("liftoff-crosscheck-sweep"))
                    .ok_or("sweep needs --out DIR (TMPDIR is not set)")?,
            };
            sweep_cmd(input, &driver, &out, bounds)
        }
        _ => Err(format!("unknown command {cmd:?}\n{USAGE}")),
    }
}

fn read_file(path: &Path) -> Result<XCheck, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    xcheck::read(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn stem(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.trim_end_matches(".xcheck").to_string())
        .unwrap_or_else(|| "input".into())
}

fn convert(input: &Path, allow_missing: bool) -> Result<(), String> {
    let text = std::fs::read_to_string(input).map_err(|e| format!("{}: {e}", input.display()))?;
    let c = probe::convert_with(&text, allow_missing)
        .map_err(|e| format!("{}: {e}", input.display()))?;
    let body = xcheck::write(&c.xcheck).map_err(|e| format!("cannot write xcheck: {e}"))?;
    println!(
        "# converted from {} by liftoff-crosscheck convert",
        input.display()
    );
    if c.formats_incomplete {
        println!(
            "# formats incomplete: planes without formats {:?}",
            c.planes_without_formats
        );
    }
    if !c.truncated_planes.is_empty() {
        println!(
            "# truncated? format lists at the probe's cut length on planes {:?}",
            c.truncated_planes
        );
    }
    println!("# no hardware claim beyond the recorded dump; profile generic, no constraints");
    print!("{body}");
    Ok(())
}

/// The origin footer of design section 8 / frame-generator notes section 8.
fn claim(x: &XCheck) -> String {
    match x.origin.kind {
        OriginKind::Observed => format!(
            "topology recorded at {}; no claim about any other board, kernel or boot",
            x.origin.date
        ),
        OriginKind::Source => format!(
            "no hardware claim: topology is hand-written from sources ({})",
            x.origin.date
        ),
        OriginKind::Synthetic => format!(
            "no hardware claim: topology is synthetic ({})",
            x.origin.date
        ),
    }
}

fn profile_word(p: Profile) -> &'static str {
    match p {
        Profile::Generic => "generic",
        Profile::Imx95Dpu => "imx95dpu",
    }
}

/// One compared frame with where it can be replayed from.
struct Entry {
    report: FrameReport,
    family: String,
    file: PathBuf,
}

fn summary(out: &mut String, x: &XCheck, entries: &[Entry]) {
    for setting in [Setting::Structural, Setting::Full] {
        let core = match setting {
            Setting::Structural => "core: profile generic, accept-all check".to_string(),
            Setting::Full => format!("core: profile {}, model check", profile_word(x.profile)),
        };
        let _ = writeln!(out, "\n== model={} ({core}) ==", setting_word(setting));
        let mine: Vec<&Entry> = entries
            .iter()
            .filter(|e| e.report.setting == setting)
            .collect();
        let mut classes: BTreeMap<String, usize> = BTreeMap::new();
        let mut families: BTreeMap<&str, BTreeMap<String, usize>> = BTreeMap::new();
        for e in &mine {
            let label = e.report.class.label();
            *classes.entry(label.clone()).or_default() += 1;
            *families
                .entry(e.family.as_str())
                .or_default()
                .entry(label)
                .or_default() += 1;
        }
        let _ = writeln!(out, "{:<32} {:>6}", "class", "frames");
        for (c, n) in &classes {
            let _ = writeln!(out, "{c:<32} {n:>6}");
        }
        let _ = writeln!(out, "{:<32} {:>6}", "total", mine.len());
        let alpha0 = mine.iter().filter(|e| e.report.alpha0).count();
        if alpha0 > 0 {
            let _ = writeln!(
                out,
                "(alpha0 frames, compared without their alpha-0 layers: {alpha0})"
            );
        }
        let _ = writeln!(out, "by family:");
        for (f, cs) in &families {
            let parts: Vec<String> = cs.iter().map(|(c, n)| format!("{c}={n}")).collect();
            let _ = writeln!(out, "  {f:<8} {}", parts.join(" "));
        }
    }
}

fn findings(out: &mut String, label: &str, entries: &[Entry], print_cap: Option<usize>) {
    let found: Vec<&Entry> = entries
        .iter()
        .filter(|e| e.report.class.is_finding())
        .collect();
    let _ = writeln!(out, "\n== findings ({}) ==", found.len());
    let mut per_class: BTreeMap<String, usize> = BTreeMap::new();
    let mut hidden = 0;
    for e in found {
        let n = per_class.entry(e.report.class.label()).or_default();
        *n += 1;
        if print_cap.is_some_and(|cap| *n > cap) {
            hidden += 1;
            continue;
        }
        let _ = writeln!(out, "{} file={}", e.report.line(label), e.file.display());
    }
    if hidden > 0 {
        let _ = writeln!(
            out,
            "({hidden} more findings beyond {PRINT_PER_CLASS} per class: see findings.txt)"
        );
    }
}

fn header(out: &mut String, cmd: &str, input: &Path, x: &XCheck, driver: &Path) {
    let _ = writeln!(
        out,
        "liftoff-crosscheck {cmd} (not in CI; no hardware claim)"
    );
    let _ = writeln!(out, "input: {}", input.display());
    let _ = writeln!(
        out,
        "origin: {} {} {}",
        origin_word(x.origin.kind),
        x.origin.date,
        x.origin.text
    );
    let _ = writeln!(out, "profile: {}", profile_word(x.profile));
    let _ = writeln!(out, "driver: {}", driver.display());
}

fn scratch_dir(base: Option<&Path>) -> PathBuf {
    match base {
        Some(b) => b.join("scratch"),
        None => std::env::var_os("TMPDIR")
            .map_or_else(std::env::temp_dir, PathBuf::from)
            .join(format!("liftoff-crosscheck-run-{}", std::process::id())),
    }
}

fn run(input: &Path, driver: &Path, trace: bool) -> Result<(), String> {
    let x = read_file(input)?;
    let topology = stem(input);
    let label = format!("[{}:{topology}]", origin_word(x.origin.kind));
    let scratch = scratch_dir(None);
    let mut src = DriverSource::new(driver, &scratch);
    src.tracing = trace;
    let reports = compare::compare_both(&x, &mut src).map_err(|e| e.to_string());
    let _ = std::fs::remove_dir_all(&scratch);
    let entries: Vec<Entry> = reports?
        .into_iter()
        .map(|report| Entry {
            report,
            family: "file".into(),
            file: input.to_path_buf(),
        })
        .collect();
    let mut out = String::new();
    header(&mut out, "run", input, &x, driver);
    summary(&mut out, &x, &entries);
    let _ = writeln!(out, "\n== all frames ==");
    for e in &entries {
        let _ = writeln!(out, "{}", e.report.line(&label));
    }
    findings(&mut out, &label, &entries, None);
    if trace {
        let _ = writeln!(out, "\n== test commits (first run) ==");
        for t in &src.trace {
            let _ = writeln!(out, "{t}");
        }
    }
    let _ = writeln!(out, "\n{}", claim(&x));
    print!("{out}");
    Ok(())
}

fn write_finding(
    dir: &Path,
    topology: &str,
    set: &XCheck,
    report: &FrameReport,
    bounds: Bounds,
) -> Result<PathBuf, String> {
    let one = sweep::single_frame(set, &report.frame)
        .ok_or_else(|| format!("frame {} not in its set", report.frame))?;
    let (family, index) = report
        .frame
        .split_once('.')
        .unwrap_or(("file", report.frame.as_str()));
    let fam_id = sweep::FAMILIES
        .iter()
        .position(|f| *f == family)
        .unwrap_or(sweep::FAMILIES.len());
    let idx: u32 = index.parse().unwrap_or(0);
    let seed = sweep::case_seed(bounds.seed, topology, fam_id, idx);
    let name = format!(
        "{topology}.{}.{family}.{index}.s{seed:016x}.{}.xcheck",
        origin_word(set.origin.kind),
        setting_word(report.setting)
    );
    let mut text = String::new();
    let _ = writeln!(
        text,
        "# finding of liftoff-crosscheck sweep: {}",
        report.class
    );
    let _ = writeln!(
        text,
        "# GEN_VERSION={GEN_VERSION} seed=0x{:016x} case_seed=0x{seed:016x}",
        bounds.seed
    );
    let _ = writeln!(
        text,
        "# family={family} index={index} topology={topology} model={}",
        setting_word(report.setting)
    );
    text.push_str(&xcheck::write(&one).map_err(|e| format!("cannot write finding: {e}"))?);
    let path = dir.join(name);
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

fn sweep_cmd(input: &Path, driver: &Path, out_dir: &Path, bounds: Bounds) -> Result<(), String> {
    let x = read_file(input)?;
    let topology = stem(input);
    let label = format!("[{}:{topology}]", origin_word(x.origin.kind));
    let dir = out_dir.join(&topology);
    let finding_dir = dir.join("findings");
    std::fs::create_dir_all(&finding_dir).map_err(|e| format!("{}: {e}", finding_dir.display()))?;
    let mut src = DriverSource::new(driver, &dir.join("scratch"));

    // The input's own frames first (family `file`), then the generated sets.
    let mut sets: Vec<(String, PathBuf, XCheck)> = Vec::new();
    if !x.frames.is_empty() {
        sets.push(("file".into(), input.to_path_buf(), x.clone()));
    }
    let generated = sweep::generate(&x, &topology, bounds);
    for set in &generated {
        let path = dir.join(set.file_name(&topology));
        std::fs::write(&path, &set.text).map_err(|e| format!("{}: {e}", path.display()))?;
        sets.push((set.family.to_string(), path, set.xcheck.clone()));
    }

    let started = std::time::Instant::now();
    let mut entries = Vec::new();
    for (family, path, set) in &sets {
        let reports = compare::compare_both(set, &mut src as &mut dyn LiftoffSource)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        for report in reports {
            let file = if report.class.is_finding() && family != "file" {
                write_finding(&finding_dir, &topology, set, &report, bounds)?
            } else {
                path.clone()
            };
            entries.push(Entry {
                report,
                family: family.clone(),
                file,
            });
        }
    }
    let _ = std::fs::remove_dir_all(dir.join("scratch"));

    let mut out = String::new();
    header(&mut out, "sweep", input, &x, driver);
    let _ = writeln!(
        out,
        "generator: GEN_VERSION={GEN_VERSION} seed=0x{:016x} samples={} edge=off (all layers inside the output)",
        bounds.seed, bounds.samples
    );
    let _ = writeln!(out, "frame files: {} ({} sets)", dir.display(), sets.len());
    let _ = writeln!(out, "elapsed: {:.1} s", started.elapsed().as_secs_f64());
    summary(&mut out, &x, &entries);
    let mut full = out.clone();
    findings(&mut out, &label, &entries, Some(PRINT_PER_CLASS));
    findings(&mut full, &label, &entries, None);
    let _ = writeln!(out, "\n{}", claim(&x));
    let _ = writeln!(full, "\n{}", claim(&x));
    let report_path = dir.join("findings.txt");
    std::fs::write(&report_path, &full).map_err(|e| format!("{}: {e}", report_path.display()))?;
    print!("{out}");
    Ok(())
}
