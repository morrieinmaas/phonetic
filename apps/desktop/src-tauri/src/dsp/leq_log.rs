//! Interval logging to disk.
//!
//! A noise log runs for hours and is evidence: each completed interval is
//! appended and flushed to disk as soon as it is recorded, never buffered
//! until close, so the log survives the app being killed mid-shift.
//!
//! The brief this was built from specified `record`'s timestamp as
//! `chrono::DateTime<Utc>`, but `chrono` is not a dependency of this crate
//! and the plan forbids adding one. `record` takes `std::time::SystemTime`
//! instead, and the CSV timestamp column is rendered with
//! [`super::format_utc`] -- the exact-civil-calendar routine that replaced
//! the old 365-day/30-day approximation. Do not reimplement date math here.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{format_utc, LeqMetrics};

/// The interval column matters: without it a row of LAeq values is ambiguous,
/// because a 1-second average and a 15-minute average look identical. It is a
/// per-row column rather than a header comment so that a log appended across
/// sessions with different intervals stays truthful.
const CSV_HEADER: &str =
    "timestamp,interval_seconds,leq_db,laeq_db,lceq_db,lafmax_db,lapeak_db,elapsed_seconds";

/// Appends completed LEQ intervals to a CSV file, on the command side --
/// never in the audio callback, where file I/O would be a dropout.
#[derive(Debug)]
pub struct LeqLogger {
    path: PathBuf,
    file: File,
    interval_seconds: f64,
}

impl LeqLogger {
    /// Opens `path` for appending, writing the header row only if the file
    /// is new or empty. An unwritable path fails here, with the path named
    /// in the message, rather than silently at the first `record`.
    pub fn new(path: PathBuf, interval_seconds: f64) -> Result<Self, String> {
        let has_header = path.metadata().map(|meta| meta.len() > 0).unwrap_or(false);

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|err| format!("cannot open LEQ log at {}: {err}", path.display()))?;

        if !has_header {
            writeln!(file, "{CSV_HEADER}")
                .map_err(|err| format!("cannot write header to {}: {err}", path.display()))?;
            file.flush()
                .map_err(|err| format!("cannot flush {}: {err}", path.display()))?;
        }

        Ok(Self {
            path,
            file,
            interval_seconds,
        })
    }

    /// Appends one completed interval and flushes it to disk immediately.
    pub fn record(&mut self, metrics: &LeqMetrics, timestamp: SystemTime) -> Result<(), String> {
        let secs_since_epoch = timestamp
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("timestamp before the Unix epoch: {err}"))?
            .as_secs();

        writeln!(
            self.file,
            "{},{:.3},{:.2},{:.2},{:.2},{:.2},{:.2},{:.3}",
            format_utc(secs_since_epoch),
            self.interval_seconds,
            metrics.leq_db,
            metrics.laeq_db,
            metrics.lceq_db,
            metrics.lafmax_db,
            metrics.lapeak_db,
            metrics.elapsed_seconds,
        )
        .map_err(|err| format!("cannot write to {}: {err}", self.path.display()))?;

        self.file
            .flush()
            .map_err(|err| format!("cannot flush {}: {err}", self.path.display()))?;

        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("phonetic-leq-{name}-{}.csv", std::process::id()));
        let _ = fs::remove_file(&p);
        p
    }

    fn sample_metrics(laeq: f32) -> LeqMetrics {
        LeqMetrics {
            leq_db: laeq,
            laeq_db: laeq,
            lceq_db: laeq,
            lafmax_db: laeq,
            lapeak_db: laeq,
            lcpeak_db: laeq,
            elapsed_seconds: 1.0,
        }
    }

    /// Each interval is on disk as soon as it is recorded, not at close: a log
    /// that only survives a clean shutdown is not a log.
    #[test]
    fn each_interval_is_readable_immediately() {
        let path = temp_path("immediate");
        let mut logger = LeqLogger::new(path.clone(), 1.0).unwrap();

        logger
            .record(&sample_metrics(85.0), SystemTime::now())
            .unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(
            contents.lines().count(),
            2,
            "header plus one row:\n{contents}"
        );

        logger
            .record(&sample_metrics(87.0), SystemTime::now())
            .unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(
            contents.lines().count(),
            3,
            "header plus two rows:\n{contents}"
        );

        let _ = fs::remove_file(&path);
    }

    /// The header names every metric, so the spreadsheet the log lands in is
    /// readable without the source.
    #[test]
    fn the_header_names_every_metric() {
        let path = temp_path("header");
        let _logger = LeqLogger::new(path.clone(), 1.0).unwrap();

        let header = fs::read_to_string(&path).unwrap();
        for column in [
            "timestamp",
            "leq_db",
            "laeq_db",
            "lceq_db",
            "lafmax_db",
            "lapeak_db",
        ] {
            assert!(header.contains(column), "header missing {column}: {header}");
        }
        let _ = fs::remove_file(&path);
    }

    /// Reopening an existing log appends rather than truncating — losing four
    /// hours of a shift to a restart is the failure mode this prevents.
    #[test]
    fn reopening_appends_rather_than_truncating() {
        let path = temp_path("append");
        {
            let mut logger = LeqLogger::new(path.clone(), 1.0).unwrap();
            logger
                .record(&sample_metrics(85.0), SystemTime::now())
                .unwrap();
        }
        {
            let mut logger = LeqLogger::new(path.clone(), 1.0).unwrap();
            logger
                .record(&sample_metrics(86.0), SystemTime::now())
                .unwrap();
        }

        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(
            contents.lines().count(),
            3,
            "expected one header, two rows:\n{contents}"
        );
        assert_eq!(
            contents.lines().filter(|l| l.contains("timestamp")).count(),
            1,
            "header written twice"
        );
        let _ = fs::remove_file(&path);
    }

    /// An unwritable path fails at construction with a message naming the path,
    /// not silently at the first record three hours in.
    #[test]
    fn an_unwritable_path_fails_at_construction() {
        let result = LeqLogger::new("/nonexistent-dir-xyz/log.csv".into(), 1.0);
        assert!(result.is_err());
        let message = result.unwrap_err();
        assert!(
            message.contains("nonexistent-dir-xyz"),
            "unhelpful error: {message}"
        );
    }

    /// Each row carries the integration interval it represents. Without it a
    /// column of LAeq values is ambiguous evidence: a 1-second average and a
    /// 15-minute average are indistinguishable on the page.
    #[test]
    fn each_row_states_the_interval_it_covers() {
        let path = temp_path("interval-column");
        let mut logger = LeqLogger::new(path.clone(), 60.0).unwrap();
        logger
            .record(&sample_metrics(85.0), SystemTime::now())
            .unwrap();

        let contents = fs::read_to_string(&path).unwrap();
        let mut lines = contents.lines();
        let header = lines.next().expect("header");
        let row = lines.next().expect("row");

        let interval_column = header
            .split(',')
            .position(|column| column == "interval_seconds")
            .expect("header names interval_seconds");

        let value: f64 = row
            .split(',')
            .nth(interval_column)
            .expect("row has an interval cell")
            .parse()
            .expect("interval parses as a number");
        assert!((value - 60.0).abs() < 1e-6, "row reported interval {value}");

        let _ = fs::remove_file(&path);
    }
}
