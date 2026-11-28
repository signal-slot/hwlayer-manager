//! Explicit synchronization: acquire fences through the plane property
//! `IN_FENCE_FD` (`docs/sync-fences.md` §3.2).
//!
//! A caller that hands over a buffer whose content is still being written
//! by a device (GPU, decoder, camera) attaches the producer's `sync_file`
//! to the imported buffer with [`DrmBackend::set_acquire_fence`]. Every
//! atomic request that shows the buffer on a plane then sets that plane's
//! `IN_FENCE_FD` to the descriptor, and the kernel holds the scanout of
//! the new configuration until the fence signals. Nothing in the core
//! (`composition-core`) knows about these descriptors; the backend keeps
//! them per buffer, next to the framebuffer.
//!
//! The rules:
//!
//! - **Discovery.** `IN_FENCE_FD` (plane) and `OUT_FENCE_PTR` (CRTC) are
//!   looked up by name, as optional properties, when the backend is
//!   created. Their presence is reported per plane and per output
//!   ([`DrmBackend::plane_in_fence_supported`],
//!   [`DrmBackend::output_out_fence_supported`]), not in
//!   [`composition_core::Capabilities`].
//! - **Refusal.** A frame that puts a buffer with an acquire fence on a
//!   plane without `IN_FENCE_FD` is refused with
//!   [`BackendError::Rejected`] by both `check` and `commit`, before the
//!   kernel is asked, so that the planner can try a plane that has the
//!   property. The fence is never silently dropped (§3.1).
//! - **`TEST_ONLY`.** The check passes a duplicate of the descriptor
//!   ([`OwnedFd::try_clone`]), closed after the ioctl, so that the real
//!   commit still has the backend's own descriptor.
//! - **Ownership.** The kernel takes its own reference to the fence during
//!   the ioctl. After a successful commit the backend closes its
//!   descriptor: the fence applies to that commit only. A later frame that
//!   shows the buffer again is not fenced unless a new fence is attached.
//!   After a failed commit the descriptor is kept, so a retry is fenced as
//!   well.
//! - **Errors.** A refusal of the kernel that goes away when the same
//!   request is tested without the fences (an `EINVAL` for a descriptor
//!   that is not a `sync_file`, for instance) is reported as
//!   [`BackendError::Platform`] with the `errno`, not as `Rejected`: it is
//!   the fence, not the plane configuration, that is wrong, and trying
//!   other assignments would not help. Other errors (`EBADF`, `ENODEV`,
//!   ...) go through the backend's usual mapping, so a lost device is still
//!   `DeviceLost`. No [`BackendError`] variant is added for fences.
//!
//! # `OUT_FENCE_PTR` is discovered but never requested
//!
//! `OUT_FENCE_PTR` takes the address of an `s32` in the caller's memory,
//! into which the kernel writes the number of a new `sync_file` descriptor.
//! Handing out that address is possible in safe Rust (an `AtomicI32` that
//! outlives the ioctl), but the descriptor number it receives can only be
//! turned into an [`OwnedFd`] with `OwnedFd::from_raw_fd`, which is
//! `unsafe`, and neither `drm` 0.15 nor `rustix` 1.x offers a safe
//! function that takes ownership of a raw descriptor. This crate is
//! `#![forbid(unsafe_code)]`. Requesting the out-fence without taking
//! ownership would leak one descriptor per commit, so the request is left
//! out. Page-flip events stay the latch, as before; the out-fence would
//! signal at the same point (§3.2) and matters only for exporting a release
//! fence to an external producer, which is not implemented (§3.4).
//!
//! Nothing here was run on hardware. The request building is tested on the
//! host against recorded property lists (`tests/fence_recorded.rs`); the
//! commit path has an ignored vkms test (`tests/vkms_fence.rs`).

use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};

use composition_core::{BackendError, BufferId, Frame, OutputId, Plan, PlaneId};
use drm::control::atomic::AtomicModeReq;
use drm::control::{property, AtomicCommitFlags, Device as ControlDevice, RawResourceHandle};

use crate::{class_of, named_properties, DrmBackend, DrmFailure, ErrnoClass, Operation};

/// Name of the plane property for an acquire fence.
pub const IN_FENCE_FD: &str = "IN_FENCE_FD";

/// Name of the CRTC property for a completion fence.
pub const OUT_FENCE_PTR: &str = "OUT_FENCE_PTR";

/// The id of the property called `name` among `(name, id)` pairs.
pub fn property_id<'a>(
    properties: impl IntoIterator<Item = (&'a str, u32)>,
    name: &str,
) -> Option<u32> {
    properties
        .into_iter()
        .find(|(n, _)| *n == name)
        .map(|(_, id)| id)
}

/// One property value that the fence table adds to an atomic request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FenceProperty {
    /// The KMS object: a plane id.
    pub object: u32,
    /// The property id (`IN_FENCE_FD`).
    pub property: u32,
    /// The raw value: a descriptor number.
    pub value: u64,
}

/// What a request is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Purpose {
    /// An atomic `TEST_ONLY` commit: descriptors are duplicated.
    Check,
    /// The real commit: the backend's own descriptors are used.
    Commit,
}

/// The fence properties of one request and the duplicates they refer to.
///
/// The duplicates of a [`Purpose::Check`] request are closed when this is
/// dropped, which must not happen before the ioctl.
#[derive(Debug, Default)]
pub struct InFences {
    properties: Vec<FenceProperty>,
    duplicates: Vec<OwnedFd>,
    buffers: Vec<BufferId>,
}

impl InFences {
    /// The properties to add, in assignment order.
    pub fn properties(&self) -> &[FenceProperty] {
        &self.properties
    }

    /// The fenced buffers the request shows, each once.
    pub fn buffers(&self) -> &[BufferId] {
        &self.buffers
    }

    /// The duplicated descriptors (only for [`Purpose::Check`]).
    pub fn duplicates(&self) -> &[OwnedFd] {
        &self.duplicates
    }

    /// Whether the request carries no fence.
    pub fn is_empty(&self) -> bool {
        self.properties.is_empty()
    }

    /// Adds the properties to `request`.
    pub fn apply(&self, request: &mut AtomicModeReq) {
        for p in &self.properties {
            let (Some(object), Some(prop)) = (
                RawResourceHandle::new(p.object),
                drm::control::from_u32::<property::Handle>(p.property),
            ) else {
                continue;
            };
            request.add_raw_property(object, prop, p.value);
        }
    }
}

/// Fence properties of the topology and the acquire fences of buffers.
///
/// Kept by [`DrmBackend`]; public so that the request building can be
/// tested on recorded topologies without a device.
#[derive(Debug, Default)]
pub struct FenceTable {
    planes: Vec<(PlaneId, Option<u32>)>,
    outputs: Vec<(OutputId, Option<u32>)>,
    acquire: Vec<(BufferId, OwnedFd)>,
}

impl FenceTable {
    /// An empty table: no plane, no output, no fence.
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets the planes and outputs; the acquire fences stay.
    pub fn clear_topology(&mut self) {
        self.planes.clear();
        self.outputs.clear();
    }

    /// Records the `IN_FENCE_FD` property id of `plane`, `None` if the
    /// plane does not list it.
    pub fn set_plane(&mut self, plane: PlaneId, in_fence_fd: Option<u32>) {
        match self.planes.iter_mut().find(|(p, _)| *p == plane) {
            Some(entry) => entry.1 = in_fence_fd,
            None => self.planes.push((plane, in_fence_fd)),
        }
    }

    /// Records the `OUT_FENCE_PTR` property id of `output`'s CRTC, `None`
    /// if the CRTC does not list it.
    pub fn set_output(&mut self, output: OutputId, out_fence_ptr: Option<u32>) {
        match self.outputs.iter_mut().find(|(o, _)| *o == output) {
            Some(entry) => entry.1 = out_fence_ptr,
            None => self.outputs.push((output, out_fence_ptr)),
        }
    }

    /// The `IN_FENCE_FD` property id of `plane`, if it lists one.
    pub fn plane_in_fence(&self, plane: PlaneId) -> Option<u32> {
        self.planes
            .iter()
            .find(|(p, _)| *p == plane)
            .and_then(|(_, id)| *id)
    }

    /// The `OUT_FENCE_PTR` property id of `output`'s CRTC, if it lists one.
    pub fn output_out_fence(&self, output: OutputId) -> Option<u32> {
        self.outputs
            .iter()
            .find(|(o, _)| *o == output)
            .and_then(|(_, id)| *id)
    }

    /// Attaches `fence` to `buffer`, returning the fence it replaces.
    pub fn set_acquire_fence(&mut self, buffer: BufferId, fence: OwnedFd) -> Option<OwnedFd> {
        match self.acquire.iter_mut().find(|(b, _)| *b == buffer) {
            Some(entry) => Some(std::mem::replace(&mut entry.1, fence)),
            None => {
                self.acquire.push((buffer, fence));
                None
            }
        }
    }

    /// Detaches and returns the fence of `buffer`.
    pub fn clear_acquire_fence(&mut self, buffer: BufferId) -> Option<OwnedFd> {
        let index = self.acquire.iter().position(|(b, _)| *b == buffer)?;
        Some(self.acquire.swap_remove(index).1)
    }

    /// The fence attached to `buffer`.
    pub fn acquire_fence(&self, buffer: BufferId) -> Option<BorrowedFd<'_>> {
        self.acquire
            .iter()
            .find(|(b, _)| *b == buffer)
            .map(|(_, fd)| fd.as_fd())
    }

    /// The `IN_FENCE_FD` values for a planned frame.
    ///
    /// Refuses with [`BackendError::Rejected`] if a fenced buffer is on a
    /// plane without `IN_FENCE_FD` or an assignment names a layer the frame
    /// does not have; with [`BackendError::Platform`] if a descriptor
    /// cannot be duplicated for a check.
    pub fn in_fences(
        &self,
        frame: &Frame<'_>,
        plan: &Plan,
        purpose: Purpose,
    ) -> Result<InFences, BackendError> {
        let mut fences = InFences::default();
        if self.acquire.is_empty() {
            return Ok(fences);
        }
        for assignment in plan.assignments() {
            let layer = frame
                .layers
                .get(assignment.layer)
                .ok_or(BackendError::Rejected)?;
            let Some(fence) = self.acquire_fence(layer.buffer) else {
                continue;
            };
            let property = self
                .plane_in_fence(assignment.plane)
                .ok_or(BackendError::Rejected)?;
            let value = match purpose {
                Purpose::Commit => fence.as_raw_fd(),
                Purpose::Check => {
                    let duplicate = fence.try_clone_to_owned().map_err(|error| {
                        BackendError::Platform(error.raw_os_error().unwrap_or(0))
                    })?;
                    let raw = duplicate.as_raw_fd();
                    fences.duplicates.push(duplicate);
                    raw
                }
            };
            fences.properties.push(FenceProperty {
                object: assignment.plane.0,
                property,
                value: value as u64,
            });
            if !fences.buffers.contains(&layer.buffer) {
                fences.buffers.push(layer.buffer);
            }
        }
        Ok(fences)
    }

    /// Closes the fences a successful commit used.
    pub fn committed(&mut self, fences: &InFences) {
        for buffer in &fences.buffers {
            self.clear_acquire_fence(*buffer);
        }
    }
}

impl DrmBackend {
    /// Looks up `IN_FENCE_FD` on the planes and `OUT_FENCE_PTR` on the
    /// CRTCs the backend drives.
    pub(crate) fn discover_fences(&mut self) -> io::Result<()> {
        self.fences.clear_topology();
        let ids = |props: &[(String, property::Info, u64)], name: &str| {
            property_id(
                props
                    .iter()
                    .map(|(n, info, _)| (n.as_str(), u32::from(info.handle()))),
                name,
            )
        };
        for plane in &self.planes {
            let props = named_properties(&self.card, plane.handle)?;
            self.fences
                .set_plane(plane.caps.id, ids(&props, IN_FENCE_FD));
        }
        for output in &self.outputs {
            let props = named_properties(&self.card, output.crtc)?;
            self.fences
                .set_output(OutputId(u32::from(output.crtc)), ids(&props, OUT_FENCE_PTR));
        }
        Ok(())
    }

    /// Whether `plane` lists `IN_FENCE_FD`, so that a buffer with an
    /// acquire fence can be shown on it.
    pub fn plane_in_fence_supported(&self, plane: PlaneId) -> bool {
        self.fences.plane_in_fence(plane).is_some()
    }

    /// Whether the CRTC of `output` lists `OUT_FENCE_PTR`. Reported only:
    /// the backend does not request it (see the [module](self) docs).
    pub fn output_out_fence_supported(&self, output: OutputId) -> bool {
        self.fences.output_out_fence(output).is_some()
    }

    /// Attaches a `sync_file` to an imported buffer: the next commit that
    /// shows the buffer sets `IN_FENCE_FD` on its plane to it, and the
    /// backend closes it after that commit succeeded. Replaces a fence
    /// attached before, which is closed.
    ///
    /// Fails with [`io::ErrorKind::NotFound`] if `buffer` is not imported.
    /// The descriptor is not checked to be a `sync_file`; the kernel does
    /// that at the check or commit.
    pub fn set_acquire_fence(&mut self, buffer: BufferId, fence: OwnedFd) -> io::Result<()> {
        if !self.buffers.iter().any(|b| b.id == buffer) {
            return Err(io::Error::new(io::ErrorKind::NotFound, "buffer"));
        }
        self.fences.set_acquire_fence(buffer, fence);
        Ok(())
    }

    /// Detaches the acquire fence of `buffer` and gives it back.
    pub fn clear_acquire_fence(&mut self, buffer: BufferId) -> Option<OwnedFd> {
        self.fences.clear_acquire_fence(buffer)
    }

    /// The acquire fence still attached to `buffer`.
    pub fn acquire_fence(&self, buffer: BufferId) -> Option<BorrowedFd<'_>> {
        self.fences.acquire_fence(buffer)
    }

    /// The fence properties and attached fences.
    pub fn fence_table(&self) -> &FenceTable {
        &self.fences
    }

    /// Adds the `IN_FENCE_FD` values for the frame to `request`.
    pub(crate) fn attach_fences(
        &self,
        request: &mut AtomicModeReq,
        frame: &Frame<'_>,
        plan: &Plan,
        purpose: Purpose,
    ) -> Result<InFences, BackendError> {
        let fences = self.fences.in_fences(frame, plan, purpose)?;
        fences.apply(request);
        Ok(fences)
    }

    /// Whether a refused request with fences passes without them; if it
    /// does, the fences are the cause, and the error is
    /// [`BackendError::Platform`] with `errno` rather than `Rejected`.
    pub(crate) fn fence_refusal(
        &mut self,
        fences: &InFences,
        frame: &Frame<'_>,
        plan: &Plan,
        operation: Operation,
        error: &io::Error,
        primary_used: bool,
    ) -> Option<BackendError> {
        let errno = error.raw_os_error().unwrap_or(0);
        if fences.is_empty() || class_of(errno) != ErrnoClass::Refused {
            return None;
        }
        let (request, modeset) = self.request(frame, plan).ok()?;
        let mut flags = AtomicCommitFlags::TEST_ONLY;
        if modeset {
            flags |= AtomicCommitFlags::ALLOW_MODESET;
        }
        self.card.atomic_commit(flags, request).ok()?;
        self.log_failure(DrmFailure::Refused {
            operation,
            errno,
            primary_unused: !primary_used,
        });
        Some(BackendError::Platform(errno))
    }
}
