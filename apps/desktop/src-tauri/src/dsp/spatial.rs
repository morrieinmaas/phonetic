//! Spatial averaging across microphone positions.
//!
//! Averaging is done on complex transfer functions. Averaging dB instead would
//! fill in every cancellation and show a system that measures better than it
//! sounds — the classic way to tune a room into a hole.

use crate::dsp::TransferFunctionData;
use realfft::num_complex::Complex;

/// Average several positions into one trace.
pub fn average_traces(traces: &[TransferFunctionData]) -> Result<TransferFunctionData, String> {
    let Some(first) = traces.first() else {
        return Err("nothing to average".into());
    };

    let n = first.frequencies.len();
    if traces.iter().any(|t| t.frequencies.len() != n) {
        return Err("all traces must share one frequency axis before averaging".into());
    }

    let count = traces.len() as f32;
    let mut magnitude_db = Vec::with_capacity(n);
    let mut phase_deg = Vec::with_capacity(n);
    let mut coherence = Vec::with_capacity(n);
    let mut group_delay_ms = Vec::with_capacity(n);

    for i in 0..n {
        let sum: Complex<f32> = traces
            .iter()
            .map(|t| {
                let magnitude = 10f32.powf(t.magnitude_db[i] / 20.0);
                Complex::from_polar(magnitude, t.phase_deg[i].to_radians())
            })
            .sum();

        let mean = sum / count;
        let magnitude = mean.norm();
        magnitude_db.push(if magnitude > 1e-10 {
            20.0 * magnitude.log10()
        } else {
            -120.0
        });
        phase_deg.push(mean.arg().to_degrees());

        // Worst case wins: an average is only as trustworthy as its weakest
        // contributing measurement.
        coherence.push(
            traces
                .iter()
                .map(|t| t.coherence[i])
                .fold(f32::INFINITY, f32::min),
        );
        group_delay_ms.push(traces.iter().map(|t| t.group_delay_ms[i]).sum::<f32>() / count);
    }

    Ok(TransferFunctionData {
        magnitude_db,
        phase_deg,
        coherence,
        group_delay_ms,
        frequencies: first.frequencies.clone(),
    })
}
