//! The formatter: one line per entry, into a [`core::fmt::Write`].

use core::fmt::{self, Write};

use composition_core::{
    Assignment, BackendError, DropReason, Error, PlanError, PresentError, Rect, TraceEvent,
};

use crate::{is_quotable, is_word, Header, Stamp, PREFIX, VERSION};

/// Writes the `hdr` line with line counter `seq`, ending in `\n`.
///
/// Fails without writing anything if `freq_hz` is 0, `at_unit` is not one
/// word, or `build` or `board` holds a `"` or a character outside printable
/// ASCII.
pub fn write_header<W: Write + ?Sized>(w: &mut W, seq: u64, header: &Header<'_>) -> fmt::Result {
    if header.freq_hz == 0
        || !is_word(header.at_unit)
        || !is_quotable(header.build)
        || !is_quotable(header.board)
    {
        return Err(fmt::Error);
    }
    writeln!(
        w,
        "{PREFIX} v={VERSION} ev=hdr seq={seq} src={} clock={} freq_hz={} t0={} at_unit={} build=\"{}\" board=\"{}\"",
        header.src.as_str(),
        header.clock.as_str(),
        header.freq_hz,
        header.t0,
        header.at_unit,
        header.build,
        header.board,
    )
}

/// Writes the line of one trace entry, ending in `\n`.
///
/// Fails without writing anything if `stamp.tev` is set on an event other
/// than [`TraceEvent::Latched`], or `stamp.dur` on one other than
/// [`TraceEvent::Validate`] and [`TraceEvent::Commit`].
pub fn write_event<W: Write + ?Sized>(
    w: &mut W,
    stamp: &Stamp,
    event: &TraceEvent<'_>,
) -> fmt::Result {
    let tev_allowed = matches!(event, TraceEvent::Latched { .. });
    let dur_allowed = matches!(
        event,
        TraceEvent::Validate { .. } | TraceEvent::Commit { .. }
    );
    if (stamp.tev.is_some() && !tev_allowed) || (stamp.dur.is_some() && !dur_allowed) {
        return Err(fmt::Error);
    }

    write!(
        w,
        "{PREFIX} v={VERSION} ev={} seq={} t={} at={}",
        name(event),
        stamp.seq,
        stamp.t,
        stamp.at
    )?;
    match *event {
        TraceEvent::Check {
            output,
            assignments,
            result,
        } => {
            write!(w, " out={}", output.0)?;
            write_assignments(w, assignments)?;
            match result {
                Ok(()) => w.write_str(" result=ok")?,
                Err(e) => {
                    w.write_str(" result=")?;
                    write_backend_error(w, e)?;
                }
            }
        }
        TraceEvent::Validate {
            output,
            layers,
            result,
        } => {
            write!(w, " out={} layers={layers}", output.0)?;
            match result {
                Ok(assignments) => {
                    write_assignments(w, assignments)?;
                    w.write_str(" result=ok")?;
                }
                Err(e) => write_error(w, e)?,
            }
        }
        TraceEvent::Commit { output, result } => {
            write!(w, " out={}", output.0)?;
            match result {
                Ok(sub) => write!(w, " result=ok sub={}", sub.0)?,
                Err(e) => write_error(w, e)?,
            }
        }
        TraceEvent::Latched { output, submission } => {
            write!(w, " out={} sub={}", output.0, submission.0)?;
        }
        TraceEvent::Released { buffer, submission } => {
            write!(w, " buf={}", buffer.0)?;
            match submission {
                Some(sub) => write!(w, " sub={}", sub.0)?,
                None => w.write_str(" sub=-")?,
            }
        }
        TraceEvent::Dropped {
            output,
            submission,
            reason,
        } => {
            let reason = match reason {
                DropReason::Replaced => "replaced",
                DropReason::DeviceLost => "device_lost",
            };
            write!(w, " out={} sub={} reason={reason}", output.0, submission.0)?;
        }
        TraceEvent::LatchTimeout { output, submission } => {
            write!(w, " out={} sub={}", output.0, submission.0)?;
        }
        TraceEvent::DeviceLost | TraceEvent::CapabilitiesChanged => {}
    }
    if let Some(tev) = stamp.tev {
        write!(w, " tev={tev}")?;
    }
    if let Some(dur) = stamp.dur {
        write!(w, " dur={dur}")?;
    }
    w.write_char('\n')
}

/// The `ev=` name of an event.
pub(crate) fn name(event: &TraceEvent<'_>) -> &'static str {
    match event {
        TraceEvent::Check { .. } => "check",
        TraceEvent::Validate { .. } => "validate",
        TraceEvent::Commit { .. } => "commit",
        TraceEvent::Latched { .. } => "latched",
        TraceEvent::Released { .. } => "released",
        TraceEvent::Dropped { .. } => "dropped",
        TraceEvent::LatchTimeout { .. } => "latch_timeout",
        TraceEvent::DeviceLost => "device_lost",
        TraceEvent::CapabilitiesChanged => "caps_changed",
    }
}

/// ` n=<count> assign=<list>`; an empty list is written `assign=-`.
fn write_assignments<W: Write + ?Sized>(w: &mut W, assignments: &[Assignment]) -> fmt::Result {
    write!(w, " n={} assign=", assignments.len())?;
    if assignments.is_empty() {
        return w.write_char('-');
    }
    for (i, a) in assignments.iter().enumerate() {
        if i > 0 {
            w.write_char(';')?;
        }
        write!(w, "{}:{}:{}:{}:", a.layer, a.plane.0, a.plane_index, a.zpos)?;
        write_rect(w, a.source)?;
        w.write_char(':')?;
        write_rect(w, a.destination)?;
    }
    Ok(())
}

fn write_rect<W: Write + ?Sized>(w: &mut W, r: Rect) -> fmt::Result {
    write!(w, "{},{},{},{}", r.x, r.y, r.width, r.height)
}

/// The code of a [`BackendError`] and its extra fields, after `result=` or
/// `result=backend.`.
fn write_backend_error<W: Write + ?Sized>(w: &mut W, e: BackendError) -> fmt::Result {
    match e {
        BackendError::Rejected => w.write_str("rejected"),
        BackendError::Busy => w.write_str("busy"),
        BackendError::UnknownBuffer(b) => write!(w, "unknown_buffer buf={}", b.0),
        BackendError::DeviceLost => w.write_str("device_lost"),
        BackendError::Platform(errno) => write!(w, "platform errno={errno}"),
    }
}

/// ` result=<code>` and the code's extra fields for an engine [`Error`].
fn write_error<W: Write + ?Sized>(w: &mut W, e: Error) -> fmt::Result {
    w.write_str(" result=")?;
    match e {
        Error::StalePlan => w.write_str("stale_plan"),
        Error::PlanMismatch => w.write_str("plan_mismatch"),
        Error::Backend(e) => {
            w.write_str("backend.")?;
            write_backend_error(w, e)
        }
        Error::Plan(e) => match e {
            PlanError::UnknownOutput(o) => write!(w, "plan.unknown_output err_out={}", o.0),
            PlanError::TooManyLayers { count } => write!(w, "plan.too_many_layers count={count}"),
            PlanError::UnknownBuffer { layer, buffer } => {
                write!(w, "plan.unknown_buffer layer={layer} buf={}", buffer.0)
            }
            PlanError::InvalidSource { layer } => write!(w, "plan.invalid_source layer={layer}"),
            PlanError::InvalidDestination { layer } => {
                write!(w, "plan.invalid_destination layer={layer}")
            }
            PlanError::CannotClip { layer } => write!(w, "plan.cannot_clip layer={layer}"),
            PlanError::NoPlane { layer, reasons } => {
                write!(
                    w,
                    "plan.no_plane layer={layer} reasons={:#x}",
                    reasons.bits()
                )
            }
            PlanError::BackendRejected { candidates } => {
                write!(w, "plan.backend_rejected candidates={candidates}")
            }
            PlanError::LimitExceeded => w.write_str("plan.limit_exceeded"),
            PlanError::Aborted => w.write_str("plan.aborted"),
        },
        Error::Present(e) => match e {
            PresentError::UnknownBuffer(b) => write!(w, "present.unknown_buffer buf={}", b.0),
            PresentError::DuplicateBuffer(b) => write!(w, "present.duplicate_buffer buf={}", b.0),
            PresentError::BufferTableFull => w.write_str("present.buffer_table_full"),
            PresentError::BufferInUse(b) => write!(w, "present.buffer_in_use buf={}", b.0),
            PresentError::BufferNotReady(b) => write!(w, "present.buffer_not_ready buf={}", b.0),
            PresentError::BufferNotWritable(b) => {
                write!(w, "present.buffer_not_writable buf={}", b.0)
            }
            PresentError::Busy(o) => write!(w, "present.busy err_out={}", o.0),
            PresentError::TooManyOutputs => w.write_str("present.too_many_outputs"),
            PresentError::TooManyBuffers => w.write_str("present.too_many_buffers"),
            PresentError::EventQueueFull => w.write_str("present.event_queue_full"),
            PresentError::UnexpectedLatch { output, submission } => write!(
                w,
                "present.unexpected_latch err_out={} err_sub={}",
                output.0, submission.0
            ),
        },
    }
}
