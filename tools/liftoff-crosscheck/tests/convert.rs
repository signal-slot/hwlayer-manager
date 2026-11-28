//! Converts every probe fixture and checks the result against the fixture text.
//! Reads only files inside this crate.

use liftoff_crosscheck::probe::{convert, convert_with, Converted, ErrorKind};
use liftoff_crosscheck::xcheck::{self, OriginKind, PlaneKind, ZposDef};

fn fixture(name: &str) -> String {
    let p = format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

/// `(id, kind, mask)` of the plane lines in the fixture text.
fn text_planes(text: &str) -> Vec<(u32, &str, u32)> {
    text.lines()
        .filter(|l| l.starts_with("plane "))
        .map(|l| {
            let mut it = l.split_whitespace();
            let id = it.nth(1).unwrap().parse().unwrap();
            let kind = it.next().unwrap().trim_end_matches(':');
            let mask = l
                .split("outputs 0b")
                .nth(1)
                .unwrap()
                .split(',')
                .next()
                .unwrap();
            (id, kind, u32::from_str_radix(mask, 2).unwrap())
        })
        .collect()
}

fn check_planes(name: &str, c: &Converted) {
    let text = fixture(name);
    let expect = text_planes(&text);
    let planes = &c.xcheck.planes;
    assert_eq!(planes.len(), expect.len(), "{name}: plane count");
    for (p, (id, kind, mask)) in planes.iter().zip(&expect) {
        assert_eq!(p.id, *id, "{name}");
        let k = match p.kind {
            PlaneKind::Primary => "Primary",
            PlaneKind::Overlay => "Overlay",
            PlaneKind::Cursor => "Cursor",
        };
        assert_eq!(k, *kind, "{name}: plane {id} type");
        assert_eq!(p.outputs, *mask, "{name}: plane {id} CRTC mask");
    }
}

fn round_trip(c: &Converted) {
    let text = xcheck::write(&c.xcheck).expect("writer accepts the converted topology");
    let back = xcheck::read(&text).expect("reader accepts the written text");
    assert_eq!(back, c.xcheck);
}

#[test]
fn rk3588_expands_identical_planes_and_marks_truncation() {
    let c = convert(&fixture("rk3588.probe.txt")).unwrap();
    check_planes("rk3588.probe.txt", &c);
    let x = &c.xcheck;
    assert_eq!(x.origin.kind, OriginKind::Observed);
    assert_eq!(x.origin.date, "2026-11-28");
    assert!(x.origin.text.contains("topology-2026-11-28.md"));
    assert!(x.origin.text.contains("db71077"));
    assert!(x.origin.text.contains("format lists truncated"));
    assert!(!c.formats_incomplete);
    assert_eq!(x.outputs.len(), 1);
    let o = &x.outputs[0];
    assert_eq!(
        (o.id, o.width, o.height, o.refresh_mhz),
        (81, 1920, 1080, 60_000)
    );
    // Cluster planes 33, 39, 45, 51 list 32 formats (the probe's cut); 57.. list 21.
    assert_eq!(c.truncated_planes, [33, 39, 45, 51]);
    for p in &x.planes {
        let want = if c.truncated_planes.contains(&p.id) {
            32
        } else {
            21
        };
        assert_eq!(p.formats.len(), want, "plane {}", p.id);
        assert_eq!(p.zpos, ZposDef::Range { min: 0, max: 7 });
    }
    // zpos_now is the plane index in the dump.
    let now: Vec<_> = x.planes.iter().map(|p| p.zpos_now).collect();
    assert_eq!(now, (0..8).map(Some).collect::<Vec<_>>());
    // Identical planes are copies of plane 33 / plane 57.
    assert_eq!(x.planes[1].formats, x.planes[0].formats);
    assert_eq!(x.planes[3].formats, x.planes[0].formats);
    assert_eq!(x.planes[5].formats, x.planes[4].formats);
    assert_eq!(x.planes[0].formats[14].fourcc, *b"XR30");
    assert_eq!(x.planes[0].formats[14].modifier, 0x800000000000041);
    assert_eq!(x.planes[4].formats[0].modifier, 0);
    round_trip(&c);
}

#[test]
fn stm32mp257_keeps_probe_text_as_printed() {
    let c = convert(&fixture("stm32mp257.probe.txt")).unwrap();
    check_planes("stm32mp257.probe.txt", &c);
    let x = &c.xcheck;
    assert!(c.truncated_planes.is_empty() && !c.formats_incomplete);
    assert!(!x.origin.text.contains("truncated"));
    assert!(x.origin.text.contains("20df182"));
    let lens: Vec<_> = x.planes.iter().map(|p| p.formats.len()).collect();
    assert_eq!(lens, [21, 25, 17]);
    // `C8??` kept, trailing space on the last line harmless.
    assert!(x.planes.iter().all(|p| p.formats[16].fourcc == *b"C8??"));
    assert_eq!(x.planes[0].zpos, ZposDef::Range { min: 0, max: 2 });
    assert_eq!(x.planes[2].zpos_now, Some(2));
    assert!(x.planes.iter().all(|p| p.blend.0 == 2 && p.alpha));
    round_trip(&c);
}

#[test]
fn bcm_dumps_are_refused_without_formats() {
    for name in ["bcm2711-cm4.probe.txt", "bcm2712.probe.txt"] {
        let e = convert(&fixture(name)).unwrap_err();
        match e.kind {
            ErrorKind::FormatsMissing { planes } => assert_eq!(planes.len(), 16, "{name}"),
            k => panic!("{name}: {k:?}"),
        }
    }
}

#[test]
fn bcm_dumps_flagged_when_allowed_never_invent_formats() {
    for name in ["bcm2711-cm4.probe.txt", "bcm2712.probe.txt"] {
        let c = convert_with(&fixture(name), true).unwrap();
        check_planes(name, &c);
        assert!(c.formats_incomplete, "{name}");
        assert_eq!(c.planes_without_formats.len(), 16);
        assert!(c.truncated_planes.is_empty());
        assert!(
            c.xcheck.origin.text.contains("formats incomplete"),
            "{name}"
        );
        assert!(c.xcheck.planes.iter().all(|p| p.formats.is_empty()));
        // Fixed primaries and 1..=17 overlays as printed.
        for p in &c.xcheck.planes {
            match p.kind {
                PlaneKind::Primary => assert_eq!(p.zpos, ZposDef::Fixed(0)),
                _ => assert_eq!(p.zpos, ZposDef::Range { min: 1, max: 17 }),
            }
        }
        round_trip(&c);
    }
    let c = convert_with(&fixture("bcm2711-cm4.probe.txt"), true).unwrap();
    assert_eq!(
        c.xcheck.planes.iter().filter(|p| p.outputs == 1).count(),
        11
    );
    assert_eq!(c.xcheck.outputs[0].id, 90);
    assert_eq!(c.xcheck.outputs[0].height, 1280);
}

#[test]
fn a_real_formats_line_is_kept_but_others_stay_missing() {
    let text =
        fixture("bcm2711-cm4.probe.txt").replacen("plane 67", "  formats (1): XR24\nplane 67", 1);
    let c = convert_with(&text, true).unwrap();
    assert_eq!(c.planes_without_formats.len(), 15);
    assert_eq!(c.xcheck.planes[0].formats.len(), 1);
    assert!(convert(&text).is_err());
}

#[test]
fn header_lines_are_skipped_and_provenance_is_required() {
    let text = fixture("stm32mp257.probe.txt");
    assert!(text.lines().take(6).all(|l| l.starts_with("# ")));
    let body: String = text
        .lines()
        .filter(|l| !l.starts_with("# "))
        .map(|l| format!("{l}\n"))
        .collect();
    let e = convert(&body).unwrap_err();
    assert!(matches!(e.kind, ErrorKind::MissingProvenance(_)), "{e}");
}

#[test]
fn malformed_text_is_a_typed_error() {
    let head = "# source: x\n# date: 2026-11-28\n# origin: synthetic\n";
    let e = convert(&format!(
        "{head}output 0: crtc 1 8x8 @ 60.000 Hz\nplane 1 Primary: nonsense\n"
    ))
    .unwrap_err();
    assert!(matches!(e.kind, ErrorKind::Syntax(_)));
    assert_eq!(e.line, 5);
    let e = convert(&format!(
        "{head}output 0: crtc 1 8x8 @ 60.000 Hz\nplane 1 Primary: outputs 0b1, zpos fixed 0, plane alpha true, blend [none], scaling not claimed\n  formats (2): XR24\n"
    ))
    .unwrap_err();
    assert!(matches!(e.kind, ErrorKind::Syntax(_)));
}

/// The synthetic i.MX95 stream (six planes, ARGB8888 and NV12) rendered as
/// probe text converts to the planes and output of the committed
/// `imx95-synthetic.xcheck`.
#[test]
fn synthetic_imx95_stream_matches_the_committed_xcheck() {
    let mut text = String::from(
        "# source: synthetic imx95 stream, test fn dpu95_stream\n# date: 2026-11-28\n# origin: synthetic\n",
    );
    text.push_str("output 0: crtc 1 1920x1080 @ 60.000 Hz\n");
    for i in 0..6 {
        let kind = if i == 0 { "Primary" } else { "Overlay" };
        text.push_str(&format!(
            "plane {} {kind}: outputs 0b1, zpos 0..=5, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed\n  formats (2): AR24 NV12\n",
            40 + i
        ));
    }
    let c = convert(&text).unwrap();
    assert_eq!(c.xcheck.origin.kind, OriginKind::Synthetic);
    assert!(c.xcheck.origin.text.contains("no hardware claim"));
    round_trip(&c);
    let committed = xcheck::read(&fixture("imx95-synthetic.xcheck")).unwrap();
    assert_eq!(committed.origin.kind, OriginKind::Synthetic);
    assert_eq!(c.xcheck.planes, committed.planes);
    assert_eq!(c.xcheck.outputs[0].width, committed.outputs[0].width);
    assert_eq!(
        c.xcheck.outputs[0].refresh_mhz,
        committed.outputs[0].refresh_mhz
    );
}
