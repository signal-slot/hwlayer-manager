//! Where the DRM file descriptor driven by the engine comes from.
//!
//! - [`Source::Master`]: a device node, used as DRM master.
//! - [`Source::KernelLease`]: a lease this process creates as lessor with
//!   the kernel's lease ioctls; the engine gets only the lease descriptor.
//! - [`Source::Dlm`]: a lease received from AGL drm-lease-manager (or from
//!   `drm-harness lessor`, which speaks the same protocol) over its socket.
//!
//! Everything after [`Source::open`] works on the descriptor alone.

use std::fs::OpenOptions;
use std::io;
use std::os::fd::OwnedFd;
use std::path::PathBuf;

use crate::lease::{LeaseRequest, Lessor};
use crate::record::Record;
use crate::wire::DlmLease;

/// A source of a DRM file descriptor.
#[derive(Clone, Debug)]
pub enum Source {
    /// A device node such as `/dev/dri/card0`, used as DRM master.
    Master {
        /// Path of the device node.
        device: PathBuf,
    },
    /// A kernel DRM lease made in this process.
    KernelLease(LeaseRequest),
    /// A lease from drm-lease-manager's socket.
    Dlm {
        /// The lease's socket, `<runtime dir>/<lease name>`.
        socket: PathBuf,
    },
}

/// What has to stay alive while the descriptor is used.
pub enum Holder {
    /// Nothing.
    None,
    /// The lessor: DRM master of the device, can revoke the lease.
    Lessor(Box<Lessor>),
    /// The connection to the lease manager; dropping it releases the lease.
    Dlm {
        /// Only held.
        _connection: DlmLease,
    },
}

/// An opened source.
pub struct Opened {
    /// The descriptor for the engine.
    pub fd: OwnedFd,
    /// What keeps it valid.
    pub holder: Holder,
}

impl Source {
    /// Opens the descriptor the engine will drive.
    pub fn open(&self) -> io::Result<Opened> {
        match self {
            Source::Master { device } => {
                let file = OpenOptions::new().read(true).write(true).open(device)?;
                Ok(Opened {
                    fd: file.into(),
                    holder: Holder::None,
                })
            }
            Source::KernelLease(request) => {
                let mut lessor = Lessor::open(request)?;
                let fd = lessor.lease()?;
                Ok(Opened {
                    fd,
                    holder: Holder::Lessor(Box::new(lessor)),
                })
            }
            Source::Dlm { socket } => {
                let (lease, fd) = DlmLease::get(socket)?;
                Record::new("lease")
                    .field("route", "drm-lease-manager")
                    .field("socket", lease.path().display())
                    .emit();
                Ok(Opened {
                    fd,
                    holder: Holder::Dlm { _connection: lease },
                })
            }
        }
    }

    /// Short name of the kind of source, for the records.
    pub fn kind(&self) -> &'static str {
        match self {
            Source::Master { .. } => "master",
            Source::KernelLease(_) => "kernel-lease",
            Source::Dlm { .. } => "dlm",
        }
    }

    /// What was opened, for the records.
    pub fn describe(&self) -> String {
        match self {
            Source::Master { device } => device.display().to_string(),
            Source::KernelLease(request) => request.device.display().to_string(),
            Source::Dlm { socket } => socket.display().to_string(),
        }
    }

    /// Whether the descriptor must be made DRM master after opening. A
    /// lessee is master of its lease already.
    pub fn needs_master(&self) -> bool {
        matches!(self, Source::Master { .. })
    }
}
