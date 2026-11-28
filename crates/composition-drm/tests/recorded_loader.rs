//! The loader of recorded probe text (`tests/common/recorded.rs`).
//!
//! The samples below are copies of board output; each names its source file
//! and date. Nothing here reads `docs/`.

mod common;

use common::recorded::{
    load, load_with, modifier_name, vendor_modifier, LoadError, Overflow, SAND128, SAND256, SAND64,
    VC4_T_TILED, VENDOR_ARM, VENDOR_BROADCOM,
};
use composition_core::limits::{MAX_FORMATS, MAX_PLANES};
use composition_core::{
    BlendMode, Capabilities, FormatModifier, Modifier, OutputCaps, OutputId, PixelFormat,
    PlaneCaps, PlaneId, PlaneKind, ScaleLimits, Size, Zpos,
};

const ALL_BLEND: u8 = 0b111;

fn fm(code: &[u8; 4], modifier: u64) -> FormatModifier {
    FormatModifier {
        format: PixelFormat::fourcc(code),
        modifier: Modifier(modifier),
    }
}

fn plane(caps: &Capabilities, id: u32) -> &PlaneCaps {
    caps.planes.iter().find(|p| p.id.0 == id).unwrap()
}

// Source: tests/data/rk3588/topology-2026-11-28.md, "Raw probe output", observed
// 2026-11-28 (NanoPC-T6, kernel 6.18.55-current-rockchip64). The `(identical
// to plane 33)` form is the doc's own shortening of the repeated lists.
const RK3588_PROBE: &str = r#"
device /dev/dri/card0, driver rockchip
output 0: crtc 81 1920x1080 @ 60.000 Hz
plane 33 Primary: outputs 0b1, zpos 0..=7, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
  formats (32): XR30:0x800000000000001 XB30:0x800000000000001 XR24:0x800000000000001 AR24:0x800000000000001 XB24:0x800000000000001 AB24:0x800000000000001 RG24:0x800000000000001 BG24:0x800000000000001 RG16:0x800000000000001 BG16:0x800000000000001 YU08:0x800000000000001 YU10:0x800000000000001 YUYV:0x800000000000001 Y210:0x800000000000001 XR30:0x800000000000041 XB30:0x800000000000041 XR24:0x800000000000041 AR24:0x800000000000041 XB24:0x800000000000041 AB24:0x800000000000041 RG24:0x800000000000041 BG24:0x800000000000041 RG16:0x800000000000041 BG16:0x800000000000041 YU08:0x800000000000041 YU10:0x800000000000041 YUYV:0x800000000000041 Y210:0x800000000000041 XR30:0x800000000000011 XB30:0x800000000000011 XR24:0x800000000000011 AR24:0x800000000000011
plane 39 Primary: outputs 0b0, zpos 0..=7, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
  formats (32): (identical to plane 33)
plane 45 Overlay: outputs 0b1, zpos 0..=7, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
  formats (32): (identical to plane 33)
plane 51 Overlay: outputs 0b1, zpos 0..=7, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
  formats (32): (identical to plane 33)
plane 57 Overlay: outputs 0b1, zpos 0..=7, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
  formats (21): XR24 AR24 XB24 AB24 RG24 BG24 RG16 BG16 NV12 NV21 NV16 NV61 NV20 NV24 NV42 NV30 NV15 YVYU VYUY YUYV UYVY
plane 63 Overlay: outputs 0b1, zpos 0..=7, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
  formats (21): (identical to plane 57)
plane 69 Overlay: outputs 0b1, zpos 0..=7, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
  formats (21): (identical to plane 57)
plane 75 Overlay: outputs 0b1, zpos 0..=7, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
  formats (21): (identical to plane 57)
"#;

// Source: docs/platform-capabilities.md, probe on a Raspberry Pi 5 (vc4), observed
// 2026-11-28: the output, four primaries and the first overlay.
const BCM2712_PROBE: &str = r#"
output 0: crtc 94 1920x1080 @ 60.000 Hz
plane 48 Primary: outputs 0b0, zpos fixed 0, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
  formats (32): XR24 AR24 AB24 XB24 RX24 RA24 BX24 BA24 RG16 BG16 AR15 XR15 RG24 BG24 YU16 YV16 YU24 YV24 YU12 YV12 NV12 NV21 NV16 NV61 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12
plane 67 Primary: outputs 0b0, zpos fixed 0, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
  formats (32): XR24 AR24 AB24 XB24 RX24 RA24 BX24 BA24 RG16 BG16 AR15 XR15 RG24 BG24 YU16 YV16 YU24 YV24 YU12 YV12 NV12 NV21 NV16 NV61 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12
plane 83 Primary: outputs 0b1, zpos fixed 0, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
  formats (32): XR24 AR24 AB24 XB24 RX24 RA24 BX24 BA24 RG16 BG16 AR15 XR15 RG24 BG24 YU16 YV16 YU24 YV24 YU12 YV12 NV12 NV21 NV16 NV61 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12
plane 95 Primary: outputs 0b0, zpos fixed 0, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
  formats (32): XR24 AR24 AB24 XB24 RX24 RA24 BX24 BA24 RG16 BG16 AR15 XR15 RG24 BG24 YU16 YV16 YU24 YV24 YU12 YV12 NV12 NV21 NV16 NV61 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12
plane 107 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
  formats (32): XR24 AR24 AB24 XB24 RX24 RA24 BX24 BA24 RG16 BG16 AR15 XR15 RG24 BG24 YU16 YV16 YU24 YV24 YU12 YV12 NV12 NV21 NV16 NV61 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12
"#;

// Source: docs/platform-capabilities.md, probe on a reTerminal (CM4), observed
// 2026-11-28: the output and the first four primaries.
const BCM2711_PROBE: &str = r#"
output 0: crtc 90 720x1280 @ 60.000 Hz
plane 48 Primary: outputs 0b0, zpos fixed 0, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
  formats (32): XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15 RG24 BG24 YU16 YV16 YU24 YV24 YU12 YV12 NV12 NV21 NV16 NV61 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12 XB12 AB12 BX12 BA12
plane 67 Primary: outputs 0b0, zpos fixed 0, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
  formats (32): XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15 RG24 BG24 YU16 YV16 YU24 YV24 YU12 YV12 NV12 NV21 NV16 NV61 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12 XB12 AB12 BX12 BA12
plane 79 Primary: outputs 0b1, zpos fixed 0, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
  formats (32): XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15 RG24 BG24 YU16 YV16 YU24 YV24 YU12 YV12 NV12 NV21 NV16 NV61 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12 XB12 AB12 BX12 BA12
plane 91 Primary: outputs 0b0, zpos fixed 0, plane alpha true, blend [none, premultiplied, coverage], scaling not claimed
  formats (32): XR24 AR24 AB24 XB24 RG16 BG16 AR15 XR15 RG24 BG24 YU16 YV16 YU24 YV24 YU12 YV12 NV12 NV21 NV16 NV61 XR30 AR30 AB30 XB30 RGB8 BGR8 XR12 AR12 XB12 AB12 BX12 BA12
"#;

#[test]
fn rk3588_eight_plane_block() {
    let loaded = load(RK3588_PROBE).unwrap();
    // Only the `device` line is not a capability line.
    assert_eq!(loaded.skipped, 1);
    let caps = loaded.caps;
    assert_eq!(
        caps.outputs.as_slice(),
        [OutputCaps {
            id: OutputId(81),
            size: Size::new(1920, 1080),
            refresh_mhz: 60_000,
        }]
    );
    let ids: Vec<u32> = caps.planes.iter().map(|p| p.id.0).collect();
    assert_eq!(ids, [33, 39, 45, 51, 57, 63, 69, 75]);
    for p in caps.planes.iter() {
        assert_eq!(p.zpos, Zpos::Range { min: 0, max: 7 });
        assert!(p.plane_alpha);
        assert_eq!(p.blend_modes, ALL_BLEND);
        assert_eq!(p.scaling, None);
        assert_eq!(p.max_source, None);
        assert_eq!(p.transforms, 0);
        // The plane that cannot reach video_port0 has an empty mask.
        assert_eq!(p.outputs, u32::from(p.id.0 != 39));
    }
    assert_eq!(plane(&caps, 33).kind, PlaneKind::Primary);
    assert_eq!(plane(&caps, 39).kind, PlaneKind::Primary);
    assert_eq!(plane(&caps, 45).kind, PlaneKind::Overlay);

    let cluster = plane(&caps, 33);
    assert_eq!(cluster.formats.len(), 32);
    assert_eq!(cluster.formats[0], fm(b"XR30", 0x0800_0000_0000_0001));
    assert_eq!(cluster.formats[14], fm(b"XR30", 0x0800_0000_0000_0041));
    assert_eq!(cluster.formats[31], fm(b"AR24", 0x0800_0000_0000_0011));
    assert!(cluster
        .formats
        .iter()
        .all(|f| f.modifier.0 >> 56 == u64::from(VENDOR_ARM)));
    for id in [39, 45, 51] {
        assert_eq!(plane(&caps, id).formats, cluster.formats);
    }

    let esmart = plane(&caps, 57);
    assert_eq!(esmart.formats.len(), 21);
    assert!(esmart
        .formats
        .iter()
        .all(|f| f.modifier == Modifier::LINEAR));
    assert_eq!(
        esmart.formats[0],
        FormatModifier::linear(PixelFormat::XRGB8888)
    );
    assert_eq!(
        esmart.formats[20],
        FormatModifier::linear(PixelFormat::UYVY)
    );
    for id in [63, 69, 75] {
        assert_eq!(plane(&caps, id).formats, esmart.formats);
    }
    assert!(caps.resource_groups.is_empty());
    assert!(!caps.replace_pending);
}

#[test]
fn bcm2712_block() {
    let loaded = load(BCM2712_PROBE).unwrap();
    assert_eq!(loaded.skipped, 0);
    let caps = loaded.caps;
    assert_eq!(caps.outputs[0].id, OutputId(94));
    assert_eq!(caps.outputs[0].size, Size::new(1920, 1080));
    let ids: Vec<u32> = caps.planes.iter().map(|p| p.id.0).collect();
    assert_eq!(ids, [48, 67, 83, 95, 107]);
    let masks: Vec<u32> = caps.planes.iter().map(|p| p.outputs).collect();
    assert_eq!(masks, [0, 0, 1, 0, 1]);
    for p in caps.planes.iter().take(4) {
        assert_eq!(p.kind, PlaneKind::Primary);
        assert_eq!(p.zpos, Zpos::Fixed(0));
    }
    let overlay = plane(&caps, 107);
    assert_eq!(overlay.kind, PlaneKind::Overlay);
    assert_eq!(overlay.zpos, Zpos::Range { min: 1, max: 17 });
    assert!(overlay.plane_alpha);
    assert_eq!(overlay.blend_modes, ALL_BLEND);
    assert_eq!(overlay.formats.len(), 32);
    assert!(overlay
        .formats
        .iter()
        .all(|f| f.modifier == Modifier::LINEAR));
    assert_eq!(
        overlay.formats[8],
        FormatModifier::linear(PixelFormat::RGB565)
    );
    assert_eq!(
        overlay.formats[20],
        FormatModifier::linear(PixelFormat::NV12)
    );
}

#[test]
fn bcm2711_block() {
    let loaded = load(BCM2711_PROBE).unwrap();
    assert_eq!(loaded.skipped, 0);
    let caps = loaded.caps;
    assert_eq!(caps.outputs[0].id, OutputId(90));
    assert_eq!(caps.outputs[0].size, Size::new(720, 1280));
    assert_eq!(caps.outputs[0].refresh_mhz, 60_000);
    let ids: Vec<u32> = caps.planes.iter().map(|p| p.id.0).collect();
    assert_eq!(ids, [48, 67, 79, 91]);
    assert!(caps.planes.iter().all(|p| p.kind == PlaneKind::Primary));
    assert_eq!(plane(&caps, 79).outputs, 1);
    let formats = &plane(&caps, 79).formats;
    assert_eq!(formats.len(), 32);
    assert_eq!(
        formats[31],
        FormatModifier::linear(PixelFormat::fourcc(b"BA12"))
    );
}

#[test]
fn modifier_names_and_values() {
    // Linux include/uapi/drm/drm_fourcc.h: Broadcom (0x07) T-tiled is code 1
    // and SAND32/64/128/256 are codes 2..5.
    assert_eq!(VC4_T_TILED, 0x0700_0000_0000_0001);
    assert_eq!(SAND64, 0x0700_0000_0000_0003);
    assert_eq!(SAND128, 0x0700_0000_0000_0004);
    assert_eq!(SAND256, 0x0700_0000_0000_0005);
    assert_eq!(vendor_modifier(VENDOR_BROADCOM, 1), VC4_T_TILED);
    assert_eq!(modifier_name(0), "linear");
    assert_eq!(modifier_name(VC4_T_TILED), "VC4_T_TILED");
    assert_eq!(modifier_name(SAND64), "SAND64");
    assert_eq!(modifier_name(SAND128), "SAND128");
    assert_eq!(modifier_name(SAND256), "SAND256");
    assert_eq!(modifier_name(0x0800_0000_0000_0001), "arm 0x1");
    assert_eq!(modifier_name(0x0700_0000_0000_0009), "broadcom 0x9");
    assert_eq!(modifier_name(0x0200_0000_0000_0001), "vendor 0x2 0x1");
}

#[test]
fn broadcom_modifiers_parse() {
    // Shaped after the modifier table of docs/platform-capabilities.md
    // (observed 2026-11-28), written as the probe prints a plane.
    let text = "output 0: crtc 94 1920x1080 @ 60.000 Hz\n\
        plane 107 Overlay: outputs 0b1, zpos 1..=17, plane alpha true, blend [none, coverage], scaling not claimed\n\
        \x20 formats (6): XR24 XR24:0x700000000000001 NV12:0x700000000000003 NV12:0x700000000000004 NV12:0x700000000000005 P030:0x700000000000004\n";
    let loaded = load(text).unwrap();
    assert_eq!(loaded.skipped, 0);
    let p = loaded.caps.planes[0];
    assert_eq!(
        p.blend_modes,
        BlendMode::Opaque.mask() | BlendMode::Coverage.mask()
    );
    let names: Vec<String> = p
        .formats
        .iter()
        .map(|f| modifier_name(f.modifier.0))
        .collect();
    assert_eq!(
        names,
        [
            "linear",
            "VC4_T_TILED",
            "SAND64",
            "SAND128",
            "SAND256",
            "SAND128"
        ]
    );
    assert_eq!(p.formats[5].format, PixelFormat::fourcc(b"P030"));
}

#[test]
fn twoplane_form() {
    // Shape of the PLANE lines in docs/platform-capabilities.md (twoplane,
    // 2026-11-27), with the format list written out.
    let text = "MODE 720x1280@60.000Hz output 0 (crtc 90) of 1\n\
        PLANE index 2 id 79 Primary zpos fixed 0 plane-alpha true blend [none,premultiplied,coverage] scaling none transforms 0x0 formats XR24 NV12:0x700000000000003\n\
        PLANE index 6 id 127 Overlay zpos 1..=17 plane-alpha false blend [none] scaling up 4.00 down 2.00 transforms 0x15 formats RG16\n";
    let loaded = load(text).unwrap();
    // The MODE line.
    assert_eq!(loaded.skipped, 1);
    let caps = loaded.caps;
    assert_eq!(caps.planes.len(), 2);
    let primary = caps.planes[0];
    assert_eq!(primary.id, PlaneId(79));
    assert_eq!(primary.zpos, Zpos::Fixed(0));
    assert_eq!(primary.blend_modes, ALL_BLEND);
    assert_eq!(primary.scaling, None);
    assert_eq!(primary.formats.len(), 2);
    assert_eq!(primary.formats[1], fm(b"NV12", SAND64));
    let overlay = caps.planes[1];
    assert_eq!(overlay.kind, PlaneKind::Overlay);
    assert!(!overlay.plane_alpha);
    assert_eq!(overlay.blend_modes, BlendMode::Opaque.mask());
    assert_eq!(
        overlay.scaling,
        Some(ScaleLimits {
            max_upscale: 4 << 16,
            max_downscale: 2 << 16
        })
    );
    assert_eq!(overlay.transforms, 0x15);
}

#[test]
fn skips_what_it_cannot_parse() {
    let text = "garbage\n\
        \n\
        output 0: crtc 1 640x480 @ 59.940 Hz\n\
        output 1: crtc x 640x480 @ 60.000 Hz\n\
        plane 5 Sideways: outputs 0b1, zpos fixed 0, plane alpha true, blend [none], scaling none\n\
        plane 6 Primary: outputs 0b1, zpos fixed 0, plane alpha true, blend [none], scaling not claimed\n\
        \x20 formats (3): XR24 bogus AR24:zz\n\
        \x20 formats (1): XR24\n\
        formats (1): XR24\n";
    let loaded = load(text).unwrap();
    // garbage, the bad output, the bad plane, and the format line with two
    // bad tokens.
    assert_eq!(loaded.skipped, 4);
    assert_eq!(loaded.caps.outputs.len(), 1);
    assert_eq!(loaded.caps.outputs[0].refresh_mhz, 59_940);
    assert_eq!(loaded.caps.planes.len(), 1);
    // The later `formats` lines replace the earlier ones.
    assert_eq!(
        loaded.caps.planes[0].formats.as_slice(),
        [FormatModifier::linear(PixelFormat::XRGB8888)]
    );
}

#[test]
fn formats_before_any_plane_are_skipped() {
    let loaded = load("  formats (1): XR24\n").unwrap();
    assert_eq!(loaded.skipped, 1);
    assert!(loaded.caps.planes.is_empty());
}

#[test]
fn raw_dump_is_not_read() {
    let text = "output 0: crtc 1 640x480 @ 60.000 Hz\n\
        plane 5 Primary: outputs 0b1, zpos fixed 0, plane alpha true, blend [none], scaling not claimed\n\
        \x20 formats (1): XR24\n\
        \n\
        raw\n\
        plane 5 type primary possible crtcs 0b1 (1)\n\
        \x20 IN_FORMATS (1): AR24:0x0\n";
    let loaded = load(text).unwrap();
    assert_eq!(loaded.skipped, 0);
    assert_eq!(loaded.caps.planes.len(), 1);
    assert_eq!(loaded.caps.planes[0].formats.len(), 1);
}

fn plane_text(id: u32, formats: usize) -> String {
    let mut text = format!(
        "plane {id} Overlay: outputs 0b1, zpos fixed 0, plane alpha true, blend [none], scaling not claimed\n  formats ({formats}):"
    );
    for n in 0..formats {
        text.push_str(&format!(" X{:03}", n));
    }
    text.push('\n');
    text
}

#[test]
fn limits_are_errors_unless_truncating() {
    let mut text = String::new();
    for id in 0..=MAX_PLANES as u32 {
        text.push_str(&plane_text(id, 1));
    }
    assert_eq!(load(&text).unwrap_err(), LoadError::TooManyPlanes);
    let kept = load_with(&text, Overflow::Truncate).unwrap();
    assert_eq!(kept.caps.planes.len(), MAX_PLANES);
    assert_eq!(
        kept.caps.planes[MAX_PLANES - 1].id,
        PlaneId(MAX_PLANES as u32 - 1)
    );
    // The format line of the dropped plane does not land on a kept one.
    assert_eq!(kept.skipped, 0);
    assert_eq!(kept.caps.planes[MAX_PLANES - 1].formats.len(), 1);

    let exact = plane_text(7, MAX_FORMATS);
    assert_eq!(
        load(&exact).unwrap().caps.planes[0].formats.len(),
        MAX_FORMATS
    );
    let over = plane_text(7, MAX_FORMATS + 1);
    assert_eq!(load(&over).unwrap_err(), LoadError::TooManyFormats(7));
    let cut = load_with(&over, Overflow::Truncate).unwrap();
    assert_eq!(cut.caps.planes[0].formats.len(), MAX_FORMATS);

    let outputs = "output 0: crtc 1 1x1 @ 1.000 Hz\n".repeat(5);
    assert_eq!(load(&outputs).unwrap_err(), LoadError::TooManyOutputs);
    assert_eq!(
        load_with(&outputs, Overflow::Truncate)
            .unwrap()
            .caps
            .outputs
            .len(),
        4
    );
}

/// Prints capabilities the way the probe example does.
fn probe_text(caps: &Capabilities) -> String {
    let fourcc = |format: PixelFormat| -> String {
        format.0.to_le_bytes().iter().map(|b| *b as char).collect()
    };
    let mut text = String::new();
    for (index, output) in caps.outputs.iter().enumerate() {
        text.push_str(&format!(
            "output {index}: crtc {} {}x{} @ {}.{:03} Hz\n",
            output.id.0,
            output.size.width,
            output.size.height,
            output.refresh_mhz / 1000,
            output.refresh_mhz % 1000
        ));
    }
    for plane in caps.planes.iter() {
        let zpos = match plane.zpos {
            Zpos::Fixed(z) => format!("fixed {z}"),
            Zpos::Range { min, max } => format!("{min}..={max}"),
        };
        let blend: Vec<&str> = [
            (BlendMode::Opaque, "none"),
            (BlendMode::Premultiplied, "premultiplied"),
            (BlendMode::Coverage, "coverage"),
        ]
        .into_iter()
        .filter(|(mode, _)| plane.blend_modes & mode.mask() != 0)
        .map(|(_, name)| name)
        .collect();
        text.push_str(&format!(
            "plane {} {:?}: outputs {:#b}, zpos {zpos}, plane alpha {}, blend [{}], scaling {}\n",
            plane.id.0,
            plane.kind,
            plane.outputs,
            plane.plane_alpha,
            blend.join(", "),
            if plane.scaling.is_some() {
                "assumed"
            } else {
                "not claimed"
            },
        ));
        let formats: Vec<String> = plane
            .formats
            .iter()
            .map(|f| {
                if f.modifier.0 == 0 {
                    fourcc(f.format)
                } else {
                    format!("{}:{:#x}", fourcc(f.format), f.modifier.0)
                }
            })
            .collect();
        text.push_str(&format!(
            "  formats ({}): {}\n",
            formats.len(),
            formats.join(" ")
        ));
    }
    text
}

#[test]
fn round_trip_of_a_built_description() {
    // probe's own formatting function is private to the example, so this
    // mirrors it line for line.
    let mut caps = Capabilities::default();
    for (id, width, height, mhz) in [(81, 1920, 1080, 60_000), (82, 1280, 720, 59_940)] {
        caps.outputs
            .push(OutputCaps {
                id: OutputId(id),
                size: Size::new(width, height),
                refresh_mhz: mhz,
            })
            .unwrap();
    }
    let mut formats = composition_core::fixed::FixedVec::new();
    for f in [
        FormatModifier::linear(PixelFormat::XRGB8888),
        fm(b"AR24", 0x0800_0000_0000_0001),
        fm(b"NV12", SAND128),
    ] {
        formats.push(f).unwrap();
    }
    let planes = [
        (
            33,
            PlaneKind::Primary,
            0b11,
            Zpos::Fixed(0),
            true,
            0b111,
            None,
        ),
        (
            45,
            PlaneKind::Overlay,
            0b01,
            Zpos::Range { min: 1, max: 17 },
            false,
            0b101,
            Some(ScaleLimits::default()),
        ),
        (
            51,
            PlaneKind::Cursor,
            0,
            Zpos::Range { min: -2, max: 3 },
            true,
            0,
            None,
        ),
    ];
    for (id, kind, outputs, zpos, plane_alpha, blend_modes, scaling) in planes {
        caps.planes
            .push(PlaneCaps {
                id: PlaneId(id),
                kind,
                outputs,
                formats,
                scaling,
                plane_alpha,
                blend_modes,
                zpos,
                ..PlaneCaps::default()
            })
            .unwrap();
    }
    let loaded = load(&probe_text(&caps)).unwrap();
    assert_eq!(loaded.skipped, 0);
    assert_eq!(loaded.caps, caps);
}
