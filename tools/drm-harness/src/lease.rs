//! A DRM lease made by this process: the lessor opens the device as DRM
//! master and leases one output (connector, CRTC, primary plane and
//! overlay planes) with `DRM_IOCTL_MODE_CREATE_LEASE`. The engine is then
//! given the lease descriptor only.

use std::fs::OpenOptions;
use std::io;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

use composition_drm::Card;
use drm::control::{connector, crtc, plane, Device as ControlDevice, PlaneType, RawResourceHandle};
use drm::{ClientCapability, Device as BasicDevice};

use crate::record::{list, Record};
use crate::session::pairings;

/// What to lease.
#[derive(Clone, Debug)]
pub struct LeaseRequest {
    /// The device node of the lessor.
    pub device: PathBuf,
    /// Connector name or prefix; the first connected one if not given.
    pub connector: Option<String>,
    /// Overlay planes to lease; every overlay that can be used on the
    /// connector's CRTC if not given.
    pub overlays: Option<Vec<u32>>,
}

/// The leased objects.
#[derive(Clone, Debug)]
pub struct Leased {
    /// Connector.
    pub connector: connector::Handle,
    /// Its name, such as `LVDS-1`.
    pub connector_name: String,
    /// The CRTC driving it.
    pub crtc: crtc::Handle,
    /// The primary plane of the CRTC.
    pub primary: plane::Handle,
    /// Overlay planes.
    pub overlays: Vec<plane::Handle>,
}

impl Leased {
    /// Every leased plane id, primary first.
    pub fn plane_ids(&self) -> Vec<u32> {
        std::iter::once(self.primary)
            .chain(self.overlays.iter().copied())
            .map(u32::from)
            .collect()
    }
}

/// The lessor: DRM master of the device, holder of one lease.
pub struct Lessor {
    card: Card,
    device: PathBuf,
    lessee: Option<drm::control::LeaseId>,
    /// What was leased.
    pub leased: Leased,
}

fn plane_type(card: &Card, plane: plane::Handle) -> io::Result<Option<u64>> {
    let props = card.get_properties(plane)?;
    let (handles, values) = props.as_props_and_values();
    for (handle, value) in handles.iter().zip(values) {
        if card.get_property(*handle)?.name().to_bytes() == b"type" {
            return Ok(Some(*value));
        }
    }
    Ok(None)
}

fn other(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, message)
}

impl Lessor {
    /// Opens the device, becomes DRM master and picks the objects to lease.
    /// Nothing is leased yet.
    pub fn open(request: &LeaseRequest) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&request.device)?;
        let card = Card::from_fd(OwnedFd::from(file));
        card.set_client_capability(ClientCapability::UniversalPlanes, true)?;
        card.set_client_capability(ClientCapability::Atomic, true)?;
        card.acquire_master_lock().map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("lessor is not DRM master ({e}); stop the compositor that owns the device"),
            )
        })?;
        let leased = choose(&card, request)?;
        Ok(Self {
            card,
            device: request.device.clone(),
            lessee: None,
            leased,
        })
    }

    /// Creates the lease and returns its descriptor (close-on-exec).
    pub fn lease(&mut self) -> io::Result<OwnedFd> {
        if self.lessee.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "this lessor already holds a lease",
            ));
        }
        let l = &self.leased;
        let mut objects: Vec<RawResourceHandle> = vec![l.connector.into(), l.crtc.into()];
        objects.push(l.primary.into());
        objects.extend(l.overlays.iter().map(|p| RawResourceHandle::from(*p)));
        let flags = rustix::fs::OFlags::CLOEXEC.bits();
        let (lessee, fd) = self.card.create_lease(&objects, flags)?;
        self.lessee = Some(lessee);
        Record::new("lease")
            .field("route", "kernel")
            .field("device", self.device.display())
            .field("lessee", u32::from(lessee))
            .field("connector", u32::from(l.connector))
            .field("connector_name", &l.connector_name)
            .field("crtc", u32::from(l.crtc))
            .field("primary", u32::from(l.primary))
            .field("overlays", list(l.overlays.iter().map(|p| u32::from(*p))))
            .field("objects", objects.len())
            .emit();
        Ok(fd)
    }

    /// Revokes the lease. Every ioctl on its descriptor fails from then on.
    pub fn revoke(&mut self) -> io::Result<()> {
        let lessee = self
            .lessee
            .take()
            .ok_or_else(|| other("no lease to revoke".into()))?;
        self.card.revoke_lease(lessee)?;
        Record::new("lease_revoked")
            .field("lessee", u32::from(lessee))
            .emit();
        Ok(())
    }

    /// Whether a lease is held.
    pub fn holds_lease(&self) -> bool {
        self.lessee.is_some()
    }

    /// The device node.
    pub fn device(&self) -> &Path {
        &self.device
    }
}

/// Picks the connector, its CRTC (the way `DrmBackend` pairs them), the
/// CRTC's primary plane and the overlays.
fn choose(card: &Card, request: &LeaseRequest) -> io::Result<Leased> {
    let found = pairings(card)?;
    let pairing = found
        .into_iter()
        .find(|p| match &request.connector {
            Some(want) => p
                .connector
                .to_ascii_lowercase()
                .starts_with(&want.to_ascii_lowercase()),
            None => true,
        })
        .ok_or_else(|| {
            other(match &request.connector {
                Some(want) => format!("no connected connector matches {want}"),
                None => "no connected connector".into(),
            })
        })?;
    let resources = card.resource_handles()?;
    let mut primary = None;
    let mut overlays = Vec::new();
    for handle in card.plane_handles()? {
        let info = card.get_plane(handle)?;
        if !resources
            .filter_crtcs(info.possible_crtcs())
            .contains(&pairing.crtc)
        {
            continue;
        }
        let kind = plane_type(card, handle)?;
        if kind == Some(PlaneType::Primary as u64) {
            // Prefer the primary plane already on this CRTC.
            let on_crtc = info.crtc() == Some(pairing.crtc);
            if primary.is_none() || on_crtc {
                primary = Some(handle);
            }
        } else if kind == Some(PlaneType::Overlay as u64) {
            overlays.push(handle);
        }
    }
    let primary = primary.ok_or_else(|| {
        other(format!(
            "no primary plane for CRTC {}",
            u32::from(pairing.crtc)
        ))
    })?;
    if let Some(wanted) = &request.overlays {
        let mut chosen = Vec::new();
        for id in wanted {
            match overlays.iter().find(|p| u32::from(**p) == *id) {
                Some(plane) => chosen.push(*plane),
                None => {
                    return Err(other(format!(
                        "plane {id} is not an overlay usable on CRTC {}",
                        u32::from(pairing.crtc)
                    )))
                }
            }
        }
        overlays = chosen;
    }
    Ok(Leased {
        connector: pairing.handle,
        connector_name: pairing.connector,
        crtc: pairing.crtc,
        primary,
        overlays,
    })
}
