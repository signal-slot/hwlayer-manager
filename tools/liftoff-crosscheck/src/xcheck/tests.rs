use super::*;

const BASE: &str = "\
xcheck 1
origin synthetic 2026-11-28 unit test imitating nothing
";

const SAMPLE: &str = "\
# a comment line
xcheck 1
origin observed 2026-11-27 topology of a recorded document   # trailing comment
profile generic
output 100 1920 1080 60000
output 101 1280 720 59940
plane 31 kind=primary outputs=0x3 zpos=fixed:0 alpha=1 blend=opaque,premul transforms=normal,rot180 scaling=up:0x20000/down:0x8000 max_source=4096x4096 full_output=1
  format XR24 0x0
  format AR24 0x0
plane 32 kind=overlay outputs=0x1 zpos=0..7 zpos_now=3 alpha=0 blend=- transforms=-
  format NV12 0x100000000000001
group consumers=any,yuv planes=31,32 capacity=2
buffer 1 640 480 XR24 0x0
buffer 2 64 64 NV12 0x0
frame f1 output=100
  layer buffer=1 src=0,0,640,480 dst=-10,20,1280,960 z=-1 alpha=65535 blend=premul transform=flip_h
  layer buffer=2 src=0,0,64,64 dst=0,0,64,64 z=2 alpha=0 blend=coverage transform=rot90
end
frame empty output=101
end
";

fn line_of(text: &str) -> Error {
    read(text).unwrap_err()
}

fn kind_at(text: &str, line: usize) -> ErrorKind {
    let e = line_of(text);
    assert_eq!(e.line, line, "{e}");
    e.kind
}

#[test]
fn reads_sample() {
    let x = read(SAMPLE).unwrap();
    assert_eq!(x.origin.kind, OriginKind::Observed);
    assert_eq!(x.origin.date, "2026-11-27");
    assert_eq!(x.origin.text, "topology of a recorded document");
    assert_eq!(x.outputs.len(), 2);
    assert_eq!(x.planes.len(), 2);
    assert_eq!(x.planes[0].formats.len(), 2);
    assert_eq!(x.planes[0].blend, BlendSet(1 | 2));
    assert_eq!(
        x.planes[0].scaling,
        Some(ScaleDef {
            up: 0x20000,
            down: 0x8000
        })
    );
    assert_eq!(x.planes[1].zpos, ZposDef::Range { min: 0, max: 7 });
    assert_eq!(x.planes[1].zpos_now, Some(3));
    assert_eq!(
        x.groups[0].consumers,
        Consumers(Consumers::ANY | Consumers::YUV)
    );
    assert_eq!(x.frames.len(), 2);
    assert_eq!(x.frames[0].layers[0].dst.x, -10);
    assert_eq!(x.frames[0].layers[0].transform, rotation::REFLECT_X);
    assert!(x.frames[1].layers.is_empty());
}

#[test]
fn round_trip() {
    let x = read(SAMPLE).unwrap();
    let text = write(&x).unwrap();
    assert_eq!(read(&text).unwrap(), x);
    // The writer's own output is a fixed point.
    assert_eq!(write(&read(&text).unwrap()).unwrap(), text);
}

#[test]
fn defaults_and_optional_fields() {
    let t = format!(
        "{BASE}output 1 10 10 60000\n\
         plane 5 kind=cursor outputs=0x1 zpos=fixed:2 alpha=0 blend=- transforms=-\n"
    );
    let x = read(&t).unwrap();
    assert_eq!(x.profile, Profile::Generic);
    let p = &x.planes[0];
    assert_eq!(
        (p.scaling, p.max_source, p.full_output),
        (None, None, false)
    );
    assert_eq!(read(&write(&x).unwrap()).unwrap(), x);
}

#[test]
fn imx95dpu_alone_is_fine() {
    let t = format!(
        "{BASE}profile imx95dpu\noutput 1 10 10 60000\n\
         plane 5 kind=primary outputs=0x1 zpos=fixed:0 alpha=1 blend=- transforms=- scaling=none max_source=none\n"
    );
    assert_eq!(read(&t).unwrap().profile, Profile::Imx95Dpu);
}

#[test]
fn refuses_missing_header() {
    assert_eq!(kind_at("", 0), ErrorKind::MissingHeader);
    assert_eq!(kind_at("# only\n", 0), ErrorKind::MissingHeader);
    assert_eq!(
        kind_at("origin synthetic 2026-11-28 x\n", 1),
        ErrorKind::MissingHeader
    );
    assert_eq!(kind_at("xcheck 2\n", 1), ErrorKind::MissingHeader);
}

#[test]
fn refuses_missing_origin() {
    assert_eq!(kind_at("xcheck 1\n", 1), ErrorKind::MissingOrigin);
    assert_eq!(
        kind_at("xcheck 1\n\nprofile generic\n", 3),
        ErrorKind::MissingOrigin
    );
}

#[test]
fn refuses_bad_origin() {
    for bad in [
        "origin guessed 2026-11-28 x",
        "origin observed 2026-13-28 x",
        "origin observed 28-11-2026 x",
        "origin observed 2026-11-28",
        "origin observed 2026-11-28   ",
    ] {
        let e = line_of(&format!("xcheck 1\n{bad}\n"));
        assert_eq!(e.line, 2, "{bad}");
        assert!(matches!(e.kind, ErrorKind::Syntax(_)), "{bad}");
    }
}

#[test]
fn refuses_range_zpos_without_zpos_now() {
    let t = format!(
        "{BASE}output 1 10 10 60000\n\
         plane 5 kind=overlay outputs=0x1 zpos=0..3 alpha=0 blend=- transforms=-\n"
    );
    assert_eq!(kind_at(&t, 4), ErrorKind::RangeZposWithoutNow);
}

#[test]
fn refuses_profile_with_explicit_constraints() {
    let plane = |extra: &str| {
        format!(
            "{BASE}profile imx95dpu\noutput 1 10 10 60000\n\
             plane 5 kind=primary outputs=0x1 zpos=fixed:0 alpha=0 blend=- transforms=- {extra}\n"
        )
    };
    assert_eq!(
        kind_at(&plane("scaling=up:0x20000/down:0x8000"), 5),
        ErrorKind::ProfileConflict
    );
    assert_eq!(
        kind_at(&plane("max_source=100x100"), 5),
        ErrorKind::ProfileConflict
    );
    let group = format!("{}group consumers=any planes=5 capacity=1\n", plane(""));
    assert_eq!(kind_at(&group, 6), ErrorKind::ProfileConflict);
    // Profile line after the explicit one is refused as well.
    let late = format!(
        "{BASE}output 1 10 10 60000\n\
         plane 5 kind=primary outputs=0x1 zpos=fixed:0 alpha=0 blend=- transforms=- max_source=1x1\n\
         profile imx95dpu\n"
    );
    assert_eq!(kind_at(&late, 5), ErrorKind::ProfileConflict);
    // The same constraints with the generic profile are fine.
    let ok = plane("max_source=100x100").replace("imx95dpu", "generic");
    assert!(read(&ok).is_ok());
}

fn limit(text: &str, what: &'static str, max: usize) -> usize {
    let e = line_of(text);
    assert_eq!(e.kind, ErrorKind::Limit { what, max }, "{e}");
    e.line
}

fn outputs(n: usize) -> String {
    (0..n)
        .map(|i| format!("output {i} 10 10 60000\n"))
        .collect()
}

fn plane_line(id: usize) -> String {
    format!("plane {id} kind=overlay outputs=0x1 zpos=fixed:{id} alpha=0 blend=- transforms=-\n")
}

fn buffer_line(id: usize) -> String {
    format!("buffer {id} 16 16 XR24 0x0\n")
}

#[test]
fn refuses_what_the_limits_cannot_hold() {
    let t = format!("{BASE}{}", outputs(MAX_OUTPUTS + 1));
    assert_eq!(limit(&t, "outputs", MAX_OUTPUTS), 3 + MAX_OUTPUTS);

    let mut t = format!("{BASE}{}", outputs(1));
    for i in 0..=MAX_PLANES {
        t.push_str(&plane_line(i));
    }
    assert_eq!(limit(&t, "planes", MAX_PLANES), 4 + MAX_PLANES);

    let mut t = format!("{BASE}{}{}", outputs(1), plane_line(1));
    for i in 0..=MAX_FORMATS {
        t.push_str(&format!("format XR24 0x{i:x}\n"));
    }
    assert_eq!(
        limit(&t, "formats per plane", MAX_FORMATS),
        4 + 1 + MAX_FORMATS
    );

    let mut t = BASE.to_string();
    for i in 0..=MAX_BUFFERS {
        t.push_str(&buffer_line(i));
    }
    assert_eq!(limit(&t, "buffers", MAX_BUFFERS), 3 + MAX_BUFFERS);

    let mut t = format!("{BASE}{}{}", outputs(1), plane_line(1));
    for _ in 0..=MAX_RESOURCE_GROUPS {
        t.push_str("group consumers=any planes=1 capacity=1\n");
    }
    assert_eq!(
        limit(&t, "groups", MAX_RESOURCE_GROUPS),
        5 + MAX_RESOURCE_GROUPS
    );

    let mut t = format!("{BASE}{}{}frame f output=0\n", outputs(1), buffer_line(1));
    for _ in 0..=MAX_LAYERS {
        t.push_str("layer buffer=1 src=0,0,16,16 dst=0,0,16,16 z=0 alpha=65535 blend=opaque transform=normal\n");
    }
    assert_eq!(limit(&t, "layers per frame", MAX_LAYERS), 6 + MAX_LAYERS);
}

#[test]
fn refuses_duplicates_and_dangling_references() {
    let o = format!("{BASE}{}", outputs(1));
    assert!(matches!(
        kind_at(&format!("{o}output 0 1 1 1\n"), 4),
        ErrorKind::Duplicate(_)
    ));
    assert!(matches!(
        kind_at(&format!("{o}{}{}", plane_line(1), plane_line(1)), 5),
        ErrorKind::Duplicate(_)
    ));
    assert!(matches!(
        kind_at(&format!("{o}{}{}", buffer_line(1), buffer_line(1)), 5),
        ErrorKind::Duplicate(_)
    ));
    // Mask names an output that is not defined.
    assert!(matches!(
        kind_at(
            &format!(
                "{o}plane 1 kind=overlay outputs=0x2 zpos=fixed:0 alpha=0 blend=- transforms=-\n"
            ),
            4
        ),
        ErrorKind::UnknownRef(_)
    ));
    assert!(matches!(
        kind_at(&format!("{o}group consumers=any planes=9 capacity=1\n"), 4),
        ErrorKind::UnknownRef(_)
    ));
    assert!(matches!(
        kind_at(&format!("{o}frame f output=7\nend\n"), 4),
        ErrorKind::UnknownRef(_)
    ));
    assert!(matches!(
        kind_at(
            &format!("{o}frame f output=0\nlayer buffer=3 src=0,0,1,1 dst=0,0,1,1 z=0 alpha=1 blend=opaque transform=normal\nend\n"),
            5
        ),
        ErrorKind::UnknownRef(_)
    ));
    assert!(matches!(
        kind_at(
            &format!("{o}frame f output=0\nend\nframe f output=0\nend\n"),
            6
        ),
        ErrorKind::Duplicate(_)
    ));
}

#[test]
fn refuses_malformed_records() {
    let o = format!("{BASE}{}", outputs(1));
    let bad = [
        "bogus 1 2".to_string(),
        "output 1 2 3".to_string(),
        "plane 1 kind=wing outputs=0x1 zpos=fixed:0 alpha=0 blend=- transforms=-".to_string(),
        "plane 1 kind=overlay outputs=1 zpos=fixed:0 alpha=0 blend=- transforms=-".to_string(),
        "plane 1 kind=overlay outputs=0x1 zpos=3..1 alpha=0 blend=- transforms=-".to_string(),
        "plane 1 kind=overlay outputs=0x1 zpos=fixed:0 alpha=2 blend=- transforms=-".to_string(),
        "plane 1 kind=overlay outputs=0x1 zpos=fixed:0 alpha=0 blend=add transforms=-".to_string(),
        "plane 1 kind=overlay outputs=0x1 zpos=fixed:0 alpha=0 blend=- transforms=spin".to_string(),
        "plane 1 kind=overlay outputs=0x1 zpos=fixed:0 alpha=0 blend=- transforms=- extra=1".to_string(),
        "plane 1 kind=overlay outputs=0x1 zpos=fixed:0 zpos=fixed:1 alpha=0 blend=- transforms=-".to_string(),
        "plane 1 kind=overlay outputs=0x1 zpos=fixed:0 alpha=0 blend=- transforms=- scaling=up:1/down:2".to_string(),
        "plane 1 kind=overlay outputs=0x1 zpos=fixed:0 alpha=0 blend=- transforms=- max_source=big".to_string(),
        "format XR24 0x0".to_string(),
        "buffer 1 2 3 XR2 0x0".to_string(),
        "group consumers=- planes=1 capacity=1".to_string(),
        "layer buffer=1".to_string(),
        "end".to_string(),
        "profile fancy".to_string(),
        "origin synthetic 2026-11-28 again".to_string(),
    ];
    for b in bad {
        let e = line_of(&format!("{o}{b}\n"));
        assert_eq!(e.line, 4, "{b}: {e}");
    }
    // Format line after a non-plane record.
    let t = format!(
        "{o}{}buffer 1 1 1 XR24 0x0\nformat XR24 0x0\n",
        plane_line(1)
    );
    assert_eq!(line_of(&t).line, 6);
    // Records inside a frame.
    let t = format!("{o}frame f output=0\nbuffer 1 1 1 XR24 0x0\n");
    assert_eq!(line_of(&t).line, 5);
}

#[test]
fn refuses_unterminated_frame() {
    let t = format!("{BASE}{}frame f output=0\n", outputs(1));
    assert_eq!(kind_at(&t, 4), ErrorKind::UnterminatedFrame);
}

#[test]
fn error_message_names_the_line() {
    let e = line_of("xcheck 1\norigin synthetic 2026-11-28 x\nnonsense\n");
    assert!(e.to_string().starts_with("xcheck line 3: "), "{e}");
}

#[test]
fn writer_refuses_what_the_format_cannot_carry() {
    let good = read(SAMPLE).unwrap();
    let mut x = good.clone();
    x.origin.text = "has # hash".into();
    assert!(write(&x).is_err());
    let mut x = good.clone();
    x.origin.text = "two\nlines".into();
    assert!(write(&x).is_err());
    let mut x = good.clone();
    x.frames[0].name = "has space".into();
    assert!(write(&x).is_err());
    let mut x = good.clone();
    x.planes[1].zpos_now = None;
    assert_eq!(write(&x).unwrap_err().kind, ErrorKind::RangeZposWithoutNow);
    let mut x = good;
    x.frames[0].layers[0].transform = rotation::ROTATE_90 | rotation::REFLECT_X;
    assert!(write(&x).is_err());
}

/// The largest file the limits allow, built through the model.
fn large() -> XCheck {
    let outputs: Vec<Output> = (0..MAX_OUTPUTS as u32)
        .map(|i| Output {
            id: 100 + i,
            width: 3840,
            height: 2160,
            refresh_mhz: 60000,
        })
        .collect();
    let planes: Vec<Plane> = (0..MAX_PLANES as u32)
        .map(|i| Plane {
            id: 30 + i,
            kind: if i == 0 {
                PlaneKind::Primary
            } else {
                PlaneKind::Overlay
            },
            outputs: (1 << MAX_OUTPUTS) - 1,
            zpos: if i % 2 == 0 {
                ZposDef::Fixed(i as i32)
            } else {
                ZposDef::Range { min: 0, max: 15 }
            },
            zpos_now: (i % 2 == 1).then_some(i as i32),
            alpha: i % 3 == 0,
            blend: BlendSet(7),
            transforms: rotation::ROTATE_0 | rotation::ROTATE_270 | rotation::REFLECT_Y,
            scaling: (i % 4 == 0).then_some(ScaleDef {
                up: 0x40000,
                down: 0x4000,
            }),
            max_source: (i % 5 == 0).then_some((2048, 2048)),
            full_output: i == 0,
            formats: (0..MAX_FORMATS as u64)
                .map(|m| FormatDef {
                    fourcc: if m % 2 == 0 { *b"XR24" } else { *b"NV12" },
                    modifier: m << 32 | i as u64,
                })
                .collect(),
        })
        .collect();
    let groups: Vec<Group> = (0..MAX_RESOURCE_GROUPS as u32)
        .map(|g| Group {
            consumers: Consumers(Consumers::SCALED_H | Consumers::SCALED_V),
            planes: vec![30 + g, 38 + g],
            capacity: 1 + g,
        })
        .collect();
    let buffers: Vec<BufferDef> = (0..MAX_BUFFERS as u32)
        .map(|i| BufferDef {
            id: 1 + i,
            width: 64 + i,
            height: 64,
            fourcc: *b"AR24",
            modifier: 0,
        })
        .collect();
    let frames: Vec<Frame> = (0..50)
        .map(|f| Frame {
            name: format!("frame{f}"),
            output: 100 + (f % MAX_OUTPUTS as u32),
            layers: (0..MAX_LAYERS as u32)
                .map(|l| Layer {
                    buffer: 1 + l,
                    src: URect {
                        x: 0,
                        y: 0,
                        w: 64 + l,
                        h: 64,
                    },
                    dst: Rect {
                        x: -(l as i32),
                        y: l as i32,
                        w: 128,
                        h: 128,
                    },
                    z: l as i32 - 4,
                    alpha: 65535 - l as u16,
                    blend: BlendMode::Coverage,
                    transform: rotation::ROTATE_180,
                })
                .collect(),
        })
        .collect();
    XCheck {
        origin: Origin {
            kind: OriginKind::Synthetic,
            date: "2026-11-28".into(),
            text: "maximum size file for the unit test".into(),
        },
        profile: Profile::Generic,
        outputs,
        planes,
        groups,
        buffers,
        frames,
    }
}

#[test]
fn large_valid_example_round_trips() {
    let x = large();
    let text = write(&x).unwrap();
    assert!(text.lines().count() > 1000);
    assert_eq!(read(&text).unwrap(), x);
}
