//! Headless measurement. Nothing else in this category is scriptable, which makes
//! this both a differentiator and the harness the backend e2e tests run through.

use crate::audio::{list_input_devices, measure_from_file};
use crate::dsp::TransferFunctionData;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Csv,
}

/// Measure a WAV file pair and render the result.
pub fn run_measure(
    path: &str,
    reference_channel: usize,
    measurement_channel: usize,
    format: OutputFormat,
) -> Result<String, String> {
    let data = measure_from_file(path, reference_channel, measurement_channel)?;
    Ok(match format {
        OutputFormat::Json => {
            serde_json::to_string_pretty(&data).map_err(|e| format!("serialise: {e}"))?
        }
        OutputFormat::Csv => to_csv(&data),
    })
}

fn to_csv(data: &TransferFunctionData) -> String {
    let mut out = String::from("frequency_hz,magnitude_db,phase_deg,coherence,group_delay_ms\n");
    for i in 0..data.frequencies.len() {
        out.push_str(&format!(
            "{:.4},{:.4},{:.4},{:.4},{:.4}\n",
            data.frequencies[i],
            data.magnitude_db[i],
            data.phase_deg[i],
            data.coherence[i],
            data.group_delay_ms[i],
        ));
    }
    out
}

/// List available input devices, one per line.
pub fn run_devices() -> Result<String, String> {
    let devices = list_input_devices();
    if devices.is_empty() {
        return Err("no input devices found".into());
    }
    Ok(devices.join("\n"))
}

/// Verify the DSP against a known signal. Exercises the same path a real
/// measurement takes, so a broken build fails here rather than on a show site.
pub fn run_selftest() -> Result<String, String> {
    use crate::dsp::TransferFunctionProcessor;

    let reference: Vec<f32> = (0..4096 * 32)
        .map(|i| {
            let mut s = (i as u32).wrapping_mul(2_654_435_761);
            s ^= s >> 16;
            (s as f32 / u32::MAX as f32) * 2.0 - 1.0
        })
        .collect();
    let measurement: Vec<f32> = reference.iter().map(|s| s * 0.5).collect();

    let mut processor = TransferFunctionProcessor::new(4096, 48_000.0);
    processor.process(&measurement, &reference);
    let data = processor.get_data();

    let bin = |f: f32| (f / (48_000.0 / 4096.0)).round() as usize;
    let slice = &data.magnitude_db[bin(500.0)..bin(10_000.0)];
    let mean = slice.iter().sum::<f32>() / slice.len() as f32;
    let error = (mean - -6.0206).abs();

    if error < 0.05 {
        Ok(format!(
            "PASS  known -6.02 dB path measured {mean:.4} dB (error {error:.4} dB)"
        ))
    } else {
        Err(format!(
            "FAIL  known -6.02 dB path measured {mean:.4} dB (error {error:.4} dB)"
        ))
    }
}
