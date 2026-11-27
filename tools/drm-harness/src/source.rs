//! Where the DRM file descriptor driven by the engine comes from.
//!
//! Only a device node opened as DRM master exists today. Other sources (a
//! DRM lease, a lease-manager client) are meant to be added as variants of
//! [`Source`]: everything after [`Source::open`] works on the descriptor
//! alone.

use std::fs::OpenOptions;
use std::io;
use std::os::fd::OwnedFd;
use std::path::PathBuf;

/// A source of a DRM file descriptor.
#[derive(Clone, Debug)]
pub enum Source {
    /// A device node such as `/dev/dri/card0`, used as DRM master.
    Master {
        /// Path of the device node.
        device: PathBuf,
    },
}

impl Source {
    /// Opens the descriptor the engine will drive.
    pub fn open(&self) -> io::Result<OwnedFd> {
        match self {
            Source::Master { device } => {
                let file = OpenOptions::new().read(true).write(true).open(device)?;
                Ok(file.into())
            }
        }
    }

    /// Short name of the kind of source, for the records.
    pub fn kind(&self) -> &'static str {
        match self {
            Source::Master { .. } => "master",
        }
    }

    /// What was opened, for the records.
    pub fn describe(&self) -> String {
        match self {
            Source::Master { device } => device.display().to_string(),
        }
    }

    /// Whether the descriptor must be made DRM master after opening.
    pub fn needs_master(&self) -> bool {
        matches!(self, Source::Master { .. })
    }
}
