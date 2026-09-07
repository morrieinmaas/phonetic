//! Impulse Response Capture and Analysis
//!
//! Captures and analyzes impulse responses using deconvolution of
//! swept-sine or MLS (Maximum Length Sequence) excitation signals.
//!
//! The impulse response h(t) is the inverse FFT of the transfer function H(f),
//! providing time-domain information about the system under test.

// Allow unused methods - these are library APIs for future frontend integration
#![allow(dead_code)]

use realfft::{RealFftPlanner, RealToComplex};
use std::sync::Arc;

/// Impulse response data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImpulseResponseData {
    /// Time-domain impulse response samples
    pub samples: Vec<f32>,
    /// Time axis in milliseconds
    pub time_ms: Vec<f32>,
    /// Sample rate
    pub sample_rate: f32,
    /// Peak value
    pub peak_value: f32,
    /// Time of peak in milliseconds
    pub peak_time_ms: f32,
    /// Estimated RT60 (reverberation time) in milliseconds
    pub rt60_ms: Option<f32>,
    /// Energy decay curve (Schroeder integration) in dB
    pub edc_db: Vec<f32>,
}

/// Impulse Response processor
pub struct ImpulseResponseProcessor {
    fft_size: usize,
    sample_rate: f32,
    fft_planner: RealFftPlanner<f32>,
    forward_fft: Arc<dyn RealToComplex<f32>>,
    inverse_fft: Arc<dyn realfft::ComplexToReal<f32>>,
    /// Accumulated reference signal
    reference_buffer: Vec<f32>,
    /// Accumulated measured signal
    measured_buffer: Vec<f32>,
    /// Whether capture is in progress
    is_capturing: bool,
    /// Scratch buffers
    scratch_ref: Vec<f32>,
    scratch_meas: Vec<f32>,
    spectrum_ref: Vec<realfft::num_complex::Complex<f32>>,
    spectrum_meas: Vec<realfft::num_complex::Complex<f32>>,
    ir_result: Vec<f32>,
}

impl ImpulseResponseProcessor {
    /// Create a new impulse response processor
    ///
    /// # Arguments
    /// * `fft_size` - FFT size for deconvolution (determines IR length)
    /// * `sample_rate` - Sample rate in Hz
    pub fn new(fft_size: usize, sample_rate: f32) -> Self {
        let mut fft_planner = RealFftPlanner::new();
        let forward_fft = fft_planner.plan_fft_forward(fft_size);
        let inverse_fft = fft_planner.plan_fft_inverse(fft_size);

        let spectrum_size = fft_size / 2 + 1;

        Self {
            fft_size,
            sample_rate,
            fft_planner,
            forward_fft,
            inverse_fft,
            reference_buffer: Vec::with_capacity(fft_size),
            measured_buffer: Vec::with_capacity(fft_size),
            is_capturing: false,
            scratch_ref: vec![0.0; fft_size],
            scratch_meas: vec![0.0; fft_size],
            spectrum_ref: vec![realfft::num_complex::Complex::new(0.0, 0.0); spectrum_size],
            spectrum_meas: vec![realfft::num_complex::Complex::new(0.0, 0.0); spectrum_size],
            ir_result: vec![0.0; fft_size],
        }
    }

    /// Start capturing impulse response
    pub fn start_capture(&mut self) {
        self.reference_buffer.clear();
        self.measured_buffer.clear();
        self.is_capturing = true;
    }

    /// Stop capturing and check if we have enough data
    pub fn stop_capture(&mut self) -> bool {
        self.is_capturing = false;
        self.reference_buffer.len() >= self.fft_size && self.measured_buffer.len() >= self.fft_size
    }

    /// Add samples during capture
    pub fn add_samples(&mut self, reference: &[f32], measured: &[f32]) {
        if !self.is_capturing {
            return;
        }

        // Only accumulate up to fft_size samples
        let remaining_ref = self.fft_size.saturating_sub(self.reference_buffer.len());
        let remaining_meas = self.fft_size.saturating_sub(self.measured_buffer.len());

        if remaining_ref > 0 {
            let take = reference.len().min(remaining_ref);
            self.reference_buffer.extend_from_slice(&reference[..take]);
        }

        if remaining_meas > 0 {
            let take = measured.len().min(remaining_meas);
            self.measured_buffer.extend_from_slice(&measured[..take]);
        }
    }

    /// Check if capture is complete
    pub fn is_capture_complete(&self) -> bool {
        self.reference_buffer.len() >= self.fft_size && self.measured_buffer.len() >= self.fft_size
    }

    /// Check if currently capturing
    pub fn is_capturing(&self) -> bool {
        self.is_capturing
    }

    /// Get capture progress (0.0 to 1.0)
    pub fn capture_progress(&self) -> f32 {
        let ref_progress = self.reference_buffer.len() as f32 / self.fft_size as f32;
        let meas_progress = self.measured_buffer.len() as f32 / self.fft_size as f32;
        ref_progress.min(meas_progress).min(1.0)
    }

    /// Compute impulse response from captured data
    ///
    /// Uses deconvolution: H(f) = Y(f) / X(f), then h(t) = IFFT(H(f))
    ///
    /// Includes regularization to avoid division by small values.
    pub fn compute_impulse_response(&mut self) -> Option<ImpulseResponseData> {
        if self.reference_buffer.len() < self.fft_size || self.measured_buffer.len() < self.fft_size
        {
            return None;
        }

        // Copy to scratch buffers
        self.scratch_ref[..self.fft_size].copy_from_slice(&self.reference_buffer[..self.fft_size]);
        self.scratch_meas[..self.fft_size].copy_from_slice(&self.measured_buffer[..self.fft_size]);

        // Forward FFT of both signals
        self.forward_fft
            .process(&mut self.scratch_ref, &mut self.spectrum_ref)
            .ok()?;
        self.forward_fft
            .process(&mut self.scratch_meas, &mut self.spectrum_meas)
            .ok()?;

        // Deconvolution with regularization: H = Y / X
        // Using Wiener deconvolution: H = (X* * Y) / (|X|^2 + epsilon)
        // Regularisation must scale with the reference, or a quiet reference is
        // swamped by the constant and a loud one is unregularised. 1e-4 of mean
        // reference energy is ~-40 dB, below any real measurement's noise floor.
        let mean_ref_energy: f32 = self.spectrum_ref.iter().map(|c| c.norm_sqr()).sum::<f32>()
            / self.spectrum_ref.len() as f32;
        let epsilon = (mean_ref_energy * 1e-4).max(1e-20);

        for i in 0..self.spectrum_ref.len() {
            let x = self.spectrum_ref[i];
            let y = self.spectrum_meas[i];

            let x_mag_sq = x.norm_sqr();
            let x_conj = x.conj();

            // Wiener deconvolution
            self.spectrum_ref[i] = (x_conj * y) / (x_mag_sq + epsilon);
        }

        // Inverse FFT to get impulse response
        self.inverse_fft
            .process(&mut self.spectrum_ref, &mut self.ir_result)
            .ok()?;

        // Normalize
        let scale = 1.0 / self.fft_size as f32;
        for sample in &mut self.ir_result {
            *sample *= scale;
        }

        // Find peak
        let (peak_idx, peak_value) = self
            .ir_result
            .iter()
            .enumerate()
            .map(|(i, &v)| (i, v.abs()))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap_or((0, 0.0));

        let peak_time_ms = peak_idx as f32 * 1000.0 / self.sample_rate;

        // Generate time axis
        let time_ms: Vec<f32> = (0..self.fft_size)
            .map(|i| i as f32 * 1000.0 / self.sample_rate)
            .collect();

        // Compute Energy Decay Curve (EDC) using Schroeder integration
        let edc_db = self.compute_edc();

        // Estimate RT60
        let rt60_ms = self.estimate_rt60(&edc_db);

        Some(ImpulseResponseData {
            samples: self.ir_result.clone(),
            time_ms,
            sample_rate: self.sample_rate,
            peak_value,
            peak_time_ms,
            rt60_ms,
            edc_db,
        })
    }

    /// Compute Energy Decay Curve (Schroeder integration)
    ///
    /// EDC(t) = integral from t to infinity of h^2(τ) dτ
    fn compute_edc(&self) -> Vec<f32> {
        // Compute squared IR
        let ir_squared: Vec<f32> = self.ir_result.iter().map(|&x| x * x).collect();

        // Reverse cumulative sum (Schroeder integration)
        let mut edc = vec![0.0f32; self.fft_size];
        let mut sum = 0.0f32;

        for i in (0..self.fft_size).rev() {
            sum += ir_squared[i];
            edc[i] = sum;
        }

        // Convert to dB
        let max_energy = edc[0].max(1e-10);
        edc.iter()
            .map(|&e| 10.0 * (e / max_energy).max(1e-10).log10())
            .collect()
    }

    /// Estimate RT60 from Energy Decay Curve
    ///
    /// Uses linear regression on the -5dB to -35dB region (T30 method)
    /// and extrapolates to -60dB
    fn estimate_rt60(&self, edc_db: &[f32]) -> Option<f32> {
        // Find indices where EDC crosses -5dB and -35dB
        let mut idx_5db = None;
        let mut idx_35db = None;

        for (i, &val) in edc_db.iter().enumerate() {
            if idx_5db.is_none() && val <= -5.0 {
                idx_5db = Some(i);
            }
            if idx_5db.is_some() && idx_35db.is_none() && val <= -35.0 {
                idx_35db = Some(i);
                break;
            }
        }

        let start_idx = idx_5db?;
        let end_idx = idx_35db?;

        if end_idx <= start_idx {
            return None;
        }

        // Linear regression to find decay rate
        let n = (end_idx - start_idx) as f32;
        let mut sum_x = 0.0f32;
        let mut sum_y = 0.0f32;
        let mut sum_xy = 0.0f32;
        let mut sum_xx = 0.0f32;

        for (i, &y) in edc_db[start_idx..end_idx].iter().enumerate() {
            let x = i as f32;
            sum_x += x;
            sum_y += y;
            sum_xy += x * y;
            sum_xx += x * x;
        }

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x * sum_x);

        if slope >= 0.0 {
            return None; // No decay
        }

        // Time for 30dB decay
        let samples_per_30db = -30.0 / slope;
        let t30_ms = samples_per_30db * 1000.0 / self.sample_rate;

        // Extrapolate to RT60
        let rt60_ms = t30_ms * 2.0;

        // Sanity check
        if rt60_ms > 0.0 && rt60_ms < 30000.0 {
            Some(rt60_ms)
        } else {
            None
        }
    }

    /// Clear captured data
    pub fn clear(&mut self) {
        self.reference_buffer.clear();
        self.measured_buffer.clear();
        self.is_capturing = false;
    }

    /// Update sample rate
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    /// Update FFT size (also clears captured data)
    pub fn set_fft_size(&mut self, fft_size: usize) {
        if fft_size != self.fft_size {
            self.fft_size = fft_size;
            self.forward_fft = self.fft_planner.plan_fft_forward(fft_size);
            self.inverse_fft = self.fft_planner.plan_fft_inverse(fft_size);

            let spectrum_size = fft_size / 2 + 1;
            self.scratch_ref = vec![0.0; fft_size];
            self.scratch_meas = vec![0.0; fft_size];
            self.spectrum_ref = vec![realfft::num_complex::Complex::new(0.0, 0.0); spectrum_size];
            self.spectrum_meas = vec![realfft::num_complex::Complex::new(0.0, 0.0); spectrum_size];
            self.ir_result = vec![0.0; fft_size];

            self.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_impulse_response_identity() {
        let mut processor = ImpulseResponseProcessor::new(4096, 48000.0);

        // Create an impulse (delta function)
        let mut reference = vec![0.0f32; 4096];
        reference[0] = 1.0;

        // Same signal should give identity IR
        processor.start_capture();
        processor.add_samples(&reference, &reference);

        assert!(processor.is_capture_complete());

        let ir = processor.compute_impulse_response().unwrap();

        // Peak should be at or near time 0
        assert!(ir.peak_time_ms < 1.0, "Peak should be near time 0");
        assert!(ir.peak_value > 0.5, "Peak should be significant");
    }

    #[test]
    fn test_delayed_impulse() {
        let mut processor = ImpulseResponseProcessor::new(4096, 48000.0);

        // Reference: impulse at t=0
        let mut reference = vec![0.0f32; 4096];
        reference[0] = 1.0;

        // Measured: impulse delayed by 480 samples (10ms at 48kHz)
        let mut measured = vec![0.0f32; 4096];
        measured[480] = 1.0;

        processor.start_capture();
        processor.add_samples(&reference, &measured);

        let ir = processor.compute_impulse_response().unwrap();

        // Peak should be around 10ms
        assert!(
            (ir.peak_time_ms - 10.0).abs() < 1.0,
            "Expected peak at ~10ms, got {}ms",
            ir.peak_time_ms
        );
    }

    #[test]
    fn test_capture_progress() {
        let mut processor = ImpulseResponseProcessor::new(4096, 48000.0);

        processor.start_capture();
        assert_eq!(processor.capture_progress(), 0.0);

        let samples = vec![0.0f32; 2048];
        processor.add_samples(&samples, &samples);

        assert!((processor.capture_progress() - 0.5).abs() < 0.01);

        processor.add_samples(&samples, &samples);
        assert!((processor.capture_progress() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_edc_decreasing() {
        let mut processor = ImpulseResponseProcessor::new(4096, 48000.0);

        // Create exponentially decaying signal
        let reference: Vec<f32> = (0..4096).map(|i| (-i as f32 / 1000.0).exp()).collect();
        let measured = reference.clone();

        processor.start_capture();
        processor.add_samples(&reference, &measured);

        let ir = processor.compute_impulse_response().unwrap();

        // EDC should be monotonically decreasing
        for i in 1..ir.edc_db.len() {
            assert!(
                ir.edc_db[i] <= ir.edc_db[i - 1] + 0.1, // Allow small numerical errors
                "EDC should be monotonically decreasing"
            );
        }
    }
}
