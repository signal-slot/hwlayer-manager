//! The `MAX_FORMATS` cap applied to the full RK3588 `IN_FORMATS` lists.
//!
//! No public API of `composition-drm` parses a blob or fills a plane's
//! format list: that happens in the private `parse_in_formats` and in
//! `DrmBackend::from_fd` (src/lib.rs), which sorts the pairs stably so linear
//! comes first and pushes the first `MAX_FORMATS` into `PlaneCaps::formats`.
//! So `cap` below restates that rule on the public core types
//! (`PlaneCaps`, `FixedVec`, `composition_core::limits::MAX_FORMATS`). If the
//! backend changes the rule this test does not notice until `cap` is updated.
//!
//! Fixture sources (observed = measured on the NanoPC-T6, 2026-11-28):
//! - Cluster: 14 formats x 9 AFBC modifiers = 126 pairs, no linear, blob order
//!   modifier-major, from docs/rk3588-vop2-session-2026-11-28.md (observed).
//!   The nine modifiers also match `format_modifiers_afbc` in
//!   docs/rk3588-vop2-formats-source.md (read from source).
//! - Esmart: 21 linear pairs, same session doc (observed).
//!
//! This documents the owner's `MAX_FORMATS` finding; it changes nothing.

use composition_core::limits::MAX_FORMATS;
use composition_core::{FormatModifier, Modifier, PixelFormat, PlaneCaps};

const CLUSTER_FORMATS: [&str; 14] = [
    "XR30", "XB30", "XR24", "AR24", "XB24", "AB24", "RG24", "BG24", "RG16", "BG16", "YU08", "YU10",
    "YUYV", "Y210",
];

/// The nine Cluster modifiers in the order the kernel lists them.
const CLUSTER_MODIFIERS: [u64; 9] = [
    0x0800_0000_0000_0001, // 16x16
    0x0800_0000_0000_0041, // 16x16|SPARSE
    0x0800_0000_0000_0011, // 16x16|YTR
    0x0800_0000_0000_0081, // 16x16|CBR
    0x0800_0000_0000_0051, // 16x16|YTR|SPARSE
    0x0800_0000_0000_00c1, // 16x16|SPARSE|CBR
    0x0800_0000_0000_0091, // 16x16|YTR|CBR
    0x0800_0000_0000_00d1, // 16x16|YTR|SPARSE|CBR
    0x0800_0000_0000_0071, // 16x16|YTR|SPLIT|SPARSE
];

const ESMART_FORMATS: [&str; 21] = [
    "XR24", "AR24", "XB24", "AB24", "RG24", "BG24", "RG16", "BG16", "NV12", "NV21", "NV16", "NV61",
    "NV20", "NV24", "NV42", "NV30", "NV15", "YVYU", "VYUY", "YUYV", "UYVY",
];

fn fourcc(name: &str) -> PixelFormat {
    let b: [u8; 4] = name.as_bytes().try_into().expect("four characters");
    PixelFormat::fourcc(&b)
}

/// Blob order: modifier-major, every format under every modifier.
fn cluster_blob() -> Vec<FormatModifier> {
    CLUSTER_MODIFIERS
        .iter()
        .flat_map(|m| {
            CLUSTER_FORMATS.iter().map(move |f| FormatModifier {
                format: fourcc(f),
                modifier: Modifier(*m),
            })
        })
        .collect()
}

fn esmart_blob() -> Vec<FormatModifier> {
    ESMART_FORMATS
        .iter()
        .map(|f| FormatModifier::linear(fourcc(f)))
        .collect()
}

/// The backend's rule: stable sort, linear first, keep `MAX_FORMATS`.
fn cap(mut pairs: Vec<FormatModifier>) -> Vec<FormatModifier> {
    pairs.sort_by_key(|p| p.modifier != Modifier::LINEAR);
    let mut caps = PlaneCaps::default();
    for pair in pairs.into_iter().take(MAX_FORMATS) {
        let _ = caps.formats.push(pair);
    }
    caps.formats.as_slice().to_vec()
}

#[test]
fn fixtures_match_the_recorded_counts() {
    assert_eq!(cluster_blob().len(), 126);
    assert_eq!(esmart_blob().len(), 21);
    assert_eq!(MAX_FORMATS, 32);
}

#[test]
fn cluster_keeps_32_of_126_and_has_no_linear() {
    let kept = cap(cluster_blob());
    assert_eq!(kept.len(), MAX_FORMATS);
    // Linear is not listed, so none is first and none is hidden.
    assert!(kept.iter().all(|p| p.modifier != Modifier::LINEAR));
    assert!(cluster_blob()
        .iter()
        .all(|p| p.modifier != Modifier::LINEAR));
    assert_eq!(126 - kept.len(), 94);
}

#[test]
fn cluster_cut_keeps_blob_prefix_and_hides_the_afbc_variants() {
    let blob = cluster_blob();
    let kept = cap(blob.clone());
    // Stable sort with no linear pair: the kept pairs are the blob prefix.
    assert_eq!(kept.as_slice(), &blob[..MAX_FORMATS]);
    let count = |m: u64| kept.iter().filter(|p| p.modifier == Modifier(m)).count();
    assert_eq!(count(CLUSTER_MODIFIERS[0]), 14);
    assert_eq!(count(CLUSTER_MODIFIERS[1]), 14);
    assert_eq!(count(CLUSTER_MODIFIERS[2]), 4);
    // The four kept YTR pairs are XR30 XB30 XR24 AR24; the last kept is AR24.
    assert_eq!(kept.last().unwrap().format, fourcc("AR24"));
    // The six later modifiers are hidden entirely.
    for m in &CLUSTER_MODIFIERS[3..] {
        assert_eq!(count(*m), 0, "{m:#x}");
    }
    // Every format still has its plain AFBC(16x16) pair.
    for f in CLUSTER_FORMATS {
        assert!(kept
            .iter()
            .any(|p| p.format == fourcc(f) && p.modifier == Modifier(CLUSTER_MODIFIERS[0])));
    }
}

#[test]
fn esmart_is_untouched_by_the_cap() {
    let blob = esmart_blob();
    let kept = cap(blob.clone());
    assert_eq!(kept, blob);
    assert_eq!(kept.len(), 21);
    assert!(kept.iter().all(|p| p.modifier == Modifier::LINEAR));
    assert!(kept.len() <= MAX_FORMATS);
}

#[test]
fn linear_would_come_first_if_a_plane_listed_it() {
    // Synthetic: a Cluster list with linear appended last still sorts it first.
    let mut blob = cluster_blob();
    blob.push(FormatModifier::linear(fourcc("XR24")));
    let kept = cap(blob);
    assert_eq!(kept.len(), MAX_FORMATS);
    assert_eq!(kept[0], FormatModifier::linear(fourcc("XR24")));
    assert!(kept[1..].iter().all(|p| p.modifier != Modifier::LINEAR));
}
