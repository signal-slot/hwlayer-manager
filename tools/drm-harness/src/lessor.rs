//! `drm-harness lessor`: a small leaser that hands kernel DRM leases to
//! other processes, speaking drm-lease-manager's client protocol on a unix
//! socket (see `wire.rs`). It lets `--source dlm` be tried without
//! drm-lease-manager, and shows the lease descriptor crossing a process
//! boundary with `SCM_RIGHTS`.

use std::os::fd::AsFd;
use std::path::PathBuf;

use crate::lease::{LeaseRequest, Lessor};
use crate::record::Record;
use crate::wire::{self, DlmRequest};

/// Settings of `drm-harness lessor`.
#[derive(Clone, Debug)]
pub struct LessorOptions {
    /// What to lease.
    pub request: LeaseRequest,
    /// The socket to serve.
    pub socket: PathBuf,
    /// Exit after the first client.
    pub once: bool,
}

/// Serves leases until killed (or after one client with `once`).
pub fn run_lessor(options: &LessorOptions) -> Result<(), String> {
    let mut lessor = Lessor::open(&options.request).map_err(|e| format!("lessor: {e}"))?;
    let listener = wire::listen(&options.socket)
        .map_err(|e| format!("listen on {}: {e}", options.socket.display()))?;
    Record::new("lessor")
        .field("pid", std::process::id())
        .field("device", lessor.device().display())
        .field("socket", options.socket.display())
        .field("connector", &lessor.leased.connector_name)
        .field(
            "planes",
            crate::record::list(lessor.leased.plane_ids().iter()),
        )
        .emit();
    let mut clients = 0u32;
    loop {
        let connection = wire::accept(listener.as_fd()).map_err(|e| format!("accept: {e}"))?;
        clients += 1;
        Record::new("lessor_client").field("n", clients).emit();
        loop {
            match wire::dlm_read_request(connection.as_fd()) {
                Ok(Some(Some(DlmRequest::GetLease))) => {
                    if lessor.holds_lease() {
                        // One lease at a time: refuse by closing, as
                        // drm-lease-manager does when it cannot serve.
                        Record::new("lessor_refused")
                            .field("n", clients)
                            .field("why", "lease already held")
                            .emit();
                        break;
                    }
                    match lessor.lease() {
                        Ok(fd) => {
                            if let Err(e) = wire::dlm_send_lease(connection.as_fd(), fd.as_fd()) {
                                Record::new("lessor_error")
                                    .field("send", e.to_string())
                                    .emit();
                                break;
                            }
                        }
                        Err(e) => {
                            Record::new("lessor_refused")
                                .field("n", clients)
                                .field("why", e.to_string())
                                .emit();
                            break;
                        }
                    }
                }
                Ok(Some(Some(DlmRequest::ReleaseLease))) | Ok(None) => break,
                Ok(Some(None)) => {
                    Record::new("lessor_error")
                        .field("request", "unknown opcode, ignored")
                        .emit();
                }
                Err(e) => {
                    Record::new("lessor_error")
                        .field("receive", e.to_string())
                        .emit();
                    break;
                }
            }
        }
        lessor.end().map_err(|e| format!("revoke: {e}"))?;
        drop(connection);
        if options.once {
            let _ = std::fs::remove_file(&options.socket);
            return Ok(());
        }
    }
}
