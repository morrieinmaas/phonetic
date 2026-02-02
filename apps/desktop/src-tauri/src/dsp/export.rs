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

/// Get current date/time as string (simple implementation without chrono dependency)
fn chrono_lite_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let secs = duration.as_secs();
    // Simple date calculation (approximate, good enough for timestamps)
    let days = secs / 86400;
    let years_since_1970 = days / 365;
    let year = 1970 + years_since_1970;
    let remaining_days = days % 365;
    let month = (remaining_days / 30).min(11) + 1;
    let day = (remaining_days % 30) + 1;

    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        year, month, day, hours, minutes, seconds
    )
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
}
