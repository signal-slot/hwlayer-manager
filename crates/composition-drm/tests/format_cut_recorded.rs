//! Pins the `MAX_FORMATS` cut on recorded `IN_FORMATS` lists.
//!
//! `DrmBackend::from_fd` parses a plane's `IN_FORMATS` blob, sorts the pairs
//! stably so linear layouts come first, and keeps the first `MAX_FORMATS`
//! in a `PlaneCaps`. Both the parser and the cut are private to the crate
//! and only run against a live device, so `cut` below restates the rule
//! (src/lib.rs, `from_fd`) on the real core types: the same stable sort and
//! `take(MAX_FORMATS)` into `PlaneCaps::formats`. If the backend changes the
//! rule, these tests do not notice until `cut` is updated; a public hook for
//! the parse and the cut would close that gap.
//!
//! The lists are copied from recorded board data; nothing reads docs/
//! at runtime. Tags: observed = measured on the board, source = read from the
//! kernel source, synthetic = made up for the test.

use composition_core::limits::MAX_FORMATS;
use composition_core::{FormatModifier, Modifier, PixelFormat, PlaneCaps};

type Groups = Vec<(Modifier, &'static [&'static str])>;

const AFBC_1: Modifier = Modifier(0x0800_0000_0000_0001);
const AFBC_41: Modifier = Modifier(0x0800_0000_0000_0041);
const AFBC_11: Modifier = Modifier(0x0800_0000_0000_0011);
const BRCM_T_TILED: Modifier = Modifier(0x0700_0000_0000_0001);
const BRCM_3: Modifier = Modifier(0x0700_0000_0000_0003);
const BRCM_4: Modifier = Modifier(0x0700_0000_0000_0004);
const BRCM_5: Modifier = Modifier(0x0700_0000_0000_0005);

const XR24: PixelFormat = PixelFormat::XRGB8888;
const AR24: PixelFormat = PixelFormat::fourcc(b"AR24");

fn fourcc(name: &str) -> PixelFormat {
    let b: [u8; 4] = name.as_bytes().try_into().expect("four characters");
    PixelFormat::fourcc(&b)
}

fn pairs(modifier: Modifier, names: &[&str]) -> Vec<FormatModifier> {
    names
        .iter()
        .map(|n| FormatModifier {
            format: fourcc(n),
            modifier,
        })
        .collect()
}

/// Expands `(modifier, formats)` groups in blob order and applies the cut.
fn cut(groups: &[(Modifier, &[&str])]) -> Vec<FormatModifier> {
    let mut all: Vec<FormatModifier> = groups
        .iter()
        .flat_map(|(modifier, names)| pairs(*modifier, names))
        .collect();
    all.sort_by_key(|p| p.modifier != Modifier::LINEAR);
    let mut caps = PlaneCaps::default();
    for pair in all.into_iter().take(MAX_FORMATS) {
        caps.formats.push(pair).unwrap();
    }
    caps.formats.as_slice().to_vec()
}

fn has(list: &[FormatModifier], format: PixelFormat, modifier: Modifier) -> bool {
    list.iter()
        .any(|p| p.format == format && p.modifier == modifier)
}

/// NanoPC-T6 Cluster plane 33 (same on the other Cluster planes): the 32
/// pairs the probe printed, in order. The kernel list is longer; the probe
/// shows the post-cut view only. Source: tests/data/rk3588/topology-2026-11-28.md,
/// observed 2026-11-28 (kernel 6.18.55-current-rockchip64).
const CLUSTER_AFBC_FULL: &[&str] = &[
    "XR30", "XB30", "XR24", "AR24", "XB24", "AB24", "RG24", "BG24", "RG16", "BG16", "YU08", "YU10",
    "YUYV", "Y210",
];
const CLUSTER_AFBC_11_SHOWN: &[&str] = &["XR30", "XB30", "XR24", "AR24"];

fn cluster_groups() -> Groups {
    vec![
        (AFBC_1, CLUSTER_AFBC_FULL),
        (AFBC_41, CLUSTER_AFBC_FULL),
        (AFBC_11, CLUSTER_AFBC_11_SHOWN),
    ]
}

/// NanoPC-T6 Esmart plane 57 (same on the other Esmart planes): 21 linear
/// pairs. Source: tests/data/rk3588/topology-2026-11-28.md, observed 2026-11-28.
const ESMART: &[&str] = &[
    "XR24", "AR24", "XB24", "AB24", "RG24", "BG24", "RG16", "BG16", "NV12", "NV21", "NV16", "NV61",
    "NV20", "NV24", "NV42", "NV30", "NV15", "YVYU", "VYUY", "YUYV", "UYVY",
];

/// BCM2712 (Pi 5) full `IN_FORMATS`, identical on the five planes read.
/// Source: docs/platform-capabilities.md ("IN_FORMATS"), observed
/// 2026-11-28. The first 32 linear entries are the probe's list.
const BCM2712_LINEAR: &[&str] = &[
    "XR24", "AR24", "AB24", "XB24", "RX24", "RA24", "BX24", "BA24", "RG16", "BG16", "AR15", "XR15",
    "RG24", "BG24", "YU16", "YV16", "YU24", "YV24", "YU12", "YV12", "NV12", "NV21", "NV16", "NV61",
    "XR30", "AR30", "AB30", "XB30", "RGB8", "BGR8", "XR12", "AR12", // 32nd
    "XB12", "AB12", "BX12", "BA12", "RX12", "RA12", "P010", "P012", "P016", "S010", "S012", "S016",
];
const BCM_TILED: &[&str] = &[
    "XR24", "AR24", "AB24", "XB24", "RG16", "BG16", "AR15", "XR15",
];
const BCM_NV: &[&str] = &["NV12", "NV21"];
const BCM_NV_P030: &[&str] = &["NV12", "NV21", "P030"];

fn bcm2712_groups() -> Groups {
    vec![
        (Modifier::LINEAR, BCM2712_LINEAR),
        (BRCM_T_TILED, BCM_TILED),
        (BRCM_3, BCM_NV),
        (BRCM_4, BCM_NV_P030),
        (BRCM_5, BCM_NV),
    ]
}

/// BCM2711 (CM4): the raw blob was not recorded. The printed first 32
/// linear pairs are observed (docs/platform-capabilities.md, plane 48,
/// 2026-11-28); they lack the RX24/RA24/BX24/BA24 entries of the BCM2712
/// list. The linear tail RX12/RA12 (cut) and the five modifier groups are
/// inferred from docs/platform-capabilities.md, which says BCM2712 is the
/// BCM2711 list plus P010/P012/P016/S010/S012/S016.
const BCM2711_PROBE_32: &str = "XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15 RG24 BG24 YU16 YV16 YU24 \
     YV24 YU12 YV12 NV12 NV21 NV16 NV61 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12 XB12 AB12 BX12 BA12";
const BCM2711_LINEAR: &[&str] = &[
    "XR24", "AR24", "AB24", "XB24", "RG16", "BG16", "AR15", "XR15", "RG24", "BG24", "YU16", "YV16",
    "YU24", "YV24", "YU12", "YV12", "NV12", "NV21", "NV16", "NV61", "XR30", "AR30", "AB30", "XB30",
    "RGB8", "BGR8", "XR12", "AR12", "XB12", "AB12", "BX12", "BA12", // 32nd
    "RX12", "RA12",
];

fn bcm2711_groups() -> Groups {
    vec![
        (Modifier::LINEAR, BCM2711_LINEAR),
        (BRCM_T_TILED, BCM_TILED),
        (BRCM_3, BCM_NV),
        (BRCM_4, BCM_NV_P030),
        (BRCM_5, BCM_NV),
    ]
}

#[test]
fn bcm2712_keeps_first_32_linear_pairs_and_cuts_the_rest() {
    let kept = cut(&bcm2712_groups());
    assert_eq!(kept.len(), MAX_FORMATS);
    assert_eq!(kept, pairs(Modifier::LINEAR, &BCM2712_LINEAR[..32]));
    assert!(has(&kept, XR24, Modifier::LINEAR));
    assert!(has(&kept, AR24, Modifier::LINEAR));
}

#[test]
fn bcm2712_cut_drops_every_broadcom_modifier_and_the_p01x_formats() {
    let kept = cut(&bcm2712_groups());
    // Pinned as of today: linear fills all 32 slots, so T-tiled and the three
    // SAND-type modifiers (and P030) never reach the planner. The kernel lists
    // 44 linear + 8 + 2 + 3 + 2 = 59 pairs.
    assert!(kept.iter().all(|p| p.modifier == Modifier::LINEAR));
    for m in [BRCM_T_TILED, BRCM_3, BRCM_4, BRCM_5] {
        assert!(!kept.iter().any(|p| p.modifier == m), "{m:?} survived");
    }
    assert!(!kept.iter().any(|p| p.format == fourcc("P030")));
    let dropped_linear = &BCM2712_LINEAR[32..];
    assert_eq!(dropped_linear.len(), 12);
    for name in dropped_linear {
        assert!(!has(&kept, fourcc(name), Modifier::LINEAR), "{name}");
    }
}

#[test]
fn bcm2711_survivors_equal_the_probe_list() {
    let kept = cut(&bcm2711_groups());
    let names: Vec<&str> = BCM2711_PROBE_32.split_whitespace().collect();
    assert_eq!(names.len(), MAX_FORMATS);
    assert_eq!(names[..], BCM2711_LINEAR[..32]);
    assert_eq!(kept, pairs(Modifier::LINEAR, &names));
    assert!(has(&kept, XR24, Modifier::LINEAR));
    assert!(has(&kept, AR24, Modifier::LINEAR));
    // Same cut as BCM2712: no T-tiled or SAND pair survives.
    assert!(kept.iter().all(|p| p.modifier == Modifier::LINEAR));
}

#[test]
fn esmart_plane_fits_under_the_cut_untouched() {
    let kept = cut(&[(Modifier::LINEAR, ESMART)]);
    assert_eq!(kept, pairs(Modifier::LINEAR, ESMART));
    assert!(has(&kept, XR24, Modifier::LINEAR));
    assert!(has(&kept, AR24, Modifier::LINEAR));
}

#[test]
fn cluster_plane_keeps_afbc_pairs_and_lists_no_linear() {
    let kept = cut(&cluster_groups());
    assert_eq!(kept.len(), MAX_FORMATS);
    for m in [AFBC_1, AFBC_41, AFBC_11] {
        assert!(has(&kept, XR24, m), "XR24 {m:?}");
        assert!(has(&kept, AR24, m), "AR24 {m:?}");
    }
    // Observed: the probe's 32 pairs hold no linear pair, and linear sorts
    // first, so the Cluster plane's IN_FORMATS blob listed none. Linear
    // XRGB8888/ARGB8888 therefore do NOT survive on Cluster planes; "linear
    // always survives" holds only where the plane lists linear.
    assert!(!has(&kept, XR24, Modifier::LINEAR));
    assert!(!has(&kept, AR24, Modifier::LINEAR));
    // The recorded view is already post-cut; cutting it again changes nothing.
    assert_eq!(kept, cut(&cluster_groups()));
}

#[test]
fn cluster_with_a_hypothetical_linear_group_would_evict_afbc_pairs() {
    // Synthetic: if a Cluster plane did list linear XR24/AR24 (fbcon scans out
    // linear XR24 from Cluster0-win0, observed 2026-11-28), linear moves to
    // the front and the last AFBC pairs fall off the end. Pinned so a change
    // of the sort shows up.
    let mut groups: Groups = vec![(Modifier::LINEAR, &["XR24", "AR24"])];
    groups.extend(cluster_groups());
    let kept = cut(&groups);
    assert_eq!(kept.len(), MAX_FORMATS);
    assert_eq!(kept[..2], pairs(Modifier::LINEAR, &["XR24", "AR24"])[..]);
    assert!(!has(&kept, AR24, AFBC_11));
    assert!(has(&kept, XR24, AFBC_1));
}

#[test]
fn the_cut_is_deterministic() {
    let sets: [(&str, Groups); 4] = [
        ("cluster", cluster_groups()),
        ("esmart", vec![(Modifier::LINEAR, ESMART)]),
        ("bcm2711", bcm2711_groups()),
        ("bcm2712", bcm2712_groups()),
    ];
    for (name, groups) in &sets {
        let first = cut(groups);
        for _ in 0..8 {
            assert_eq!(first, cut(groups), "{name}");
        }
        assert!(first.len() <= MAX_FORMATS, "{name}");
        for (i, a) in first.iter().enumerate() {
            assert!(!first[i + 1..].contains(a), "{name}: duplicate {a:?}");
        }
    }
}
