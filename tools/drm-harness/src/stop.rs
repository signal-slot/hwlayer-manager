//! Stopping a run on SIGTERM or SIGINT.
//!
//! The handler is installed by `signal-hook`, which only sets a flag in the
//! handler, so this crate keeps `#![forbid(unsafe_code)]`. The scenarios poll
//! [`requested`] where they wait, and end with [`STOPPED`] as their error;
//! `run` turns that into the `resources` and `VERDICT` lines of a normal end.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

/// The error text of a scenario that was told to stop.
pub const STOPPED: &str = "stopped by signal";

static FLAG: OnceLock<Arc<AtomicBool>> = OnceLock::new();

fn flag() -> &'static Arc<AtomicBool> {
    FLAG.get_or_init(|| Arc::new(AtomicBool::new(false)))
}

/// Makes SIGTERM and SIGINT set the stop flag instead of killing the process.
pub fn install() -> std::io::Result<()> {
    for signal in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        signal_hook::flag::register(signal, Arc::clone(flag()))?;
    }
    Ok(())
}

/// Whether a stop was asked for.
pub fn requested() -> bool {
    flag().load(Ordering::Relaxed)
}

/// Asks for a stop without a signal (what the handler does).
#[cfg(test)]
pub fn request() {
    flag().store(true, Ordering::Relaxed);
}

/// `Err(STOPPED)` if a stop was asked for.
pub fn check() -> Result<(), String> {
    if requested() {
        Err(STOPPED.to_string())
    } else {
        Ok(())
    }
}

/// Calls `tick` (with the time left, at most `slice`) until `seconds` have
/// passed or `stopped` says so. Returns whether the whole time was held.
pub fn hold_for<E>(
    seconds: u64,
    slice: Duration,
    stopped: &dyn Fn() -> bool,
    mut tick: impl FnMut(Duration) -> Result<(), E>,
) -> Result<bool, E> {
    let deadline = Instant::now() + Duration::from_secs(seconds.min(MAX_SECONDS));
    loop {
        if stopped() {
            return Ok(false);
        }
        let now = Instant::now();
        if now >= deadline {
            return Ok(true);
        }
        tick((deadline - now).min(slice))?;
    }
}

/// Longest hold; larger `--seconds` would overflow the clock.
pub const MAX_SECONDS: u64 = 86_400;

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn hold_runs_its_time() {
        let ticks = Cell::new(0);
        let held = hold_for::<()>(0, Duration::from_millis(10), &|| false, |_| {
            ticks.set(ticks.get() + 1);
            Ok(())
        });
        assert_eq!(held, Ok(true));
        assert_eq!(ticks.get(), 0);
    }

    #[test]
    fn hold_ends_early_when_stopped() {
        let ticks = Cell::new(0);
        let started = Instant::now();
        let held = hold_for::<()>(3600, Duration::from_millis(1), &|| ticks.get() >= 3, |_| {
            ticks.set(ticks.get() + 1);
            Ok(())
        });
        assert_eq!(held, Ok(false));
        assert_eq!(ticks.get(), 3);
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn hold_passes_on_errors() {
        let held = hold_for(5, Duration::from_millis(1), &|| false, |_| Err("lost"));
        assert_eq!(held, Err("lost"));
    }

    #[test]
    fn huge_seconds_do_not_overflow_the_clock() {
        let held = hold_for::<()>(u64::MAX, Duration::from_millis(1), &|| true, |_| Ok(()));
        assert_eq!(held, Ok(false));
    }

    #[test]
    fn the_flag_is_what_the_handler_sets() {
        // The only test that touches the process-wide flag.
        assert!(check().is_ok());
        request();
        assert!(requested());
        assert_eq!(check(), Err(STOPPED.to_string()));
    }
}
