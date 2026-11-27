//! Tests of the comparison (with a stub libliftoff side, no C build) and of
//! the frame generator's determinism.

use composition_core::planner::PlanError;
use liftoff_crosscheck::compare::{
    classify, compare, compare_both, Class, CoreResult, LiftoffSource, ModelCommit, Side,
};
use liftoff_crosscheck::model::{Model, Setting};
use liftoff_crosscheck::pipe::{
    Answer, Apply, FrameResult, Placement, Request, RequestPlane, Run, TestCommit,
};
use liftoff_crosscheck::state::{PlaneState, Rect, URect};
use liftoff_crosscheck::sweep::{self, Bounds};
use liftoff_crosscheck::xcheck::{self, XCheck, ZposDef};

/// Two planes: 10 (primary, fixed zpos 0, no scaling) and 11 (overlay,
/// zpos 1, scaling up to 2x, alpha). Only AR24 on 10; AR24 and NV12 on 11.
const TOPO: &str = "\
xcheck 1
origin synthetic 2026-11-28 compare unit test, imitates no device
profile generic
output 7 100 100 60000
plane 10 kind=primary outputs=0x1 zpos=fixed:0 alpha=0 blend=opaque transforms=normal scaling=none max_source=none full_output=0
  format AR24 0x0
plane 11 kind=overlay outputs=0x1 zpos=fixed:1 alpha=1 blend=opaque transforms=normal scaling=up:0x20000/down:0x10000 max_source=none full_output=0
  format AR24 0x0
  format NV12 0x0
buffer 1 100 100 AR24 0x0
buffer 2 50 50 AR24 0x0
buffer 3 50 50 NV12 0x0
";

const L_FULL: &str =
    "  layer buffer=1 src=0,0,100,100 dst=0,0,100,100 z=0 alpha=65535 blend=opaque transform=normal\n";
const L_SMALL: &str =
    "  layer buffer=2 src=0,0,50,50 dst=10,10,50,50 z=1 alpha=65535 blend=opaque transform=normal\n";

fn file(frames: &[(&str, &[&str])]) -> XCheck {
    let mut s = TOPO.to_string();
    for (name, layers) in frames {
        s.push_str(&format!("frame {name} output=7\n"));
        for l in *layers {
            s.push_str(l);
        }
        s.push_str("end\n");
    }
    xcheck::read(&s).unwrap()
}

/// What the stub libliftoff does with one frame.
#[derive(Clone)]
enum Act {
    /// Put layer i on plane `planes[i]` (in z order), ask the model once;
    /// on `ok` every layer is placed, else all go to composition.
    Greedy,
    /// Report this placement without asking (`apply=ok`).
    Fixed(Vec<Placement>),
    /// `apply=skipped:<why>`.
    Skip(&'static str),
    /// Greedy, with the deadline flag set.
    Deadline,
}

/// A stub libliftoff side: `act(frame name, attempt)`.
struct Stub<F: FnMut(&str, u32) -> Act> {
    act: F,
    runs: u32,
}

fn zpos_of(x: &XCheck, id: u32) -> i32 {
    match x.planes.iter().find(|p| p.id == id).unwrap().zpos {
        ZposDef::Fixed(n) => n,
        ZposDef::Range { .. } => 0,
    }
}

impl<F: FnMut(&str, u32) -> Act> LiftoffSource for Stub<F> {
    fn run(&mut self, x: &XCheck, model: &ModelCommit<'_>, attempt: u32) -> Result<Run, String> {
        self.runs += 1;
        let mut run = Run::default();
        for f in &x.frames {
            let plane_ids: Vec<u32> = x.planes.iter().map(|p| p.id).collect();
            let mut order: Vec<usize> = (0..f.layers.len()).collect();
            order.sort_by_key(|&i| f.layers[i].z);
            let state = |i: usize, plane: u32| {
                let l = &f.layers[i];
                PlaneState {
                    plane_id: plane,
                    fb: Some(l.buffer),
                    src: URect {
                        x: l.src.x << 16,
                        y: l.src.y << 16,
                        w: l.src.w << 16,
                        h: l.src.h << 16,
                    },
                    crtc: Rect { ..l.dst },
                    zpos: zpos_of(x, plane),
                    alpha: l.alpha,
                    blend: l.blend,
                    rotation: l.transform,
                }
            };
            let mut deadline = false;
            let (apply, planes, tests) = match (self.act)(&f.name, attempt) {
                Act::Skip(w) => (Apply::Skipped(w.into()), Vec::new(), 0),
                Act::Fixed(p) => (Apply::Ok, p, 0),
                a @ (Act::Greedy | Act::Deadline) => {
                    deadline = matches!(a, Act::Deadline);
                    let visible: Vec<usize> = order
                        .iter()
                        .copied()
                        .filter(|&i| f.layers[i].alpha != 0)
                        .collect();
                    let mut placements = vec![Placement::None; f.layers.len()];
                    let mut states = Vec::new();
                    let fits = visible.len() <= plane_ids.len();
                    if fits {
                        for (k, &i) in visible.iter().enumerate() {
                            states.push(state(i, plane_ids[k]));
                        }
                    }
                    let ok = fits && model.test(&states) == Answer::Ok;
                    let request = Request {
                        frame: f.name.clone(),
                        seq: 1,
                        flags: 0x100,
                        planes: states
                            .iter()
                            .map(|s| RequestPlane {
                                state: *s,
                                crtc_id: f.output,
                            })
                            .collect(),
                    };
                    run.requests
                        .push((request, Some(if ok { Answer::Ok } else { Answer::Einval })));
                    for (k, &i) in visible.iter().enumerate() {
                        placements[i] = if ok {
                            Placement::Plane(plane_ids[k])
                        } else {
                            Placement::Composition
                        };
                    }
                    (Apply::Ok, placements, 1)
                }
            };
            let placed = planes
                .iter()
                .filter(|p| matches!(p, Placement::Plane(_)))
                .count() as u32;
            run.frames.push(FrameResult {
                frame: f.name.clone(),
                output: f.output,
                layers: f.layers.len() as u32,
                apply,
                placed,
                planes,
                deadline,
                tests,
                origin: format!("{} {}", x.origin.date, x.origin.text),
            });
        }
        Ok(run)
    }
}

fn stub(act: impl FnMut(&str, u32) -> Act) -> Stub<impl FnMut(&str, u32) -> Act> {
    Stub { act, runs: 0 }
}

fn classes(x: &XCheck, setting: Setting, act: impl FnMut(&str, u32) -> Act) -> Vec<Class> {
    let mut s = stub(act);
    let r = compare(x, setting, &mut s).unwrap();
    assert_eq!(s.runs, 2, "the libliftoff side runs twice per output");
    r.into_iter().map(|r| r.class).collect()
}

#[test]
fn both_accept_and_both_reject() {
    let x = file(&[
        ("ok", &[L_FULL, L_SMALL]),
        ("three", &[L_FULL, L_SMALL, L_SMALL]),
    ]);
    assert_eq!(
        classes(&x, Setting::Full, |_, _| Act::Greedy),
        [Class::BothAccept, Class::BothReject]
    );
}

#[test]
fn a_partial_allocation_is_a_rejection_and_keeps_the_count() {
    let x = file(&[("ok", &[L_FULL, L_SMALL])]);
    let mut s = stub(|_, _| Act::Fixed(vec![Placement::Plane(10), Placement::Composition]));
    let r = compare(&x, Setting::Full, &mut s).unwrap();
    assert_eq!(r[0].class, Class::CoreOnly);
    let lo = r[0].liftoff.as_ref().unwrap();
    assert!(!lo.accepted);
    assert_eq!((lo.result.placed, lo.visible), (1, 2));
    assert!(r[0].line("[t]").contains("placed=1/2"));
}

#[test]
fn liftoff_only_when_core_knows_a_constraint_the_model_does_not() {
    // A 2x upscale on the primary plane, which has scaling=none: core refuses
    // the primary (and the overlay needs zpos 1 above it), structural model
    // does not check scaling, so the stub's assignment passes.
    let up = "  layer buffer=2 src=0,0,50,50 dst=0,0,100,100 z=0 alpha=65535 blend=opaque transform=normal\n";
    let x = file(&[("up", &[up, L_SMALL])]);
    let r = {
        let mut s = stub(|_, _| Act::Greedy);
        compare(&x, Setting::Structural, &mut s).unwrap()
    };
    assert_eq!(r[0].class, Class::LiftoffOnly, "{}", r[0].line("[t]"));
    assert!(matches!(r[0].core, CoreResult::Reject(_)));
    // Under the full model the stub's commit is refused as well.
    assert_eq!(
        classes(&x, Setting::Full, |_, _| Act::Greedy),
        [Class::BothReject]
    );
}

#[test]
fn an_assignment_the_model_refuses_is_invalid() {
    // NV12 on plane 10, which lists AR24 only.
    let yuv = "  layer buffer=3 src=0,0,50,50 dst=0,0,50,50 z=0 alpha=65535 blend=opaque transform=normal\n";
    let x = file(&[("yuv", &[yuv])]);
    let mut s = stub(|_, _| Act::Fixed(vec![Placement::Plane(10)]));
    let r = compare(&x, Setting::Structural, &mut s).unwrap();
    assert_eq!(r[0].class, Class::InvalidAssignment(Side::Liftoff));
    assert!(r[0].detail.as_deref().unwrap().contains("not listed"));
    assert_eq!(
        r[0].class.label(),
        "invalid-assignment(liftoff)".to_string()
    );
}

#[test]
fn unstable_skipped_and_deadline() {
    let x = file(&[
        ("flip", &[L_FULL]),
        ("skip", &[L_FULL]),
        ("late", &[L_FULL]),
        (
            "badsrc",
            &["  layer buffer=2 src=0,0,80,80 dst=0,0,80,80 z=0 alpha=65535 blend=opaque transform=normal\n"],
        ),
    ]);
    let got = classes(&x, Setting::Full, |name, attempt| match name {
        "flip" if attempt == 1 => Act::Fixed(vec![Placement::Composition]),
        "skip" => Act::Skip("unknown-buffer"),
        "late" => Act::Deadline,
        _ => Act::Greedy,
    });
    assert_eq!(
        got,
        [
            Class::Unstable,
            Class::Skipped("liftoff:unknown-buffer".into()),
            Class::Skipped("deadline".into()),
            Class::Skipped("InvalidSource".into()),
        ]
    );
}

#[test]
fn alpha_zero_frames_are_compared_without_those_layers() {
    let invisible = "  layer buffer=2 src=0,0,50,50 dst=10,10,50,50 z=1 alpha=0 blend=opaque transform=normal\n";
    let x = file(&[("a0", &[L_FULL, invisible, L_SMALL])]);
    let mut s = stub(|_, _| Act::Greedy);
    let r = compare(&x, Setting::Full, &mut s).unwrap();
    assert!(r[0].alpha0);
    assert_eq!(r[0].class, Class::BothAccept, "{}", r[0].line("[t]"));
    assert!(r[0].line("[t]").contains(" alpha0 "));
}

#[test]
fn every_line_carries_origin_and_setting() {
    let x = file(&[("ok", &[L_FULL])]);
    let mut s = stub(|_, _| Act::Greedy);
    let r = compare_both(&x, &mut s).unwrap();
    assert_eq!(r.len(), 2);
    for (rep, word) in r.iter().zip(["structural", "full"]) {
        let line = rep.line("[synthetic:t]");
        assert!(line.starts_with("[synthetic:t] "), "{line}");
        assert!(line.contains(&format!("model={word}")), "{line}");
        assert!(
            line.ends_with("origin=\"synthetic 2026-11-28 compare unit test, imitates no device\""),
            "{line}"
        );
    }
}

/// The first greedy stub run of `x` and its result for frame 0.
fn greedy_run(x: &XCheck, model: &Model<'_>) -> (Run, FrameResult) {
    let commit = ModelCommit { model, output: 7 };
    let run = stub(|_, _| Act::Greedy).run(x, &commit, 0).unwrap();
    let first = run.frames[0].clone();
    (run, first)
}

#[test]
fn a_core_error_is_classified_as_such() {
    // Core and the model share every rule, so compare() gets an error from
    // core's backend check only once the two drift apart. Drive classify
    // with the errors core then returns.
    let x = file(&[("ok", &[L_FULL, L_SMALL])]);
    for setting in [Setting::Structural, Setting::Full] {
        let model = Model::new(&x, setting).unwrap();
        let (run, first) = greedy_run(&x, &model);
        for (err, name) in [
            (
                PlanError::BackendRejected { candidates: 2 },
                "BackendRejected",
            ),
            (PlanError::LimitExceeded, "LimitExceeded"),
            (PlanError::Aborted, "Aborted"),
        ] {
            let core = CoreResult::Error(err);
            let (class, detail) = classify(
                &x,
                &model,
                &x.frames[0],
                &core,
                Some(&first),
                Some(&first),
                &run,
            );
            assert_eq!(class, Class::CoreError(name.into()));
            assert_eq!(class.label(), format!("core-error({name})"));
            assert!(class.is_finding());
            assert_eq!(detail, None);
        }
    }
}

#[test]
fn a_core_plan_the_model_refuses_is_invalid() {
    // A made-up core plan: the NV12 buffer on plane 10, which lists AR24
    // only.
    let yuv = "  layer buffer=3 src=0,0,50,50 dst=0,0,50,50 z=0 alpha=65535 blend=opaque transform=normal\n";
    let x = file(&[("yuv", &[yuv])]);
    let layer = &x.frames[0].layers[0];
    let bad = PlaneState {
        plane_id: 10,
        fb: Some(3),
        src: URect {
            x: 0,
            y: 0,
            w: 50 << 16,
            h: 50 << 16,
        },
        crtc: Rect {
            x: 0,
            y: 0,
            w: 50,
            h: 50,
        },
        zpos: 0,
        alpha: layer.alpha,
        blend: layer.blend,
        rotation: layer.transform,
    };
    for setting in [Setting::Structural, Setting::Full] {
        let model = Model::new(&x, setting).unwrap();
        let (run, first) = greedy_run(&x, &model);
        let core = CoreResult::Accept(vec![bad]);
        let (class, detail) = classify(
            &x,
            &model,
            &x.frames[0],
            &core,
            Some(&first),
            Some(&first),
            &run,
        );
        assert_eq!(class, Class::InvalidAssignment(Side::Core));
        assert_eq!(class.label(), "invalid-assignment(core)");
        let detail = detail.unwrap();
        assert!(detail.starts_with("core: plane 10:"), "{detail}");
        assert!(
            detail.contains("NV12") && detail.contains("not listed"),
            "{detail}"
        );
        // Through compare(), core's own plan for the frame is valid.
        let mut s = stub(|_, _| Act::Greedy);
        let r = compare(&x, setting, &mut s).unwrap();
        assert_ne!(r[0].class, Class::InvalidAssignment(Side::Core));
    }
}

#[test]
fn class_labels() {
    assert_eq!(
        Class::CoreError("LimitExceeded".into()).label(),
        "core-error(LimitExceeded)"
    );
    assert_eq!(
        Class::InvalidAssignment(Side::Core).label(),
        "invalid-assignment(core)"
    );
    assert!(!Class::BothReject.is_finding());
    assert!(Class::Unstable.is_finding());
}

fn imx95() -> XCheck {
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/imx95-synthetic.xcheck"),
    )
    .unwrap();
    xcheck::read(&text).unwrap()
}

#[test]
fn the_generator_is_deterministic() {
    let x = imx95();
    let a = sweep::generate(&x, "imx95-synthetic", Bounds::default());
    let b = sweep::generate(&x, "imx95-synthetic", Bounds::default());
    assert_eq!(a, b);
    let names: Vec<&str> = a.iter().map(|s| s.family).collect();
    for f in sweep::FAMILIES {
        assert!(names.contains(&f), "family {f} missing");
    }
    // A different seed changes the sampled family only; fewer samples keep
    // the first ones.
    let c = sweep::generate(
        &x,
        "imx95-synthetic",
        Bounds {
            seed: 1,
            ..Bounds::default()
        },
    );
    for (p, q) in a.iter().zip(&c) {
        assert_eq!(
            p.family == "rand",
            p.xcheck != q.xcheck,
            "family {}",
            p.family
        );
    }
    let few = sweep::generate(
        &x,
        "imx95-synthetic",
        Bounds {
            samples: 10,
            ..Bounds::default()
        },
    );
    let rand_a = &a.iter().find(|s| s.family == "rand").unwrap().xcheck.frames;
    let rand_f = &few
        .iter()
        .find(|s| s.family == "rand")
        .unwrap()
        .xcheck
        .frames;
    assert_eq!(rand_f.len(), 10);
    for (p, q) in rand_f.iter().zip(rand_a) {
        assert_eq!(p.name, q.name);
        assert_eq!(p.layers.len(), q.layers.len());
    }
}

#[test]
fn generated_files_replay_and_stay_inside_the_output() {
    let x = imx95();
    let out = &x.outputs[0];
    for set in sweep::generate(&x, "imx95-synthetic", Bounds::default()) {
        let back = xcheck::read(&set.text).unwrap();
        assert_eq!(back, set.xcheck);
        assert_eq!(back.origin, x.origin);
        assert!(set.text.contains("# GEN_VERSION="));
        let name = set.file_name("imx95-synthetic");
        assert!(name.starts_with("imx95-synthetic.synthetic."), "{name}");
        for f in &back.frames {
            assert!(f.name.starts_with(set.family));
            assert!(!f.layers.is_empty());
            for l in &f.layers {
                assert!(l.dst.x >= 0 && l.dst.y >= 0);
                assert!(l.dst.x as u32 + l.dst.w <= out.width);
                assert!(l.dst.y as u32 + l.dst.h <= out.height);
            }
            if set.family == "alpha0" {
                assert!(f.layers.iter().any(|l| l.alpha == 0));
            } else {
                assert!(f.layers.iter().all(|l| l.alpha != 0));
            }
            let one = sweep::single_frame(&back, &f.name).unwrap();
            xcheck::write(&one).unwrap();
        }
    }
}

#[test]
fn tie_frames_have_equal_z() {
    let x = imx95();
    let sets = sweep::generate(&x, "imx95-synthetic", Bounds::default());
    let tie = sets.iter().find(|s| s.family == "tie").unwrap();
    for f in &tie.xcheck.frames {
        let z: Vec<i32> = f.layers.iter().map(|l| l.z).collect();
        assert!(z.windows(2).any(|w| w[0] == w[1]), "{}: {z:?}", f.name);
    }
}

#[test]
fn output_zero_is_remapped_for_the_driver_only() {
    let x = imx95();
    assert_eq!(x.outputs[0].id, 0);
    let d = liftoff_crosscheck::compare::driver_file(&x);
    let id = d.outputs[0].id;
    assert_ne!(id, 0);
    assert!(x.planes.iter().all(|p| p.id != id));
    assert!(x.buffers.iter().all(|b| b.id != id));
    assert!(d.frames.iter().all(|f| f.output == id));
    // Files without output 0 pass unchanged.
    assert_eq!(liftoff_crosscheck::compare::driver_file(&d), d);
}
