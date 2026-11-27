//! Measurement: duration statistics, frame pacing and process resources.

use std::time::{Duration, Instant};

use crate::record::Record;

/// Statistics of a set of samples, in microseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Summary {
    /// Number of samples.
    pub count: usize,
    /// Smallest sample.
    pub min: u64,
    /// Nearest-rank 50th percentile (the lower middle for an even count).
    pub median: u64,
    /// Nearest-rank 99th percentile.
    pub p99: u64,
    /// Largest sample.
    pub max: u64,
    /// Arithmetic mean, rounded down.
    pub mean: u64,
}

/// Nearest-rank percentile of sorted samples: the smallest sample such that
/// at least `percent` % of the samples are not larger.
pub fn percentile(sorted: &[u64], percent: u32) -> Option<u64> {
    if sorted.is_empty() {
        return None;
    }
    let n = sorted.len() as u64;
    let rank = (u64::from(percent.min(100)) * n).div_ceil(100).max(1);
    Some(sorted[(rank - 1) as usize])
}

/// Summarizes samples given in microseconds. `None` if there are none.
pub fn summarize(samples: &[u64]) -> Option<Summary> {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let sum: u128 = sorted.iter().map(|&s| u128::from(s)).sum();
    Some(Summary {
        count: sorted.len(),
        min: *sorted.first()?,
        median: percentile(&sorted, 50)?,
        p99: percentile(&sorted, 99)?,
        max: *sorted.last()?,
        mean: (sum / sorted.len() as u128) as u64,
    })
}

impl Summary {
    /// Adds the summary to a record as `<prefix>_min_us=...` and so on.
    pub fn fields(&self, record: Record, prefix: &str) -> Record {
        record
            .field(&format!("{prefix}_count"), self.count)
            .field(&format!("{prefix}_min_us"), self.min)
            .field(&format!("{prefix}_median_us"), self.median)
            .field(&format!("{prefix}_p99_us"), self.p99)
            .field(&format!("{prefix}_max_us"), self.max)
            .field(&format!("{prefix}_mean_us"), self.mean)
    }
}

/// Adds the summary of `samples`, or `<prefix>_count=0`.
pub fn summary_fields(record: Record, prefix: &str, samples: &[u64]) -> Record {
    match summarize(samples) {
        Some(summary) => summary.fields(record, prefix),
        None => record.field(&format!("{prefix}_count"), 0),
    }
}

/// Duration in whole microseconds, saturating.
pub fn micros(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

/// Refresh period in microseconds for a refresh rate in millihertz.
pub fn period_us(refresh_mhz: u32) -> Option<u64> {
    (refresh_mhz > 0).then(|| 1_000_000_000 / u64::from(refresh_mhz))
}

/// Vertical blanks that passed without a new frame between two latches
/// `interval_us` apart: the interval in whole refresh periods, rounded to
/// the nearest, minus the one expected.
pub fn missed_vblanks(interval_us: u64, period_us: u64) -> u64 {
    if period_us == 0 {
        return 0;
    }
    ((interval_us + period_us / 2) / period_us).saturating_sub(1)
}

/// CPU times of a process, in clock ticks, from `/proc/<pid>/stat`.
///
/// The command name in field 2 may contain spaces and parentheses, so the
/// fields are counted from the last `)`.
pub fn parse_stat_times(stat: &str) -> Option<(u64, u64)> {
    let rest = &stat[stat.rfind(')')? + 1..];
    let mut fields = rest.split_whitespace();
    // After the name come state (field 3) ... utime (14) and stime (15).
    let utime = fields.nth(11)?.parse().ok()?;
    let stime = fields.next()?.parse().ok()?;
    Some((utime, stime))
}

/// A `kB` value of `/proc/<pid>/status`, such as `VmHWM`.
pub fn parse_status_kib(status: &str, key: &str) -> Option<u64> {
    status.lines().find_map(|line| {
        let value = line.strip_prefix(key)?.strip_prefix(':')?;
        let mut parts = value.split_whitespace();
        let number = parts.next()?.parse().ok()?;
        (parts.next() == Some("kB")).then_some(number)
    })
}

/// Process resource usage at one point in time.
#[derive(Clone, Copy, Debug)]
pub struct Usage {
    at: Instant,
    /// User CPU time in microseconds, if known.
    user_us: Option<u64>,
    /// System CPU time in microseconds, if known.
    system_us: Option<u64>,
}

impl Usage {
    /// Reads the current process's usage.
    pub fn now() -> Self {
        let ticks = rustix::param::clock_ticks_per_second();
        let times = std::fs::read_to_string("/proc/self/stat")
            .ok()
            .and_then(|s| parse_stat_times(&s));
        let to_us = |t: u64| (ticks > 0).then(|| t * 1_000_000 / ticks);
        Self {
            at: Instant::now(),
            user_us: times.and_then(|(u, _)| to_us(u)),
            system_us: times.and_then(|(_, s)| to_us(s)),
        }
    }

    /// Adds the usage since `self` and the memory figures to a record.
    pub fn fields_since(&self, record: Record) -> Record {
        let now = Usage::now();
        let wall_us = micros(now.at - self.at);
        let user = now.user_us.zip(self.user_us).map(|(b, a)| b - a);
        let system = now.system_us.zip(self.system_us).map(|(b, a)| b - a);
        let cpu_pct = user
            .zip(system)
            .filter(|_| wall_us > 0)
            .map(|(u, s)| format!("{:.1}", (u + s) as f64 * 100.0 / wall_us as f64));
        let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
        record
            .field("wall_ms", wall_us / 1000)
            .field_opt("cpu_user_ms", user.map(|u| u / 1000))
            .field_opt("cpu_sys_ms", system.map(|s| s / 1000))
            .field_opt("cpu_pct", cpu_pct)
            .field_opt("rss_kib", parse_status_kib(&status, "VmRSS"))
            .field_opt("peak_rss_kib", parse_status_kib(&status, "VmHWM"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles() {
        let sorted: Vec<u64> = (1..=100).collect();
        assert_eq!(percentile(&sorted, 50), Some(50));
        assert_eq!(percentile(&sorted, 99), Some(99));
        assert_eq!(percentile(&sorted, 100), Some(100));
        assert_eq!(percentile(&sorted, 0), Some(1));
        assert_eq!(percentile(&[7], 99), Some(7));
        assert_eq!(percentile(&[], 50), None);
        // Ten samples: p99 is the largest, the median the lower middle.
        let ten: Vec<u64> = (1..=10).collect();
        assert_eq!(percentile(&ten, 99), Some(10));
        assert_eq!(percentile(&ten, 50), Some(5));
    }

    #[test]
    fn summaries() {
        assert_eq!(summarize(&[]), None);
        let s = summarize(&[30, 10, 20, 1000]).unwrap();
        assert_eq!(
            s,
            Summary {
                count: 4,
                min: 10,
                median: 20,
                p99: 1000,
                max: 1000,
                mean: 265,
            }
        );
        let r = s.fields(Record::new("latency"), "latch");
        assert_eq!(
            r.to_string(),
            "latency latch_count=4 latch_min_us=10 latch_median_us=20 latch_p99_us=1000 \
             latch_max_us=1000 latch_mean_us=265"
        );
        assert_eq!(
            summary_fields(Record::new("x"), "v", &[]).to_string(),
            "x v_count=0"
        );
    }

    #[test]
    fn pacing() {
        assert_eq!(period_us(60_000), Some(16_666));
        assert_eq!(period_us(0), None);
        let p = 16_666;
        assert_eq!(missed_vblanks(16_700, p), 0);
        assert_eq!(missed_vblanks(16_000, p), 0);
        assert_eq!(missed_vblanks(24_000, p), 0);
        assert_eq!(missed_vblanks(26_000, p), 1);
        assert_eq!(missed_vblanks(33_300, p), 1);
        assert_eq!(missed_vblanks(50_000, p), 2);
        assert_eq!(missed_vblanks(100, p), 0);
        assert_eq!(missed_vblanks(100, 0), 0);
    }

    #[test]
    fn proc_stat() {
        let stat = "1234 (drm harness (x)) S 1 1234 1234 0 -1 4194560 120 0 0 0 \
                    42 17 0 0 20 0 1 0 5000 1000000 300 18446744073709551615";
        assert_eq!(parse_stat_times(stat), Some((42, 17)));
        assert_eq!(parse_stat_times("garbage"), None);
        assert_eq!(parse_stat_times("1 (x) S 1 2"), None);
    }

    #[test]
    fn proc_status() {
        let status = "Name:\tdrm-harness\nVmPeak:\t   10000 kB\nVmHWM:\t    5120 kB\n\
                      VmRSS:\t    4096 kB\nThreads:\t1\n";
        assert_eq!(parse_status_kib(status, "VmHWM"), Some(5120));
        assert_eq!(parse_status_kib(status, "VmRSS"), Some(4096));
        assert_eq!(parse_status_kib(status, "Threads"), None);
        assert_eq!(parse_status_kib(status, "VmSwap"), None);
        // A key that is a prefix of another is not confused with it.
        assert_eq!(parse_status_kib(status, "Vm"), None);
    }
}
