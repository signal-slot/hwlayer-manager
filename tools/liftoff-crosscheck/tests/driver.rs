//! Tests of the TEST_ONLY pipe: line parsing (no C build) and a run of the
//! real driver on `c/testdata/minimal.xcheck` (skipped with a note when the
//! driver cannot be built here).

use liftoff_crosscheck::pipe::{
    build_script, parse_request, parse_result, run_driver, serve, AcceptAll, Answer, Apply, Mode,
    Placement, RejectAll,
};
use liftoff_crosscheck::state::{BlendMode, PlaneState};
use std::path::{Path, PathBuf};
use std::process::Command;

const REQ: &str = "test frame=two-layers seq=2 flags=0x100 planes=2 \
plane=31 fb=100 crtc=40 src16=0,0,125829120,70778880 dst=0,0,1920,1080 alpha=65535 blend=- rotation=0x1 zpos=0 \
plane=32 fb=101 crtc=40 src16=0,0,41943040,31457280 dst=-5,100,640,480 alpha=32768 blend=coverage rotation=0x14 zpos=1";

#[test]
fn request_line_parses() {
    let r = parse_request(REQ).unwrap();
    assert_eq!((r.frame.as_str(), r.seq, r.flags), ("two-layers", 2, 0x100));
    assert_eq!(r.planes.len(), 2);
    let a = r.planes[0].state;
    assert_eq!(
        (a.plane_id, a.fb, a.zpos, a.alpha),
        (31, Some(100), 0, 65535)
    );
    assert_eq!(a.src.w, 125829120);
    assert_eq!(a.blend, BlendMode::Opaque);
    let b = r.planes[1].state;
    assert_eq!(
        (b.crtc.x, b.crtc.y, b.crtc.w, b.crtc.h),
        (-5, 100, 640, 480)
    );
    assert_eq!(
        (b.alpha, b.blend, b.rotation),
        (32768, BlendMode::Coverage, 0x14)
    );
    assert_eq!(r.planes[1].crtc_id, 40);
    assert!(!r.half_set_up());
}

#[test]
fn half_set_up_planes_are_detected() {
    let r = parse_request(&REQ.replace("fb=101", "fb=0")).unwrap();
    assert_eq!(r.planes[1].state.fb, None);
    assert!(r.half_set_up());
    let c = REQ.replace("crtc=40 src16=0,0,41", "crtc=0 src16=0,0,41");
    assert!(parse_request(&c).unwrap().half_set_up());
}

#[test]
fn bad_request_lines_are_refused() {
    let bad = [
        "result frame=x".to_string(),
        "test frame=a seq=1 flags=0x100 planes=1".to_string(),
        "test frame=a seq=x flags=0x100 planes=0".to_string(),
        "test frame=a seq=1 flags=100 planes=0".to_string(),
        REQ.replace("blend=coverage", "blend=weird"),
        REQ.replace("zpos=1", "zpos=99999999999"),
        REQ.replace("src16=0,0,125829120,70778880", "src16=0,0,1"),
    ];
    for b in &bad {
        assert!(parse_request(b).is_err(), "{b}");
    }
    let empty = parse_request("test frame=a seq=1 flags=0x100 planes=0").unwrap();
    assert!(empty.planes.is_empty());
}

const RES: &str = "result frame=two-layers output=40 layers=3 apply=ok placed=2 \
planes=31,composition,none deadline=0 tests=5 origin=synthetic 2026-11-28 a b  c";

#[test]
fn result_line_parses_with_origin() {
    let r = parse_result(RES).unwrap();
    assert_eq!(
        (r.frame.as_str(), r.output, r.layers, r.placed),
        ("two-layers", 40, 3, 2)
    );
    assert_eq!(
        r.planes,
        [
            Placement::Plane(31),
            Placement::Composition,
            Placement::None
        ]
    );
    assert_eq!(
        (r.apply.clone(), r.deadline, r.tests),
        (Apply::Ok, false, 5)
    );
    assert_eq!(r.origin, "synthetic 2026-11-28 a b  c");
}

#[test]
fn result_line_variants() {
    let e = parse_result(
        "result frame=f output=1 layers=0 apply=error:22 placed=0 planes=- deadline=1 tests=0 origin=o",
    )
    .unwrap();
    assert_eq!(e.apply, Apply::Error(22));
    assert!(e.planes.is_empty() && e.deadline);
    let s = parse_result(
        "result frame=f output=1 layers=1 apply=skipped:unknown-buffer placed=0 planes=- deadline=0 tests=0 origin=o",
    )
    .unwrap();
    assert_eq!(s.apply, Apply::Skipped("unknown-buffer".into()));
    assert!(parse_result("result frame=f output=1").is_err());
    assert!(parse_result(&RES.replace(" origin=", " x=")).is_err());
}

#[test]
fn serve_answers_each_request_and_ignores_unknown_lines() {
    let input = format!("future thing\n{REQ}\n{RES}\n");
    let mut out = Vec::new();
    let model = |s: &[PlaneState]| {
        if s.len() == 2 {
            Answer::Enospc
        } else {
            Answer::Ok
        }
    };
    let run = serve(input.as_bytes(), &mut out, Mode::Pipe, &model).unwrap();
    assert_eq!(String::from_utf8(out).unwrap(), "enospc\n");
    assert_eq!(run.frames.len(), 1);
    assert_eq!(run.requests[0].1, Some(Answer::Enospc));
}

/// The pipe's `blend=-` mapping must be one the model accepts: a plane with
/// no blend property takes `BlendMode::Opaque`.
#[test]
fn blend_less_plane_passes_the_model() {
    use liftoff_crosscheck::model::{Model, Setting};
    let file = liftoff_crosscheck::xcheck::read(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("c/testdata/minimal.xcheck"),
        )
        .unwrap(),
    )
    .unwrap();
    let model = Model::new(&file, Setting::Full).unwrap();
    let line = REQ.replace("planes=2", "planes=1");
    let line = &line[..line.find(" plane=32").unwrap()];
    let req = parse_request(line).unwrap();
    assert_eq!(req.planes[0].state.blend, BlendMode::Opaque);
    let out = req.planes[0].crtc_id;
    let answer = |s: &[PlaneState]| match model.check(out, s) {
        Ok(()) => Answer::Ok,
        Err(r) => panic!("model refused a blend-less plane: {}", r.reason),
    };
    let mut sink = Vec::new();
    let input = format!("{line}\n");
    serve(input.as_bytes(), &mut sink, Mode::Pipe, &answer).unwrap();
    assert_eq!(String::from_utf8(sink).unwrap(), "ok\n");
}

#[test]
fn serve_refuses_half_set_up_without_asking_the_model() {
    let input = format!("{}\n", REQ.replace("fb=101", "fb=0"));
    let mut out = Vec::new();
    serve(input.as_bytes(), &mut out, Mode::Pipe, &AcceptAll).unwrap();
    assert_eq!(String::from_utf8(out).unwrap(), "einval\n");
}

#[test]
fn serve_in_driver_mode_writes_no_answers() {
    let input = format!("{REQ}\n");
    let mut out = Vec::new();
    let run = serve(
        input.as_bytes(),
        &mut out,
        Mode::DriverAcceptAll,
        &RejectAll,
    )
    .unwrap();
    assert!(out.is_empty());
    assert_eq!(run.requests[0].1, None);
}

/// Builds the driver, or says why not (then the test is skipped).
fn driver() -> Option<PathBuf> {
    for tool in ["gcc", "ninja", "meson"] {
        if Command::new(tool).arg("--version").output().is_err() {
            eprintln!("SKIP driver integration test: {tool} not found");
            return None;
        }
    }
    let out = match Command::new("bash").arg(build_script()).output() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("SKIP driver integration test: cannot run bash: {e}");
            return None;
        }
    };
    if !out.status.success() {
        eprintln!(
            "SKIP driver integration test: build-liftoff.sh failed (libliftoff checkout or build unavailable?):\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let path = PathBuf::from(stdout.lines().last()?.trim());
    path.is_file().then_some(path)
}

#[test]
fn driver_on_minimal_file() {
    let Some(driver) = driver() else { return };
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("c/testdata/minimal.xcheck");
    let origin = "synthetic 2026-11-28 self-test topology of the C driver, imitates no device";

    let acc = run_driver(&driver, &file, Mode::Pipe, &AcceptAll).unwrap();
    let names: Vec<_> = acc.frames.iter().map(|f| f.frame.as_str()).collect();
    assert_eq!(names, ["one-layer", "two-layers"]);
    for f in &acc.frames {
        assert_eq!(f.origin, origin);
        assert_eq!(f.apply, Apply::Ok);
        assert_eq!(f.output, 40);
        assert!(!f.deadline);
        assert_eq!(f.planes.len() as u32, f.layers);
    }
    // Accepting everything puts every layer on a plane.
    assert_eq!(acc.frames[0].placed, 1);
    assert_eq!(acc.frames[1].placed, 2);
    assert!(acc.frames[1]
        .planes
        .iter()
        .all(|p| matches!(p, Placement::Plane(_))));
    let tests: u32 = acc.frames.iter().map(|f| f.tests).sum();
    assert_eq!(acc.requests.len() as u32, tests);
    assert!(acc.requests.iter().all(|(_, a)| *a == Some(Answer::Ok)));

    // Rejecting everything leaves every layer to composition.
    let rej = run_driver(&driver, &file, Mode::Pipe, &RejectAll).unwrap();
    for f in &rej.frames {
        assert_eq!(f.placed, 0);
        assert!(f.planes.iter().all(|p| *p == Placement::Composition));
        assert_eq!(f.origin, origin);
    }
    assert!(rej.requests.iter().all(|(_, a)| *a == Some(Answer::Einval)));

    // The driver's own modes agree with the pipe answers.
    for (mode, pipe) in [(Mode::DriverAcceptAll, &acc), (Mode::DriverRejectAll, &rej)] {
        let own = run_driver(&driver, &file, mode, &AcceptAll).unwrap();
        assert_eq!(own.frames.len(), pipe.frames.len());
        for (a, b) in own.frames.iter().zip(&pipe.frames) {
            assert_eq!(
                (&a.planes, a.placed, &a.apply),
                (&b.planes, b.placed, &b.apply)
            );
        }
    }
}
