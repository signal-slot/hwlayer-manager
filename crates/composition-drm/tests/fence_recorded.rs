//! Acquire fences (`IN_FENCE_FD`) and `OUT_FENCE_PTR` discovery on the
//! recorded BCM2711, BCM2712 and RK3588 topologies (`docs/sync-fences.md`
//! §3.2).
//!
//! Host tests only: nothing here reaches a kernel. The property ids come
//! from the recorded `props` dumps, the planes and outputs from the
//! recorded `probe --raw` dumps, and the plans from the core planner. Any
//! open file stands in for a `sync_file`, since only descriptor numbers are
//! compared. None of this is a hardware or vkms result.
//!
//! Fixture data (all recorded 2026-11-27, read from `tests/data/`):
//! - `bcm2711/props-2026-11-27.txt` (reTerminal, CM4, kernel
//!   6.12.47+rpt-rpi-v8): 60 planes, each with `IN_FENCE_FD (id 18)`; six
//!   CRTCs, each with `OUT_FENCE_PTR (id 19)`. Output 0 is CRTC 90.
//! - `bcm2712/props-2026-11-27.txt` (Raspberry Pi 5, kernel
//!   6.18.50+rpt-rpi-2712): 56 planes with `IN_FENCE_FD (id 18)`, four
//!   CRTCs with `OUT_FENCE_PTR (id 19)`. Output 0 is CRTC 94.
//! - `rk3588/props-2026-11-27.txt` (NanoPC-T6, kernel
//!   6.18.55-current-rockchip64): 8 planes with `IN_FENCE_FD (id 18)`, two
//!   CRTCs with `OUT_FENCE_PTR (id 19)`. Output 0 is CRTC 81.

mod common;

use std::fs::File;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::os::unix::fs::MetadataExt;

use common::recorded;
use composition_core::planner::{plan, BufferLookup};
use composition_core::{
    BackendError, BlendMode, BufferId, BufferInfo, Capabilities, Frame, Layer, Modifier, OutputId,
    PixelFormat, Plan, PlaneId, Rect, Size,
};
use composition_drm::fence::{property_id, FenceTable, Purpose, IN_FENCE_FD, OUT_FENCE_PTR};

struct Soc {
    name: &'static str,
    props: &'static str,
    probe: &'static str,
    planes: usize,
    crtcs: usize,
    output: u32,
}

const SOCS: [Soc; 3] = [
    Soc {
        name: "BCM2711",
        props: include_str!("data/bcm2711/props-2026-11-27.txt"),
        probe: include_str!("data/bcm2711/probe-raw-2026-11-27.txt"),
        planes: 60,
        crtcs: 6,
        output: 90,
    },
    Soc {
        name: "BCM2712",
        props: include_str!("data/bcm2712/props-2026-11-27.txt"),
        probe: include_str!("data/bcm2712/probe-raw-2026-11-27.txt"),
        planes: 56,
        crtcs: 4,
        output: 94,
    },
    Soc {
        name: "RK3588",
        props: include_str!("data/rk3588/props-2026-11-27.txt"),
        probe: include_str!("data/rk3588/probe-raw-2026-11-27.txt"),
        planes: 8,
        crtcs: 2,
        output: 81,
    },
];

/// Objects of a `props` dump with their `(name, id)` properties.
#[derive(Default)]
struct Props {
    planes: Vec<(u32, Vec<(String, u32)>)>,
    crtcs: Vec<(u32, Vec<(String, u32)>)>,
}

/// Parses `crtc N (index I): ...` and `plane N: ...` blocks and their
/// `    NAME (id P): ...` lines; other blocks (connectors) are skipped.
fn parse_props(text: &str) -> Props {
    enum Block {
        Plane,
        Crtc,
        Other,
    }
    let mut props = Props::default();
    let mut block = Block::Other;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("plane ") {
            let id = rest.split(':').next().unwrap().parse().unwrap();
            props.planes.push((id, Vec::new()));
            block = Block::Plane;
        } else if let Some(rest) = line.strip_prefix("crtc ") {
            let id = rest.split(' ').next().unwrap().parse().unwrap();
            props.crtcs.push((id, Vec::new()));
            block = Block::Crtc;
        } else if !line.starts_with(' ') {
            block = Block::Other;
        } else if let Some((name, rest)) = line.trim().split_once(" (id ") {
            let Some((id, _)) = rest.split_once("):") else {
                continue;
            };
            let entry = (name.to_string(), id.parse().unwrap());
            match block {
                Block::Plane => props.planes.last_mut().unwrap().1.push(entry),
                Block::Crtc => props.crtcs.last_mut().unwrap().1.push(entry),
                Block::Other => {}
            }
        }
    }
    props
}

/// The fence table the backend would build from the dump.
fn table(props: &Props) -> FenceTable {
    let lookup = |list: &[(String, u32)], name| {
        property_id(list.iter().map(|(n, id)| (n.as_str(), *id)), name)
    };
    let mut table = FenceTable::new();
    for (id, list) in &props.planes {
        table.set_plane(PlaneId(*id), lookup(list, IN_FENCE_FD));
    }
    for (id, list) in &props.crtcs {
        table.set_output(OutputId(*id), lookup(list, OUT_FENCE_PTR));
    }
    table
}

/// The dump with the `name` property line removed from object `header`'s
/// block (a line starting with `header`).
fn without(text: &str, header: &str, name: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    let mut removed = false;
    for line in text.lines() {
        if !line.starts_with(' ') {
            inside = line.starts_with(header);
        }
        if inside && line.trim().starts_with(&format!("{name} (id ")) {
            removed = true;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    assert!(removed, "no {name} in {header}");
    out
}

fn caps(soc: &Soc) -> Capabilities {
    recorded::load(soc.probe).expect(soc.name).caps
}

const BACKGROUND: BufferId = BufferId(1);
const OVERLAY: BufferId = BufferId(2);

struct Buffers(Size);

impl BufferLookup for Buffers {
    fn buffer_info(&self, buffer: BufferId) -> Option<BufferInfo> {
        let (size, format) = match buffer {
            BACKGROUND => (self.0, PixelFormat::XRGB8888),
            OVERLAY => (Size::new(128, 128), PixelFormat::ARGB8888),
            _ => return None,
        };
        Some(BufferInfo {
            size,
            format,
            modifier: Modifier::LINEAR,
        })
    }
}

/// A full-output background and a premultiplied overlay above it.
fn layers(size: Size) -> [Layer; 2] {
    let background = Layer::new(
        BACKGROUND,
        Rect::new(0, 0, size.width, size.height),
        Rect::new(0, 0, size.width, size.height),
        0,
    );
    let mut overlay = Layer::new(
        OVERLAY,
        Rect::new(0, 0, 128, 128),
        Rect::new(64, 64, 128, 128),
        1,
    );
    overlay.blend = BlendMode::Premultiplied;
    [background, overlay]
}

/// Plans the two-layer frame on the recorded output 0.
fn planned(caps: &Capabilities, layers: &[Layer]) -> Plan {
    let output = caps.outputs[0];
    let frame = Frame {
        output: output.id,
        layers,
    };
    plan(&frame, caps, &Buffers(output.size)).expect("two layers plan")
}

fn plane_of(plan: &Plan, layer: usize) -> PlaneId {
    plan.assignments()
        .iter()
        .find(|a| a.layer == layer)
        .expect("layer assigned")
        .plane
}

/// Any open descriptor; nothing here reaches a kernel.
fn stand_in() -> OwnedFd {
    File::open("/dev/null").unwrap().into()
}

fn inode(fd: impl AsFd) -> (u64, u64) {
    let file = File::from(fd.as_fd().try_clone_to_owned().unwrap());
    let meta = file.metadata().unwrap();
    (meta.dev(), meta.ino())
}

#[test]
fn every_recorded_plane_lists_in_fence_fd_and_every_crtc_out_fence_ptr() {
    for soc in &SOCS {
        let props = parse_props(soc.props);
        assert_eq!(props.planes.len(), soc.planes, "{}", soc.name);
        assert_eq!(props.crtcs.len(), soc.crtcs, "{}", soc.name);
        let table = table(&props);
        for (id, _) in &props.planes {
            assert_eq!(
                table.plane_in_fence(PlaneId(*id)),
                Some(18),
                "{} plane {id}",
                soc.name
            );
        }
        for (id, _) in &props.crtcs {
            assert_eq!(
                table.output_out_fence(OutputId(*id)),
                Some(19),
                "{} crtc {id}",
                soc.name
            );
        }
        // The planes and the output the backend keeps are among them.
        let caps = caps(soc);
        assert_eq!(caps.outputs[0].id, OutputId(soc.output), "{}", soc.name);
        assert!(table.output_out_fence(caps.outputs[0].id).is_some());
        for plane in caps.planes.iter() {
            assert!(
                table.plane_in_fence(plane.id).is_some(),
                "{} kept plane {:?}",
                soc.name,
                plane.id
            );
        }
        // Unknown objects report no support.
        assert_eq!(table.plane_in_fence(PlaneId(1)), None);
        assert_eq!(table.output_out_fence(OutputId(1)), None);
    }
}

#[test]
fn a_frame_without_fences_adds_nothing_to_the_request() {
    for soc in &SOCS {
        let caps = caps(soc);
        let layers = layers(caps.outputs[0].size);
        let plan = planned(&caps, &layers);
        let frame = Frame {
            output: caps.outputs[0].id,
            layers: &layers,
        };
        let mut table = table(&parse_props(soc.props));
        for purpose in [Purpose::Check, Purpose::Commit] {
            let fences = table.in_fences(&frame, &plan, purpose).unwrap();
            assert!(fences.is_empty(), "{}", soc.name);
            assert!(fences.duplicates().is_empty());
            assert!(fences.buffers().is_empty());
        }
        // A fence on a buffer the frame does not show changes nothing.
        table.set_acquire_fence(BufferId(99), stand_in());
        let fences = table.in_fences(&frame, &plan, Purpose::Commit).unwrap();
        assert!(fences.is_empty(), "{}", soc.name);
    }
}

#[test]
fn a_fenced_buffer_sets_in_fence_fd_on_its_plane_at_commit() {
    for soc in &SOCS {
        let caps = caps(soc);
        let layers = layers(caps.outputs[0].size);
        let plan = planned(&caps, &layers);
        let frame = Frame {
            output: caps.outputs[0].id,
            layers: &layers,
        };
        let mut table = table(&parse_props(soc.props));
        let fence = stand_in();
        let raw = fence.as_raw_fd();
        assert!(table.set_acquire_fence(OVERLAY, fence).is_none());

        let fences = table.in_fences(&frame, &plan, Purpose::Commit).unwrap();
        let plane = plane_of(&plan, 1);
        assert_eq!(fences.properties().len(), 1, "{}", soc.name);
        let property = fences.properties()[0];
        assert_eq!(property.object, plane.0, "{}", soc.name);
        assert_eq!(property.property, 18);
        assert_eq!(property.value, raw as u64, "the backend's own descriptor");
        assert!(fences.duplicates().is_empty());
        assert_eq!(fences.buffers(), [OVERLAY]);
        // The background's plane is not fenced.
        assert!(fences
            .properties()
            .iter()
            .all(|p| p.object != plane_of(&plan, 0).0));
    }
}

#[test]
fn the_check_passes_a_duplicate_and_the_commit_keeps_the_fence() {
    for soc in &SOCS {
        let caps = caps(soc);
        let layers = layers(caps.outputs[0].size);
        let plan = planned(&caps, &layers);
        let frame = Frame {
            output: caps.outputs[0].id,
            layers: &layers,
        };
        let mut table = table(&parse_props(soc.props));
        let fence = stand_in();
        let raw = fence.as_raw_fd();
        let file = inode(&fence);
        table.set_acquire_fence(BACKGROUND, fence);

        let check = table.in_fences(&frame, &plan, Purpose::Check).unwrap();
        assert_eq!(check.properties().len(), 1, "{}", soc.name);
        assert_eq!(check.duplicates().len(), 1);
        let duplicate = &check.duplicates()[0];
        assert_eq!(check.properties()[0].value, duplicate.as_raw_fd() as u64);
        assert_ne!(duplicate.as_raw_fd(), raw, "a duplicate, not the fence");
        assert_eq!(inode(duplicate), file, "the same open file");
        assert_eq!(check.properties()[0].object, plane_of(&plan, 0).0);
        drop(check);

        // The fence is still there for the real commit.
        let stored = table.acquire_fence(BACKGROUND).expect("fence kept");
        assert_eq!(stored.as_raw_fd(), raw);
        let commit = table.in_fences(&frame, &plan, Purpose::Commit).unwrap();
        assert_eq!(commit.properties()[0].value, raw as u64);

        // After a successful commit the backend's copy is closed.
        table.committed(&commit);
        assert!(table.acquire_fence(BACKGROUND).is_none());
        assert!(table
            .in_fences(&frame, &plan, Purpose::Commit)
            .unwrap()
            .is_empty());
    }
}

#[test]
fn a_buffer_shown_twice_is_fenced_on_both_planes() {
    for soc in &SOCS {
        let caps = caps(soc);
        let size = caps.outputs[0].size;
        let [background, overlay] = layers(size);
        let mut second = overlay;
        second.destination = Rect::new(300, 64, 128, 128);
        second.z_order = 2;
        let layers = [background, overlay, second];
        let plan = planned(&caps, &layers);
        let frame = Frame {
            output: caps.outputs[0].id,
            layers: &layers,
        };
        let mut table = table(&parse_props(soc.props));
        table.set_acquire_fence(OVERLAY, stand_in());
        let check = table.in_fences(&frame, &plan, Purpose::Check).unwrap();
        let objects: Vec<u32> = check.properties().iter().map(|p| p.object).collect();
        assert_eq!(
            objects,
            [plane_of(&plan, 1).0, plane_of(&plan, 2).0],
            "{}",
            soc.name
        );
        assert_eq!(check.duplicates().len(), 2);
        assert_eq!(check.buffers(), [OVERLAY]);
    }
}

#[test]
fn a_fenced_buffer_on_a_plane_without_in_fence_fd_is_rejected() {
    for soc in &SOCS {
        let caps = caps(soc);
        let layers = layers(caps.outputs[0].size);
        let plan = planned(&caps, &layers);
        let frame = Frame {
            output: caps.outputs[0].id,
            layers: &layers,
        };
        let plane = plane_of(&plan, 1);
        let edited = without(soc.props, &format!("plane {}:", plane.0), IN_FENCE_FD);
        let mut table = table(&parse_props(&edited));
        assert_eq!(table.plane_in_fence(plane), None, "{}", soc.name);
        assert_eq!(table.plane_in_fence(plane_of(&plan, 0)), Some(18));

        // Unfenced, the edited topology works as before.
        assert!(table
            .in_fences(&frame, &plan, Purpose::Check)
            .unwrap()
            .is_empty());

        table.set_acquire_fence(OVERLAY, stand_in());
        for purpose in [Purpose::Check, Purpose::Commit] {
            assert_eq!(
                table.in_fences(&frame, &plan, purpose).unwrap_err(),
                BackendError::Rejected,
                "{} {purpose:?}",
                soc.name
            );
        }
        // Refused, not dropped: the fence is still attached.
        assert!(table.acquire_fence(OVERLAY).is_some());

        // A fence on the other layer only is fine.
        let overlay_fence = table.clear_acquire_fence(OVERLAY).unwrap();
        drop(overlay_fence);
        table.set_acquire_fence(BACKGROUND, stand_in());
        let fences = table.in_fences(&frame, &plan, Purpose::Commit).unwrap();
        assert_eq!(fences.properties().len(), 1);
    }
}

#[test]
fn out_fence_ptr_is_reported_only_where_listed_and_never_requested() {
    for soc in &SOCS {
        let caps = caps(soc);
        let output = caps.outputs[0].id;
        let crtcs: Vec<u32> = parse_props(soc.props)
            .crtcs
            .iter()
            .map(|(id, _)| *id)
            .collect();
        let edited = without(soc.props, &format!("crtc {} ", output.0), OUT_FENCE_PTR);
        let props = parse_props(&edited);
        let mut table = table(&props);
        assert_eq!(table.output_out_fence(output), None, "{}", soc.name);
        for crtc in crtcs.iter().filter(|c| **c != output.0) {
            assert_eq!(table.output_out_fence(OutputId(*crtc)), Some(19));
        }

        // A fenced request touches planes only, whether or not the CRTC
        // lists the property: OUT_FENCE_PTR is not requested (see the fence
        // module docs: the returned descriptor cannot be owned without
        // unsafe code).
        let layers = layers(caps.outputs[0].size);
        let plan = planned(&caps, &layers);
        let frame = Frame {
            output,
            layers: &layers,
        };
        let mut listed = self::table(&parse_props(soc.props));
        for table in [&mut table, &mut listed] {
            table.set_acquire_fence(OVERLAY, stand_in());
            table.set_acquire_fence(BACKGROUND, stand_in());
            for purpose in [Purpose::Check, Purpose::Commit] {
                let fences = table.in_fences(&frame, &plan, purpose).unwrap();
                assert_eq!(fences.properties().len(), 2, "{}", soc.name);
                for property in fences.properties() {
                    assert!(!crtcs.contains(&property.object), "{}", soc.name);
                    assert_ne!(property.property, 19);
                }
            }
        }
    }
}

#[test]
fn replacing_and_clearing_a_fence() {
    let mut table = FenceTable::new();
    let first = stand_in();
    let first_raw = first.as_raw_fd();
    assert!(table.set_acquire_fence(OVERLAY, first).is_none());
    let replaced = table.set_acquire_fence(OVERLAY, stand_in()).unwrap();
    assert_eq!(replaced.as_raw_fd(), first_raw);
    assert!(table.clear_acquire_fence(OVERLAY).is_some());
    assert!(table.clear_acquire_fence(OVERLAY).is_none());
    assert!(table.acquire_fence(OVERLAY).is_none());
}
