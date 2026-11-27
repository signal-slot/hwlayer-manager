//! Property tests of the fixed-size and fixed-point parts of composition-core.
//!
//! `fixed.rs` holds the fixed-capacity containers (`FixedVec`, `FixedQueue`);
//! it contains no fixed-point arithmetic. The only 16.16 arithmetic in the
//! crate is the planner's scale-limit check, reached through the public
//! `plan` API with `ScaleLimits`. Both are tested here against references
//! computed in `i128` or with a model, using a seeded deterministic
//! generator, so the crate keeps zero dependencies. Failures print inputs.

use composition_core::fixed::{CapacityError, FixedQueue, FixedVec};
use composition_core::{
    planner::plan, BufferId, BufferInfo, BufferLookup, Capabilities, FormatModifier, Frame, Layer,
    Modifier, OutputCaps, OutputId, PixelFormat, PlanError, PlaneCaps, PlaneId, PlaneKind, Rect,
    RejectReasons, ScaleLimits, Size, Transform, Zpos,
};

/// splitmix64: small, seeded, deterministic.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    /// A u32 biased towards boundary values.
    fn u32(&mut self) -> u32 {
        const EDGE: [u32; 10] = [
            0,
            1,
            2,
            0xFFFF,
            0x1_0000,
            0x1_0001,
            0x7FFF_FFFF,
            0x8000_0000,
            u32::MAX - 1,
            u32::MAX,
        ];
        match self.below(4) {
            0 => EDGE[self.below(EDGE.len() as u64) as usize],
            1 => self.below(4096) as u32,
            2 => (self.next() as u32) >> self.below(32),
            _ => self.next() as u32,
        }
    }
}

// ---------------------------------------------------------------- containers

const CAP: usize = 5;

#[test]
fn fixed_vec_matches_model() {
    for seed in 0..200u64 {
        let mut rng = Rng(seed);
        let mut v: FixedVec<u32, CAP> = FixedVec::new();
        let mut model: Vec<u32> = Vec::new();
        let mut log: Vec<String> = Vec::new();
        for _ in 0..200 {
            match rng.below(6) {
                0 | 1 => {
                    let x = rng.u32();
                    log.push(format!("push({x})"));
                    let r = v.push(x);
                    if model.len() == CAP {
                        assert_eq!(r, Err(CapacityError), "seed {seed} {log:?}");
                    } else {
                        assert_eq!(r, Ok(()), "seed {seed} {log:?}");
                        model.push(x);
                    }
                }
                2 => {
                    log.push("pop".into());
                    assert_eq!(v.pop(), model.pop(), "seed {seed} {log:?}");
                }
                3 if !model.is_empty() => {
                    let i = rng.below(model.len() as u64) as usize;
                    log.push(format!("swap_remove({i})"));
                    let expect = model.swap_remove(i);
                    assert_eq!(v.swap_remove(i), expect, "seed {seed} {log:?}");
                }
                4 if rng.below(8) == 0 => {
                    log.push("clear".into());
                    v.clear();
                    model.clear();
                }
                _ => {
                    for s in v.as_mut_slice() {
                        *s = s.wrapping_add(1);
                    }
                    for s in model.iter_mut() {
                        *s = s.wrapping_add(1);
                    }
                    log.push("bump".into());
                }
            }
            assert_eq!(v.as_slice(), &model[..], "seed {seed} {log:?}");
            assert_eq!(&*v, &model[..], "seed {seed} {log:?}");
            assert_eq!(v.remaining(), CAP - model.len(), "seed {seed} {log:?}");
        }
    }
}

#[test]
fn fixed_vec_from_slice_boundaries() {
    for n in 0..=CAP + 2 {
        let src: Vec<u32> = (0..n as u32).collect();
        let r = FixedVec::<u32, CAP>::from_slice(&src);
        if n <= CAP {
            assert_eq!(r.unwrap().as_slice(), &src[..], "n={n}");
        } else {
            assert_eq!(r.unwrap_err(), CapacityError, "n={n}");
        }
    }
}

#[test]
fn fixed_vec_zero_capacity() {
    let mut v: FixedVec<u8, 0> = FixedVec::new();
    assert_eq!(v.push(1), Err(CapacityError));
    assert_eq!(v.pop(), None);
    assert_eq!(v.remaining(), 0);
    assert!(v.is_empty());
    assert_eq!(FixedVec::<u8, 0>::from_slice(&[]).unwrap().len(), 0);
}

#[test]
#[should_panic]
fn fixed_vec_swap_remove_out_of_range_panics() {
    // The index must be in range; the assert is the only promise made.
    let mut v: FixedVec<u8, 3> = FixedVec::from_slice(&[1, 2]).unwrap();
    v.swap_remove(2);
}

#[test]
fn fixed_vec_equality_ignores_unused_slots() {
    let mut a: FixedVec<u8, 4> = FixedVec::from_slice(&[1, 2, 3]).unwrap();
    let b: FixedVec<u8, 4> = FixedVec::from_slice(&[1]).unwrap();
    a.pop();
    a.pop();
    assert_eq!(a, b);
    let c = FixedVec::<u8, 4>::empty_filled(9);
    assert_eq!(c, FixedVec::new());
}

#[test]
fn fixed_queue_matches_model() {
    for seed in 0..200u64 {
        let mut rng = Rng(seed ^ 0xABCD);
        let mut q: FixedQueue<u32, CAP> = FixedQueue::new();
        let mut model: std::collections::VecDeque<u32> = Default::default();
        let mut log: Vec<String> = Vec::new();
        for _ in 0..300 {
            if rng.below(2) == 0 {
                let x = rng.u32();
                log.push(format!("push({x})"));
                let r = q.push(x);
                if model.len() == CAP {
                    assert_eq!(r, Err(CapacityError), "seed {seed} {log:?}");
                } else {
                    assert_eq!(r, Ok(()), "seed {seed} {log:?}");
                    model.push_back(x);
                }
            } else {
                log.push("pop".into());
                assert_eq!(q.pop(), model.pop_front(), "seed {seed} {log:?}");
            }
            assert_eq!(q.len(), model.len(), "seed {seed} {log:?}");
            assert_eq!(q.is_empty(), model.is_empty());
            assert_eq!(q.remaining(), CAP - model.len());
            assert_eq!(
                format!("{q:?}"),
                format!("{:?}", model.iter().collect::<Vec<_>>()),
                "seed {seed} {log:?}"
            );
        }
    }
}

#[test]
fn fixed_queue_zero_capacity_does_not_divide_by_zero() {
    let mut q: FixedQueue<u8, 0> = FixedQueue::new();
    assert_eq!(q.push(1), Err(CapacityError));
    assert_eq!(q.pop(), None);
    assert!(q.is_empty());
}

// ------------------------------------------------- 16.16 scale-limit check

const XRGB: PixelFormat = PixelFormat::XRGB8888;
const OUT: OutputId = OutputId(0);
const OUT_SIZE: Size = Size::new(1920, 1080);

struct One(BufferInfo);

impl BufferLookup for One {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        (buffer == BufferId(1)).then_some(self.0)
    }
}

fn caps(scaling: Option<ScaleLimits>) -> Capabilities {
    let mut c = Capabilities::default();
    c.outputs
        .push(OutputCaps {
            id: OUT,
            size: OUT_SIZE,
            refresh_mhz: 60_000,
        })
        .unwrap();
    let mut p = PlaneCaps {
        id: PlaneId(1),
        kind: PlaneKind::Overlay,
        outputs: 1,
        zpos: Zpos::Fixed(0),
        scaling,
        transforms: Transform::Rotate90.mask(),
        ..PlaneCaps::default()
    };
    p.formats.push(FormatModifier::linear(XRGB)).unwrap();
    c.planes.push(p).unwrap();
    c
}

/// Plans a single layer showing the whole `src` buffer at 0,0 with size `dst`.
fn plans(
    src: (u32, u32),
    dst: (u32, u32),
    transform: Transform,
    scaling: Option<ScaleLimits>,
) -> Result<(), PlanError> {
    let buffers = One(BufferInfo {
        size: Size::new(src.0, src.1),
        format: XRGB,
        modifier: Modifier::LINEAR,
    });
    let mut layer = Layer::new(
        BufferId(1),
        Rect::new(0, 0, src.0, src.1),
        Rect::new(0, 0, dst.0, dst.1),
        0,
    );
    layer.transform = transform;
    plan(
        &Frame {
            output: OUT,
            layers: &[layer],
        },
        &caps(scaling),
        &buffers,
    )
    .map(|_| ())
}

/// Reference in i128: the 16.16 ratio on one axis, compared exactly.
fn axis_ok(s: u32, d: u32, up: u32, down: u32) -> bool {
    let (s, d, up, down) = (s as i128, d as i128, up as i128, down as i128);
    if d > s {
        d * 65536 <= s * up
    } else {
        s * 65536 <= d * down
    }
}

fn reference(src: (u32, u32), dst: (u32, u32), l: ScaleLimits) -> bool {
    if src == dst {
        return true;
    }
    axis_ok(src.0, dst.0, l.max_upscale, l.max_downscale)
        && axis_ok(src.1, dst.1, l.max_upscale, l.max_downscale)
}

fn describe(src: (u32, u32), dst: (u32, u32), t: Transform, l: ScaleLimits) -> String {
    format!("src={src:?} dst={dst:?} transform={t:?} limits={l:?}")
}

#[test]
fn scale_check_matches_i128_reference() {
    let mut rng = Rng(0x5CA1E);
    let edge = [1u32, 2, 3, 0xFFFF, 0x1_0000, 0x1_0001, u32::MAX];
    for _ in 0..20_000 {
        let limits = ScaleLimits {
            // At least 1.0 for both, which any sane backend reports.
            max_upscale: 0x1_0000u32.saturating_add(rng.u32() >> rng.below(24)),
            max_downscale: 0x1_0000u32.saturating_add(rng.u32() >> rng.below(24)),
        };
        let dim = |rng: &mut Rng, max: u32| -> u32 {
            if rng.below(5) == 0 {
                edge[rng.below(edge.len() as u64) as usize]
            } else {
                1 + rng.u32() % max
            }
        };
        // The destination stays inside the output, so no clipping is involved.
        let dst = (dim(&mut rng, 1920).min(1920), dim(&mut rng, 1080).min(1080));
        let src = (dim(&mut rng, 20_000), dim(&mut rng, 20_000));
        let (t, eff_src) = if rng.below(2) == 0 {
            (Transform::Normal, src)
        } else {
            (Transform::Rotate90, (src.1, src.0))
        };
        let got = plans(src, dst, t, Some(limits));
        let want = reference(eff_src, dst, limits);
        let ctx = describe(src, dst, t, limits);
        match got {
            Ok(()) => assert!(want, "accepted but reference rejects: {ctx}"),
            Err(PlanError::NoPlane { reasons, .. }) => {
                assert!(!want, "rejected but reference accepts: {ctx}");
                assert!(
                    reasons.contains(RejectReasons::SCALING),
                    "{ctx}: {reasons:?}"
                );
            }
            Err(e) => panic!("unexpected {e:?}: {ctx}"),
        }
    }
}

#[test]
fn scale_boundaries_are_exact_to_one_ulp() {
    let n = Transform::Normal;
    // 2.0 limit: 100 -> 200 passes, 100 -> 201 fails.
    let l = ScaleLimits {
        max_upscale: 2 << 16,
        max_downscale: 2 << 16,
    };
    assert!(plans((100, 100), (200, 100), n, Some(l)).is_ok());
    assert!(plans((100, 100), (201, 100), n, Some(l)).is_err());
    assert!(plans((200, 100), (100, 100), n, Some(l)).is_ok());
    assert!(plans((201, 100), (100, 100), n, Some(l)).is_err());
    // One ulp below 2.0 no longer admits 2x.
    let l = ScaleLimits {
        max_upscale: (2 << 16) - 1,
        max_downscale: (2 << 16) - 1,
    };
    assert!(plans((100, 100), (200, 100), n, Some(l)).is_err());
    assert!(plans((200, 100), (100, 100), n, Some(l)).is_err());
    // One ulp above 1.0 admits no whole-pixel enlargement on this output.
    let l = ScaleLimits {
        max_upscale: 0x1_0001,
        max_downscale: 0x1_0001,
    };
    assert!(plans((1000, 1), (1001, 1), n, Some(l)).is_err());
    assert!(plans((1000, 1), (1000, 1), n, Some(l)).is_ok());
}

#[test]
fn scale_extreme_values_do_not_overflow() {
    let n = Transform::Normal;
    // u32::MAX limits with u32::MAX sizes must neither panic nor wrap.
    let l = ScaleLimits {
        max_upscale: u32::MAX,
        max_downscale: u32::MAX,
    };
    for src in [1, 2, 0xFFFF, 0x1_0000, u32::MAX - 1, u32::MAX] {
        for dst in [1, 2, 1080, 1920] {
            let got = plans((src, 1), (dst, 1), n, Some(l)).is_ok();
            assert_eq!(got, reference((src, 1), (dst, 1), l), "src={src} dst={dst}");
        }
    }
    let zero = ScaleLimits::default();
    assert!(plans((10, 10), (20, 10), n, Some(zero)).is_err());
    assert!(plans((10, 10), (10, 10), n, Some(zero)).is_ok());
    assert!(plans((10, 10), (10, 10), n, None).is_ok());
    assert!(plans((10, 10), (11, 10), n, None).is_err());
}

#[test]
fn scale_acceptance_is_monotonic() {
    let n = Transform::Normal;
    let mut rng = Rng(0xA0B1);
    for _ in 0..300 {
        let l = ScaleLimits {
            max_upscale: 0x1_0000 + (rng.u32() >> rng.below(24).max(1)),
            max_downscale: 0x1_0000 + (rng.u32() >> rng.below(24).max(1)),
        };
        let s = 1 + rng.below(1500) as u32;
        // Growing destination: once the upscale limit fails it stays failed;
        // the shrinking side (d <= s) only gets easier as d grows.
        let mut seen_up_fail = false;
        let mut prev_ok_down = false;
        for d in 1..=1920u32 {
            let ok = plans((s, 1), (d, 1), n, Some(l)).is_ok();
            let ctx = format!("s={s} d={d} {l:?}");
            if d <= s {
                assert!(!(prev_ok_down && !ok), "downscale not monotonic: {ctx}");
                prev_ok_down = ok;
            } else {
                if seen_up_fail {
                    assert!(!ok, "upscale not monotonic: {ctx}");
                }
                seen_up_fail |= !ok;
            }
        }
        // Raising either limit never turns an accepted layer into a rejected one.
        let d = 1 + rng.below(1920) as u32;
        let base = plans((s, 1), (d, 1), n, Some(l)).is_ok();
        let looser = ScaleLimits {
            max_upscale: l.max_upscale.saturating_add(1 + rng.below(1 << 20) as u32),
            max_downscale: l
                .max_downscale
                .saturating_add(1 + rng.below(1 << 20) as u32),
        };
        let more = plans((s, 1), (d, 1), n, Some(looser)).is_ok();
        assert!(
            !base || more,
            "looser limits rejected: s={s} d={d} {l:?} -> {looser:?}"
        );
    }
}

#[test]
fn scale_limit_is_symmetric_under_rotation() {
    // Rotate90 swaps source axes, so rotated (w,h) equals plain (h,w).
    let mut rng = Rng(77);
    for _ in 0..2000 {
        let l = ScaleLimits {
            max_upscale: 0x1_0000 + (rng.u32() >> 20),
            max_downscale: 0x1_0000 + (rng.u32() >> 20),
        };
        let src = (1 + rng.below(3000) as u32, 1 + rng.below(3000) as u32);
        let dst = (1 + rng.below(1920) as u32, 1 + rng.below(1080) as u32);
        let rot = plans(src, dst, Transform::Rotate90, Some(l)).is_ok();
        let plain = plans((src.1, src.0), dst, Transform::Normal, Some(l)).is_ok();
        assert_eq!(rot, plain, "{}", describe(src, dst, Transform::Rotate90, l));
    }
}

// ------------------------------------------------------------ bug candidates

#[test]
#[ignore = "bug: an unscaled axis is checked against max_downscale, so a limit below 1.0 rejects a layer scaled on the other axis only"]
fn unscaled_axis_must_not_be_limited() {
    // Factor 1 on the height cannot exceed any limit that admits upscaling.
    let l = ScaleLimits {
        max_upscale: 4 << 16,
        max_downscale: 0,
    };
    assert!(plans((100, 100), (200, 100), Transform::Normal, Some(l)).is_ok());
}
