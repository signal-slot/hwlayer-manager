//! Pins what the `MAX_FORMATS` cut does to the vc4 HVS5/HVS6 plane format
//! lists, and that the HVS6 modifier filter of `DriverProfile::Vc4Hvs6`
//! therefore never sees a T-tiled or SAND pair on a live backend
//! (`docs/vc4-formats-source.md` section 4, findings 1 and 2).
//!
//! Data and tags:
//! - BCM2711 (HVS5, reTerminal CM4) and BCM2712 (HVS6, Pi 5) "recorded" lists
//!   are the `IN_FORMATS` blobs of `crates/composition-drm/tests/data/
//!   bcm2711/props-2026-11-27.txt` and `.../bcm2712/props-2026-11-27.txt`
//!   (observed 2026-11-27; the same text is in docs/observed/bcm2712-topology.md).
//! - The "pinned source" HVS6 list is read from source: `vc4_plane.c` at
//!   raspberrypi/linux rpi-6.18.y 16ae2649 as counted in
//!   `docs/vc4-formats-source.md` section 3.2 (no T_TILED, no SAND64).
//! - The cut itself is private to `DrmBackend::from_fd` (src/lib.rs: stable
//!   sort with linear first, then `take(MAX_FORMATS)`), so `cut` below restates
//!   it on the real core types, as `format_cut_recorded.rs` does. If the
//!   backend changes the rule, update `cut`.
//!
//! This test records today's behaviour. It changes nothing; if `MAX_FORMATS`
//! is raised on purpose, these assertions are expected to change.

use composition_core::limits::MAX_FORMATS;
use composition_core::{
    Capabilities, FormatModifier, Modifier, OutputCaps, OutputId, PixelFormat, PlaneCaps, PlaneId,
    PlaneKind, Size,
};
use composition_drm::DriverProfile;

const T_TILED: Modifier = Modifier(0x0700_0000_0000_0001);
const SAND64: Modifier = Modifier(0x0700_0000_0000_0003);
const SAND128: Modifier = Modifier(0x0700_0000_0000_0004);
const SAND256: Modifier = Modifier(0x0700_0000_0000_0005);

const RGB_TILED: &str = "XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15";
const NV: &str = "NV12 NV21";

const LINEAR_32: &str = "XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15 RG24 BG24 YU16 YV16 YU24 YV24 \
YU12 YV12 NV12 NV21 NV16 NV61 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12 XB12 AB12 BX12 BA12";

/// BCM2711 recorded: 35 formats; the linear row is 34 (no RX24 RA24 BX24 BA24).
const BCM2711_LINEAR: &str = "XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15 RG24 BG24 YU16 YV16 YU24 \
YV24 YU12 YV12 NV12 NV21 NV16 NV61 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12 XB12 AB12 BX12 BA12 \
RX12 RA12";

/// BCM2712 recorded: 45 formats; the linear row is 44 (P030 is SAND128 only).
const BCM2712_LINEAR: &str = "XR24 AR24 AB24 XB24 RX24 RA24 BX24 BA24 RG16 BG16 AR15 XR15 RG24 \
BG24 YU16 YV16 YU24 YV24 YU12 YV12 NV12 NV21 NV16 NV61 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12 \
XB12 AB12 BX12 BA12 RX12 RA12 P010 P012 P016 S010 S012 S016";

/// Linear pairs the cut removes on the recorded BCM2712 list (positions 33-44).
const BCM2712_LINEAR_CUT: &str = "XB12 AB12 BX12 BA12 RX12 RA12 P010 P012 P016 S010 S012 S016";

fn fourcc(name: &str) -> PixelFormat {
    let b: [u8; 4] = name.as_bytes().try_into().expect("four characters");
    PixelFormat::fourcc(&b)
}

/// The blob rows in the recorded order: T_TILED, SAND128, SAND64, SAND256,
/// linear. `tiled`/`sand64` are empty for the pinned HVS6 source.
fn blob(
    linear: &str,
    tiled: &str,
    sand128: &str,
    sand64: &str,
    sand256: &str,
) -> Vec<FormatModifier> {
    let rows = [
        (T_TILED, tiled),
        (SAND128, sand128),
        (SAND64, sand64),
        (SAND256, sand256),
        (Modifier::LINEAR, linear),
    ];
    rows.iter()
        .flat_map(|(modifier, names)| {
            names.split_whitespace().map(|n| FormatModifier {
                format: fourcc(n),
                modifier: *modifier,
            })
        })
        .collect()
}

fn recorded_bcm2711() -> Vec<FormatModifier> {
    blob(BCM2711_LINEAR, RGB_TILED, "NV12 NV21 P030", NV, NV)
}

fn recorded_bcm2712() -> Vec<FormatModifier> {
    blob(BCM2712_LINEAR, RGB_TILED, "NV12 NV21 P030", NV, NV)
}

fn source_hvs6() -> Vec<FormatModifier> {
    blob(BCM2712_LINEAR, "", "NV12 NV21 P030", "", NV)
}

/// `DrmBackend::from_fd`'s cut: stable sort linear first, keep `MAX_FORMATS`.
fn cut(mut pairs: Vec<FormatModifier>) -> Vec<FormatModifier> {
    pairs.sort_by_key(|p| p.modifier != Modifier::LINEAR);
    let mut caps = PlaneCaps::default();
    for pair in pairs.into_iter().take(MAX_FORMATS) {
        caps.formats.push(pair).unwrap();
    }
    caps.formats.as_slice().to_vec()
}

fn count(list: &[FormatModifier], modifier: Modifier) -> usize {
    list.iter().filter(|p| p.modifier == modifier).count()
}

fn nonlinear(list: &[FormatModifier]) -> usize {
    list.iter()
        .filter(|p| p.modifier != Modifier::LINEAR)
        .count()
}

fn names(list: &[FormatModifier]) -> Vec<PixelFormat> {
    list.iter().map(|p| p.format).collect()
}

fn expect_names(s: &str) -> Vec<PixelFormat> {
    s.split_whitespace().map(fourcc).collect()
}

fn linear_dropped(full: &[FormatModifier], kept: &[FormatModifier]) -> Vec<PixelFormat> {
    full.iter()
        .filter(|p| p.modifier == Modifier::LINEAR && !kept.contains(p))
        .map(|p| p.format)
        .collect()
}

fn caps_with(formats: &[FormatModifier]) -> Capabilities {
    let mut caps = Capabilities::default();
    caps.outputs
        .push(OutputCaps {
            id: OutputId(0),
            size: Size::new(1920, 1080),
            refresh_mhz: 60_000,
        })
        .unwrap();
    let mut plane = PlaneCaps {
        id: PlaneId(1),
        kind: PlaneKind::Overlay,
        outputs: 1,
        ..PlaneCaps::default()
    };
    for p in formats {
        plane.formats.push(*p).unwrap();
    }
    caps.planes.push(plane).unwrap();
    caps
}

#[test]
fn cap_is_32() {
    assert_eq!(MAX_FORMATS, 32);
}

#[test]
fn recorded_bcm2711_hvs5_totals() {
    let full = recorded_bcm2711();
    assert_eq!(full.len(), 49);
    assert_eq!(count(&full, Modifier::LINEAR), 34);
    assert_eq!(nonlinear(&full), 15);
    let kept = cut(full.clone());
    assert_eq!(kept.len(), 32);
    // All 15 tiled/SAND pairs are cut; the linear list is its first 32.
    assert_eq!(nonlinear(&kept), 0);
    for m in [T_TILED, SAND64, SAND128, SAND256] {
        assert_eq!(count(&kept, m), 0);
    }
    assert_eq!(names(&kept), expect_names(LINEAR_32));
    assert_eq!(
        linear_dropped(&full, &kept),
        expect_names("RX12 RA12"),
        "only the last two linear pairs are lost"
    );
}

#[test]
fn recorded_bcm2712_hvs6_totals() {
    let full = recorded_bcm2712();
    assert_eq!(full.len(), 59);
    assert_eq!(count(&full, Modifier::LINEAR), 44);
    assert_eq!(count(&full, T_TILED), 8);
    assert_eq!(count(&full, SAND64), 2);
    assert_eq!(count(&full, SAND128), 3);
    assert_eq!(count(&full, SAND256), 2);
    let kept = cut(full.clone());
    assert_eq!(kept.len(), 32);
    assert_eq!(nonlinear(&kept), 0);
    assert_eq!(names(&kept), names(&full[full.len() - 44..][..MAX_FORMATS]));
    assert_eq!(kept[4].format, fourcc("RX24"));
    // 59 - 32 = 27 cut: 12 linear (incl. P010 P012 P016 S010 S012 S016) + 15.
    assert_eq!(full.len() - kept.len(), 27);
    assert_eq!(
        linear_dropped(&full, &kept),
        expect_names(BCM2712_LINEAR_CUT)
    );
}

#[test]
fn pinned_source_hvs6_totals() {
    let full = source_hvs6();
    assert_eq!(full.len(), 49);
    assert_eq!(count(&full, Modifier::LINEAR), 44);
    assert_eq!(nonlinear(&full), 5);
    let kept = cut(full.clone());
    assert_eq!(kept.len(), 32);
    // 17 cut: 12 linear and the 5 SAND pairs.
    assert_eq!(full.len() - kept.len(), 17);
    assert_eq!(nonlinear(&kept), 0);
    assert_eq!(
        linear_dropped(&full, &kept),
        expect_names(BCM2712_LINEAR_CUT)
    );
}

#[test]
fn hvs6_filter_sees_nothing_after_the_cut() {
    for full in [recorded_bcm2712(), source_hvs6()] {
        let kept = cut(full);
        let mut caps = caps_with(&kept);
        DriverProfile::Vc4Hvs6.apply(&mut caps);
        // No T-tiled or SAND64 pair was left to remove: the list is unchanged.
        assert_eq!(caps.planes[0].formats.as_slice(), kept.as_slice());
    }
}

#[test]
fn hvs6_filter_would_matter_without_the_cut() {
    // Control: on the uncut recorded list the filter does remove pairs. Only
    // possible here by building the caps directly; the live backend cannot.
    let full = recorded_bcm2712();
    let mut plane = PlaneCaps::default();
    for p in full.iter().take(MAX_FORMATS) {
        plane.formats.push(*p).unwrap();
    }
    // Put tiled and SAND64 pairs inside the 32 slots by hand.
    let hand: Vec<FormatModifier> = full
        .iter()
        .filter(|p| p.modifier == T_TILED || p.modifier == SAND64)
        .copied()
        .collect();
    assert_eq!(hand.len(), 10);
    let mut caps = caps_with(&hand);
    DriverProfile::Vc4Hvs6.apply(&mut caps);
    assert!(caps.planes[0].formats.as_slice().is_empty());
}
