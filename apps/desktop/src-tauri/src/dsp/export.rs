//! Measurement Export
//!
//! Export measurement data to various formats:
//! - CSV (universal spreadsheet format)
//! - JSON (for programmatic access)
//! - TXT (SMAART-compatible format)

use serde::{Deserialize, Serialize};
use std::io::Write;

use super::{ImpulseResponseData, TransferFunctionData};

/// Export format options
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Csv,
    Json,
    Txt,
}

/// Measurement metadata for export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasurementMetadata {
    pub name: String,
    pub date: String,
    pub sample_rate: f32,
    pub fft_size: usize,
    pub averaging_count: usize,
    pub notes: Option<String>,
}

impl Default for MeasurementMetadata {
    fn default() -> Self {
        Self {
            name: "Measurement".to_string(),
            date: chrono_lite_now(),
            sample_rate: 48000.0,
            fft_size: 2048,
            averaging_count: 1,
            notes: None,
        }
    }
}

/// Format a UTC timestamp from seconds since the Unix epoch, as `YYYY-MM-DD HH:MM:SS`.
///
/// The previous implementation here assumed 365-day years and 30-day months,
/// which put the wrong date on every exported measurement — on 2026-09-08 it
/// reported 2026-09-25. Measurements get filed and referred back to, so the
/// date has to be right.
///
/// This is the standard civil-from-days conversion (Howard Hinnant's
/// `civil_from_days`), exact for the whole range and no dependency needed.
pub fn format_utc(secs_since_epoch: u64) -> String {
    let days = (secs_since_epoch / 86400) as i64;
    let time_of_day = secs_since_epoch % 86400;

    // Shift the epoch to 0000-03-01 so leap day lands at the end of the cycle.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // day of era, [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11], March-based
    let day = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = yoe + era * 400 + i64::from(month <= 2);

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        year,
        month,
        day,
        time_of_day / 3600,
        (time_of_day % 3600) / 60,
        time_of_day % 60
    )
}

/// Current UTC date/time as a string.
fn chrono_lite_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    format_utc(secs)
}

/// Export transfer function data to CSV
pub fn export_transfer_function_csv(
    data: &TransferFunctionData,
    metadata: &MeasurementMetadata,
) -> String {
    let mut output = String::new();

    // Header comments
    output.push_str("# Phonetic Transfer Function Measurement\n");
    output.push_str(&format!("# Name: {}\n", metadata.name));
    output.push_str(&format!("# Date: {}\n", metadata.date));
    output.push_str(&format!("# Sample Rate: {} Hz\n", metadata.sample_rate));
    output.push_str(&format!("# FFT Size: {}\n", metadata.fft_size));
    output.push_str(&format!(
        "# Averaging: {} frames\n",
        metadata.averaging_count
    ));
    if let Some(ref notes) = metadata.notes {
        output.push_str(&format!("# Notes: {}\n", notes));
    }
    output.push('\n');

    // Column headers
    output.push_str("Frequency (Hz),Magnitude (dB),Phase (deg),Coherence,Group Delay (ms)\n");

    // Data rows
    let len = data.frequencies.len();
    for i in 0..len {
        let freq = data.frequencies.get(i).copied().unwrap_or(0.0);
        let mag = data.magnitude_db.get(i).copied().unwrap_or(-120.0);
        let phase = data.phase_deg.get(i).copied().unwrap_or(0.0);
        let coherence = data.coherence.get(i).copied().unwrap_or(0.0);
        let group_delay = data.group_delay_ms.get(i).copied().unwrap_or(0.0);

        output.push_str(&format!(
            "{:.2},{:.2},{:.2},{:.4},{:.4}\n",
            freq, mag, phase, coherence, group_delay
        ));
    }

    output
}

/// Export transfer function data to JSON
pub fn export_transfer_function_json(
    data: &TransferFunctionData,
    metadata: &MeasurementMetadata,
) -> Result<String, serde_json::Error> {
    #[derive(Serialize)]
    struct ExportData<'a> {
        metadata: &'a MeasurementMetadata,
        data: &'a TransferFunctionData,
    }

    let export = ExportData { metadata, data };
    serde_json::to_string_pretty(&export)
}

/// Export transfer function data to SMAART-compatible TXT format
pub fn export_transfer_function_txt(
    data: &TransferFunctionData,
    metadata: &MeasurementMetadata,
) -> String {
    let mut output = String::new();

    // SMAART-style header
    output.push_str("* Phonetic Transfer Function Export\n");
    output.push_str(&format!("* {}\n", metadata.name));
    output.push_str(&format!("* {}\n", metadata.date));
    output.push_str(&format!("* Sample Rate: {} Hz\n", metadata.sample_rate));
    output.push('\n');

    // Data: frequency (tab) magnitude
    let len = data.frequencies.len();
    for i in 0..len {
        let freq = data.frequencies.get(i).copied().unwrap_or(0.0);
        let mag = data.magnitude_db.get(i).copied().unwrap_or(-120.0);

        output.push_str(&format!("{:.2}\t{:.2}\n", freq, mag));
    }

    output
}

/// Export impulse response data to CSV
pub fn export_impulse_response_csv(
    data: &ImpulseResponseData,
    metadata: &MeasurementMetadata,
) -> String {
    let mut output = String::new();

    // Header comments
    output.push_str("# Phonetic Impulse Response Measurement\n");
    output.push_str(&format!("# Name: {}\n", metadata.name));
    output.push_str(&format!("# Date: {}\n", metadata.date));
    output.push_str(&format!("# Sample Rate: {} Hz\n", data.sample_rate));
    output.push_str(&format!("# Peak Value: {:.6}\n", data.peak_value));
    output.push_str(&format!("# Peak Time: {:.2} ms\n", data.peak_time_ms));
    if let Some(rt60) = data.rt60_ms {
        output.push_str(&format!("# RT60: {:.0} ms\n", rt60));
    }
    output.push('\n');

    // Column headers
    output.push_str("Time (ms),Amplitude,EDC (dB)\n");

    // Data rows
    let len = data.samples.len();
    for i in 0..len {
        let time = data.time_ms.get(i).copied().unwrap_or(0.0);
        let amplitude = data.samples.get(i).copied().unwrap_or(0.0);
        let edc = data.edc_db.get(i).copied().unwrap_or(-120.0);

        output.push_str(&format!("{:.4},{:.6},{:.2}\n", time, amplitude, edc));
    }

    output
}

/// Export impulse response data to JSON
pub fn export_impulse_response_json(
    data: &ImpulseResponseData,
    metadata: &MeasurementMetadata,
) -> Result<String, serde_json::Error> {
    #[derive(Serialize)]
    struct ExportData<'a> {
        metadata: &'a MeasurementMetadata,
        data: &'a ImpulseResponseData,
    }

    let export = ExportData { metadata, data };
    serde_json::to_string_pretty(&export)
}

/// Export impulse response data to WAV-compatible TXT (for import into audio editors)
pub fn export_impulse_response_txt(
    data: &ImpulseResponseData,
    _metadata: &MeasurementMetadata,
) -> String {
    let mut output = String::new();

    // Simple format: one sample per line
    // This can be imported by many audio tools
    output.push_str(&format!("; Sample Rate: {}\n", data.sample_rate as u32));
    output.push_str(&format!("; Samples: {}\n", data.samples.len()));
    output.push('\n');

    for sample in &data.samples {
        output.push_str(&format!("{:.8}\n", sample));
    }

    output
}

/// Write export data to a file
pub fn write_to_file(path: &str, content: &str) -> Result<(), std::io::Error> {
    let mut file = std::fs::File::create(path)?;
    file.write_all(content.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_transfer_function() -> TransferFunctionData {
        TransferFunctionData {
            frequencies: vec![100.0, 1000.0, 10000.0],
            magnitude_db: vec![-10.0, 0.0, -5.0],
            phase_deg: vec![0.0, -45.0, -90.0],
            coherence: vec![0.95, 0.98, 0.90],
            group_delay_ms: vec![1.0, 1.5, 2.0],
        }
    }

    fn create_test_metadata() -> MeasurementMetadata {
        MeasurementMetadata {
            name: "Test Measurement".to_string(),
            date: "2024-01-01 12:00:00".to_string(),
            sample_rate: 48000.0,
            fft_size: 2048,
            averaging_count: 16,
            notes: Some("Test notes".to_string()),
        }
    }

    #[test]
    fn test_csv_export() {
        let data = create_test_transfer_function();
        let metadata = create_test_metadata();

        let csv = export_transfer_function_csv(&data, &metadata);

        assert!(csv.contains("Frequency (Hz)"));
        assert!(csv.contains("100.00"));
        assert!(csv.contains("1000.00"));
        assert!(csv.contains("10000.00"));
    }

    #[test]
    fn test_json_export() {
        let data = create_test_transfer_function();
        let metadata = create_test_metadata();

        let json = export_transfer_function_json(&data, &metadata).unwrap();

        assert!(json.contains("\"frequencies\""));
        assert!(json.contains("\"magnitude_db\""));
        assert!(json.contains("Test Measurement"));
    }

    #[test]
    fn test_txt_export() {
        let data = create_test_transfer_function();
        let metadata = create_test_metadata();

        let txt = export_transfer_function_txt(&data, &metadata);

        assert!(txt.contains("* Phonetic"));
        assert!(txt.contains("100.00\t-10.00"));
    }

    #[test]
    fn format_utc_matches_known_timestamps() {
        // Epoch/UTC pairs generated with `date -u -j -f %s`.
        let cases = [
            (0u64, "1970-01-01 00:00:00"),
            (951_782_400, "2000-02-29 00:00:00"), // leap day in a 400-divisible century
            (1_709_208_000, "2024-02-29 12:00:00"), // ordinary leap day
            (1_788_871_068, "2026-09-08 12:37:48"),
            (4_102_444_800, "2100-01-01 00:00:00"), // 2100 is NOT a leap year
        ];
        for (epoch, expected) in cases {
            assert_eq!(format_utc(epoch), expected, "epoch {epoch}");
        }
    }

    #[test]
    fn format_utc_rejects_the_old_thirty_day_month_approximation() {
        // The previous implementation returned 2026-09-25 for this instant.
        // This test exists so that regression cannot come back unnoticed.
        assert_eq!(&format_utc(1_788_871_068)[..10], "2026-09-08");
    }

    #[test]
    fn format_utc_rolls_over_midnight_correctly() {
        assert_eq!(format_utc(86_399), "1970-01-01 23:59:59");
        assert_eq!(format_utc(86_400), "1970-01-02 00:00:00");
    }
}
