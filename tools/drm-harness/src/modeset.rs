//! The mode switch of the `modeset` scenario, made by the harness itself.
//!
//! composition-drm programs the mode it found when it was opened, once, and
//! has no way to change it. So the harness changes the mode with its own
//! atomic commit on the same descriptor (master or lease): the new mode on
//! the CRTC, a buffer of the new size on the primary plane, every other
//! plane of the CRTC off. A fresh backend opened afterwards on the same
//! descriptor then finds the new mode and plans for it. The same commit,
//! with the original mode, puts the output back before the run ends. No
//! DRM, udev or seat configuration is touched.

use std::collections::HashMap;
use std::io;
use std::time::Instant;

use composition_drm::Card;
use drm::buffer::DrmFourcc;
use drm::control::atomic::AtomicModeReq;
use drm::control::dumbbuffer::DumbBuffer;
use drm::control::{
    connector, crtc, framebuffer, plane, property, AtomicCommitFlags, Device as ControlDevice,
    Mode, ModeFlags, PlaneType,
};

use crate::draw::{self, Op};
use crate::record::Record;
use crate::session::Pairing;
use crate::stats::micros;

/// What tells two modes apart for the scenario: size and refresh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeKey {
    /// Active width in pixels.
    pub width: u16,
    /// Active height in lines.
    pub height: u16,
    /// Refresh rate in hertz, as the kernel rounds it.
    pub vrefresh: u32,
    /// Interlaced modes are never chosen: not every CRTC takes them.
    pub interlaced: bool,
}

impl ModeKey {
    /// The key of a DRM mode.
    pub fn of(mode: &Mode) -> Self {
        let (width, height) = mode.size();
        Self {
            width,
            height,
            vrefresh: mode.vrefresh(),
            interlaced: mode.flags().contains(ModeFlags::INTERLACE),
        }
    }

    /// `WxH@R`, with `i` for an interlaced mode.
    pub fn text(&self) -> String {
        format!(
            "{}x{}{}@{}",
            self.width,
            self.height,
            if self.interlaced { "i" } else { "" },
            self.vrefresh
        )
    }
}

/// Indexes into `modes` of the modes to switch to, best first: progressive
/// modes that differ from `current` in size or refresh, each size and
/// refresh once, in the connector's order (the kernel lists the preferred
/// mode first, then by size). Empty when the sink offers only `current`.
pub fn candidates(current: ModeKey, modes: &[ModeKey]) -> Vec<usize> {
    let differs = |m: &ModeKey| {
        (m.width, m.height, m.vrefresh) != (current.width, current.height, current.vrefresh)
    };
    let mut chosen: Vec<usize> = Vec::new();
    for (index, mode) in modes.iter().enumerate() {
        if mode.interlaced || !differs(mode) {
            continue;
        }
        if chosen.iter().any(|&c| modes[c] == *mode) {
            continue;
        }
        chosen.push(index);
    }
    chosen
}

/// The BLOCKED reason when there is nothing to switch to, else `None`.
pub fn blocked_reason(current: ModeKey, modes: &[ModeKey]) -> Option<String> {
    if !candidates(current, modes).is_empty() {
        return None;
    }
    Some(if modes.iter().all(|m| !m.interlaced) {
        format!(
            "the connector offers only one mode ({}, {} listed)",
            current.text(),
            modes.len()
        )
    } else {
        format!(
            "the connector offers no progressive mode other than {} ({} listed)",
            current.text(),
            modes.len()
        )
    })
}

fn other(message: String) -> io::Error {
    io::Error::other(message)
}

/// Property handles of a KMS object by name.
fn props<H: drm::control::ResourceHandle>(
    card: &Card,
    handle: H,
) -> io::Result<HashMap<String, property::Handle>> {
    let set = card.get_properties(handle)?;
    let (handles, _) = set.as_props_and_values();
    let mut named = HashMap::new();
    for h in handles {
        let info = card.get_property(*h)?;
        named.insert(info.name().to_string_lossy().into_owned(), *h);
    }
    Ok(named)
}

fn need(map: &HashMap<String, property::Handle>, name: &str) -> io::Result<property::Handle> {
    map.get(name)
        .copied()
        .ok_or_else(|| other(format!("property {name} missing")))
}

fn plane_type(card: &Card, handle: plane::Handle) -> io::Result<Option<u64>> {
    let set = card.get_properties(handle)?;
    let (handles, values) = set.as_props_and_values();
    for (h, value) in handles.iter().zip(values) {
        if card.get_property(*h)?.name().to_bytes() == b"type" {
            return Ok(Some(*value));
        }
    }
    Ok(None)
}

/// A buffer the harness put on the primary plane for a switch.
struct Scaffold {
    framebuffer: framebuffer::Handle,
    dumb: DumbBuffer,
}

/// The objects a switch commit sets, found once.
pub struct Switcher {
    crtc: crtc::Handle,
    connector: connector::Handle,
    primary: plane::Handle,
    connector_crtc_id: property::Handle,
    crtc_mode_id: property::Handle,
    crtc_active: property::Handle,
    /// FB_ID, CRTC_ID, SRC_X/Y/W/H, CRTC_X/Y/W/H of the primary plane.
    plane: [property::Handle; 10],
    /// Every other plane that can be on this CRTC, with FB_ID and CRTC_ID.
    others: Vec<(plane::Handle, property::Handle, property::Handle)>,
    /// Buffers put on screen by switches, freed when the switcher goes.
    scaffolds: Vec<Scaffold>,
    /// Mode blobs made, destroyed when the switcher goes.
    blobs: Vec<u64>,
}

impl Switcher {
    /// Finds the CRTC's primary plane and the properties a switch sets.
    pub fn new(card: &Card, pairing: &Pairing) -> io::Result<Self> {
        let resources = card.resource_handles()?;
        let mut primary = None;
        let mut others = Vec::new();
        for handle in card.plane_handles()? {
            let info = card.get_plane(handle)?;
            if !resources
                .filter_crtcs(info.possible_crtcs())
                .contains(&pairing.crtc)
            {
                continue;
            }
            let on_crtc = info.crtc() == Some(pairing.crtc);
            if plane_type(card, handle)? == Some(PlaneType::Primary as u64)
                && (primary.is_none() || on_crtc)
            {
                // Prefer the primary plane already on this CRTC.
                if let Some(previous) = primary.replace(handle) {
                    others.push(previous);
                }
            } else {
                others.push(handle);
            }
        }
        let primary = primary.ok_or_else(|| {
            other(format!(
                "no primary plane for CRTC {}",
                u32::from(pairing.crtc)
            ))
        })?;
        let pp = props(card, primary)?;
        let mut plane = [need(&pp, "FB_ID")?; 10];
        for (slot, name) in plane.iter_mut().zip([
            "FB_ID", "CRTC_ID", "SRC_X", "SRC_Y", "SRC_W", "SRC_H", "CRTC_X", "CRTC_Y", "CRTC_W",
            "CRTC_H",
        ]) {
            *slot = need(&pp, name)?;
        }
        let mut off = Vec::new();
        for handle in others {
            let op = props(card, handle)?;
            off.push((handle, need(&op, "FB_ID")?, need(&op, "CRTC_ID")?));
        }
        let cp = props(card, pairing.handle)?;
        let rp = props(card, pairing.crtc)?;
        Ok(Self {
            crtc: pairing.crtc,
            connector: pairing.handle,
            primary,
            connector_crtc_id: need(&cp, "CRTC_ID")?,
            crtc_mode_id: need(&rp, "MODE_ID")?,
            crtc_active: need(&rp, "ACTIVE")?,
            plane,
            others: off,
            scaffolds: Vec::new(),
            blobs: Vec::new(),
        })
    }

    fn blob(&mut self, card: &Card, mode: &Mode) -> io::Result<u64> {
        match card.create_property_blob(mode)? {
            property::Value::Blob(id) => {
                self.blobs.push(id);
                Ok(id)
            }
            _ => Err(other("mode blob".into())),
        }
    }

    fn request(
        &self,
        blob: u64,
        framebuffer: Option<framebuffer::Handle>,
        mode: &Mode,
    ) -> AtomicModeReq {
        let (w, h) = mode.size();
        let (w, h) = (u64::from(w), u64::from(h));
        let mut request = AtomicModeReq::new();
        request.add_property(
            self.connector,
            self.connector_crtc_id,
            property::Value::CRTC(Some(self.crtc)),
        );
        request.add_property(self.crtc, self.crtc_mode_id, property::Value::Blob(blob));
        request.add_property(self.crtc, self.crtc_active, property::Value::Boolean(true));
        let p = &self.plane;
        request.add_property(
            self.primary,
            p[0],
            property::Value::Framebuffer(framebuffer),
        );
        request.add_property(self.primary, p[1], property::Value::CRTC(Some(self.crtc)));
        for (prop, value) in p[2..6].iter().zip([0, 0, w << 16, h << 16]) {
            request.add_property(self.primary, *prop, property::Value::UnsignedRange(value));
        }
        request.add_property(self.primary, p[6], property::Value::SignedRange(0));
        request.add_property(self.primary, p[7], property::Value::SignedRange(0));
        request.add_property(self.primary, p[8], property::Value::UnsignedRange(w));
        request.add_property(self.primary, p[9], property::Value::UnsignedRange(h));
        for (handle, fb_id, crtc_id) in &self.others {
            request.add_property(*handle, *fb_id, property::Value::Framebuffer(None));
            request.add_property(*handle, *crtc_id, property::Value::CRTC(None));
        }
        request
    }

    /// A buffer of the mode's size filled with `ops`, kept until the
    /// switcher goes.
    fn scaffold(
        &mut self,
        card: &Card,
        mode: &Mode,
        ops: &[Op],
    ) -> io::Result<framebuffer::Handle> {
        let (w, h) = mode.size();
        let mut dumb =
            card.create_dumb_buffer((u32::from(w), u32::from(h)), DrmFourcc::Xrgb8888, 32)?;
        let pitch = drm::buffer::Buffer::pitch(&dumb) as usize;
        let filled = card
            .map_dumb_buffer(&mut dumb)
            .map(|mut map| draw::apply(map.as_mut(), pitch, ops));
        let framebuffer = filled.and_then(|()| card.add_framebuffer(&dumb, 24, 32));
        match framebuffer {
            Ok(framebuffer) => {
                self.scaffolds.push(Scaffold { framebuffer, dumb });
                Ok(framebuffer)
            }
            Err(error) => {
                let _ = card.destroy_dumb_buffer(dumb);
                Err(error)
            }
        }
    }

    /// Whether the kernel would take `mode` (`TEST_ONLY`, nothing changes).
    pub fn test(&mut self, card: &Card, mode: &Mode) -> io::Result<()> {
        let blob = self.blob(card, mode)?;
        let framebuffer = self.scaffold(card, mode, &[])?;
        let request = self.request(blob, Some(framebuffer), mode);
        let result = card.atomic_commit(
            AtomicCommitFlags::TEST_ONLY | AtomicCommitFlags::ALLOW_MODESET,
            request,
        );
        // Never on screen: free it at once.
        if let Some(s) = self.scaffolds.pop() {
            let _ = card.destroy_framebuffer(s.framebuffer);
            let _ = card.destroy_dumb_buffer(s.dumb);
        }
        result
    }

    /// Sets `mode` with a blocking commit, `ops` drawn on the primary
    /// plane, every other plane of the CRTC off. Prints a `modeset` record
    /// and returns how long the commit took.
    pub fn switch(&mut self, card: &Card, step: &str, mode: &Mode, ops: &[Op]) -> io::Result<u64> {
        let blob = self.blob(card, mode)?;
        let framebuffer = self.scaffold(card, mode, ops)?;
        let request = self.request(blob, Some(framebuffer), mode);
        let start = Instant::now();
        let result = card.atomic_commit(AtomicCommitFlags::ALLOW_MODESET, request);
        let us = micros(start.elapsed());
        let set = card.get_crtc(self.crtc).ok().and_then(|c| c.mode());
        Record::new("modeset")
            .field("step", step)
            .field("mode", ModeKey::of(mode).text())
            .field("mode_name", mode.name().to_string_lossy())
            .field("commit_us", us)
            .field(
                "result",
                match &result {
                    Ok(()) => "ok".to_string(),
                    Err(error) => format!("{error}"),
                },
            )
            .field(
                "crtc_mode",
                set.map_or("-".to_string(), |m| ModeKey::of(&m).text()),
            )
            .emit();
        result.map(|()| us)
    }

    /// Frees the scaffold buffers and mode blobs. The buffer still on the
    /// primary plane goes with the descriptor, as every scenario's does.
    pub fn release(&mut self, card: &Card, keep_last: bool) {
        let keep = if keep_last {
            self.scaffolds.pop()
        } else {
            None
        };
        for s in self.scaffolds.drain(..) {
            let _ = card.destroy_framebuffer(s.framebuffer);
            let _ = card.destroy_dumb_buffer(s.dumb);
        }
        self.scaffolds.extend(keep);
        for blob in self.blobs.drain(..) {
            let _ = card.destroy_property_blob(blob);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(width: u16, height: u16, vrefresh: u32) -> ModeKey {
        ModeKey {
            width,
            height,
            vrefresh,
            interlaced: false,
        }
    }

    #[test]
    fn the_first_mode_that_differs_is_chosen() {
        let current = key(1920, 1080, 60);
        let modes = [
            key(1920, 1080, 60),
            key(1920, 1080, 60),
            key(1920, 1080, 50),
            key(1280, 720, 60),
            key(1920, 1080, 50),
        ];
        assert_eq!(candidates(current, &modes), vec![2, 3]);
        assert_eq!(blocked_reason(current, &modes), None);
    }

    #[test]
    fn the_current_mode_need_not_be_first() {
        let modes = [key(3840, 2160, 30), key(1920, 1080, 60), key(1024, 768, 60)];
        assert_eq!(candidates(key(1920, 1080, 60), &modes), vec![0, 2]);
    }

    #[test]
    fn interlaced_modes_are_never_chosen() {
        let mut i = key(1920, 1080, 60);
        i.interlaced = true;
        let modes = [key(1920, 1080, 60), i, key(720, 576, 50)];
        assert_eq!(candidates(key(1920, 1080, 60), &modes), vec![2]);
        let reason = blocked_reason(key(1920, 1080, 60), &[key(1920, 1080, 60), i]).unwrap();
        assert!(
            reason.contains("no progressive mode other than 1920x1080@60"),
            "{reason}"
        );
    }

    #[test]
    fn one_mode_is_blocked() {
        let current = key(800, 480, 60);
        let reason = blocked_reason(current, &[current]).unwrap();
        assert_eq!(
            reason,
            "the connector offers only one mode (800x480@60, 1 listed)"
        );
        // A panel listing its one mode twice still offers one mode.
        assert!(blocked_reason(current, &[current, current])
            .unwrap()
            .contains("only one mode"));
        assert!(candidates(current, &[]).is_empty());
        assert!(blocked_reason(current, &[]).is_some());
    }

    #[test]
    fn mode_text() {
        assert_eq!(key(1280, 720, 60).text(), "1280x720@60");
        let mut i = key(1920, 1080, 60);
        i.interlaced = true;
        assert_eq!(i.text(), "1920x1080i@60");
    }
}
