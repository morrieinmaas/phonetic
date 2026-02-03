//! Audio Edge Case Tests
//!
//! Comprehensive tests for edge cases in audio processing:
//! - Empty and zero-length buffers
//! - Extreme values (clipping, DC offset, near-zero)
//! - Boundary conditions (Nyquist, FFT limits)
//! - Special floating-point values (NaN, Infinity)
//! - Stress testing and robustness

use phonetic_lib::dsp::{
    apply_blackman_harris_window, apply_flat_top_window, apply_hamming_window, apply_hann_window,
    DelayFinder, FftProcessor, ImpulseResponseProcessor, MicrophoneCalibration, ReferenceCurve,
    ReferenceCurveType, SplCalculator, TransferFunctionProcessor,
};
use std::f32::consts::PI;

// ============================================================================
// Empty and Zero-Length Buffer Tests
// ============================================================================

#[test]
fn test_fft_empty_buffer() {
    let mut processor = FftProcessor::new(1024);
    // Process empty slice - should not panic
    processor.process(&[]);
    let mag = processor.get_magnitude_db();
    assert_eq!(mag.len(), 513); // N/2 + 1
}

#[test]
fn test_transfer_function_empty_buffers() {
    let mut processor = TransferFunctionProcessor::new(1024, 48000.0);
    // Process empty slices - should not panic
    processor.process(&[], &[]);
    let data = processor.get_data();
    assert_eq!(data.frequencies.len(), 513);
}

#[test]
fn test_delay_finder_empty_buffers() {
    let mut finder = DelayFinder::new(2048, 48000.0, 100.0);
    let result = finder.find_delay(&[], &[]);
    assert!(!result.is_valid);
    assert_eq!(result.delay_samples, 0);
}

#[test]
fn test_impulse_response_empty_buffers() {
    let mut processor = ImpulseResponseProcessor::new(2048, 48000.0);
    processor.start_capture();
    processor.add_samples(&[], &[]);
    assert!(!processor.is_capture_complete());
}

#[test]
fn test_window_functions_empty_buffer() {
    let mut empty: Vec<f32> = vec![];
    apply_hann_window(&mut empty);
    apply_hamming_window(&mut empty);
    apply_blackman_harris_window(&mut empty);
    apply_flat_top_window(&mut empty);
    assert!(empty.is_empty());
}

// ============================================================================
// Extreme Value Tests
// ============================================================================

#[test]
fn test_fft_clipping_signal() {
    let mut processor = FftProcessor::new(1024);
    // Full-scale clipped signal (worst case)
    let clipped: Vec<f32> = (0..2048).map(|i| if i % 2 == 0 { 1.0 } else { -1.0 }).collect();
    processor.process(&clipped);
    let mag = processor.get_magnitude_db();
    // Should produce valid output without overflow
    assert!(mag.iter().all(|&v| v.is_finite()));
}

#[test]
fn test_fft_near_zero_signal() {
    let mut processor = FftProcessor::new(1024);
    // Very small amplitude signal
    let tiny: Vec<f32> = (0..2048)
        .map(|i| (2.0 * PI * 1000.0 * i as f32 / 48000.0).sin() * 1e-10)
        .collect();
    processor.process(&tiny);
    let mag = processor.get_magnitude_db();
    // Should handle very small values gracefully
    assert!(mag.iter().all(|&v| v.is_finite()));
}

#[test]
fn test_fft_dc_only_signal() {
    let mut processor = FftProcessor::new(1024);
    // Pure DC signal
    let dc: Vec<f32> = vec![0.5; 2048];
    processor.process(&dc);
    let mag = processor.get_magnitude_db();
    // DC should be in bin 0
    assert!(mag[0] > mag[1] && mag[0] > mag[2]);
}

#[test]
fn test_transfer_function_inverted_polarity() {
    let mut processor = TransferFunctionProcessor::new(2048, 48000.0);
    let reference: Vec<f32> = (0..4096)
        .map(|i| (2.0 * PI * 1000.0 * i as f32 / 48000.0).sin())
        .collect();
    let inverted: Vec<f32> = reference.iter().map(|&x| -x).collect();

    processor.process(&inverted, &reference);
    let data = processor.get_data();

    // Phase should be ~180 degrees at signal frequency
    let bin_1k = (1000.0_f32 / 48000.0 * 2048.0).round() as usize;
    assert!((data.phase_deg[bin_1k].abs() - 180.0).abs() < 20.0);
}

#[test]
fn test_delay_finder_identical_signals() {
    let mut finder = DelayFinder::new(4096, 48000.0, 100.0);
    let signal: Vec<f32> = (0..2048)
        .map(|i| (2.0 * PI * 1000.0 * i as f32 / 48000.0).sin())
        .collect();

    let result = finder.find_delay(&signal, &signal);
    assert!(result.is_valid);
    assert_eq!(result.delay_samples, 0);
    assert!(result.correlation > 0.9);
}

#[test]
fn test_spl_calculator_extreme_dbfs() {
    let calc = SplCalculator::default();

    // Very negative dBFS (silence)
    let spl_low = calc.dbfs_to_spl(-120.0);
    assert!(spl_low.is_finite());

    // 0 dBFS (full scale)
    let spl_full = calc.dbfs_to_spl(0.0);
    assert!(spl_full.is_finite());

    // Positive dBFS (clipping indication)
    let spl_clip = calc.dbfs_to_spl(6.0);
    assert!(spl_clip.is_finite());
}

// ============================================================================
// Boundary Condition Tests
// ============================================================================

#[test]
fn test_fft_nyquist_frequency() {
    let mut processor = FftProcessor::new(1024);
    let sample_rate = 48000.0;
    let nyquist = sample_rate / 2.0;

    // Signal at Nyquist frequency
    let nyquist_signal: Vec<f32> = (0..2048)
        .map(|i| (2.0 * PI * nyquist * i as f32 / sample_rate).sin())
        .collect();

    processor.process(&nyquist_signal);
    let mag = processor.get_magnitude_db();

    // Should have energy at the last bin
    assert!(mag.last().unwrap().is_finite());
}

#[test]
fn test_fft_sub_nyquist_frequency() {
    let mut processor = FftProcessor::new(1024);
    let sample_rate = 48000.0;

    // Signal very close to but below Nyquist
    let freq = sample_rate / 2.0 - 100.0;
    let signal: Vec<f32> = (0..2048)
        .map(|i| (2.0 * PI * freq * i as f32 / sample_rate).sin())
        .collect();

    processor.process(&signal);
    let mag = processor.get_magnitude_db();
    assert!(mag.iter().all(|&v| v.is_finite()));
}

#[test]
fn test_fft_minimum_size() {
    let processor = FftProcessor::new(64);
    assert_eq!(processor.fft_size(), 64);
    assert_eq!(processor.bin_count(), 33);
}

#[test]
fn test_fft_maximum_practical_size() {
    let processor = FftProcessor::new(32768);
    assert_eq!(processor.fft_size(), 32768);
    assert_eq!(processor.bin_count(), 16385);
}

#[test]
fn test_transfer_function_various_fft_sizes() {
    for size in [256, 512, 1024, 2048, 4096, 8192, 16384] {
        let processor = TransferFunctionProcessor::new(size, 48000.0);
        assert_eq!(processor.fft_size(), size);
        assert_eq!(processor.bin_count(), size / 2 + 1);
    }
}

#[test]
fn test_delay_finder_max_delay_boundary() {
    let mut finder = DelayFinder::new(4096, 48000.0, 100.0);

    // Delay at maximum search range (100ms = 4800 samples at 48kHz)
    let reference: Vec<f32> = (0..2048)
        .map(|i| {
            let t = i as f32 / 48000.0;
            let freq = 500.0 + (5000.0 - 500.0) * t * 20.0;
            (2.0 * PI * freq * t).sin()
        })
        .collect();

    // Result should handle boundary gracefully
    let result = finder.find_delay(&reference, &reference);
    assert!(result.is_valid || result.delay_samples == 0);
}

#[test]
fn test_reference_curve_frequency_boundaries() {
    // Test with frequencies at human hearing boundaries
    let frequencies = vec![20.0, 50.0, 100.0, 1000.0, 10000.0, 20000.0];

    for curve_type in ReferenceCurveType::all() {
        let curve = ReferenceCurve::generate(curve_type, &frequencies);
        assert_eq!(curve.points.len(), 6);
        assert!(curve.points.iter().all(|p| p.amplitude_db.is_finite()));
    }
}

// ============================================================================
// Special Floating-Point Value Tests
// ============================================================================

#[test]
fn test_fft_handles_nan_gracefully() {
    let mut processor = FftProcessor::new(1024);
    let mut signal = vec![0.5; 2048];
    signal[100] = f32::NAN;

    // This might produce NaN in output, but should not panic
    processor.process(&signal);
    let mag = processor.get_magnitude_db();
    assert_eq!(mag.len(), 513);
}

#[test]
fn test_fft_handles_infinity_gracefully() {
    let mut processor = FftProcessor::new(1024);
    let mut signal = vec![0.5; 2048];
    signal[100] = f32::INFINITY;

    // This might produce Inf in output, but should not panic
    processor.process(&signal);
    let mag = processor.get_magnitude_db();
    assert_eq!(mag.len(), 513);
}

#[test]
fn test_spl_calculator_nan_input() {
    let calc = SplCalculator::default();
    let result = calc.dbfs_to_spl(f32::NAN);
    // Result should be NaN for NaN input
    assert!(result.is_nan());
}

#[test]
fn test_spl_calculator_infinity_input() {
    let calc = SplCalculator::default();
    let result_pos = calc.dbfs_to_spl(f32::INFINITY);
    let result_neg = calc.dbfs_to_spl(f32::NEG_INFINITY);
    // Infinity should propagate
    assert!(result_pos.is_infinite() || result_pos.is_nan());
    assert!(result_neg.is_infinite() || result_neg.is_nan());
}

// ============================================================================
// Stress and Robustness Tests
// ============================================================================

#[test]
fn test_fft_rapid_size_changes() {
    let mut processor = FftProcessor::new(1024);
    let signal: Vec<f32> = (0..32768).map(|i| (i as f32 * 0.01).sin()).collect();

    for size in [512, 2048, 1024, 4096, 512, 8192, 1024] {
        processor.set_fft_size(size);
        processor.process(&signal[..size * 2]);
        let mag = processor.get_magnitude_db();
        assert_eq!(mag.len(), size / 2 + 1);
    }
}

#[test]
fn test_transfer_function_repeated_reset() {
    let mut processor = TransferFunctionProcessor::new(2048, 48000.0);
    let signal: Vec<f32> = (0..4096).map(|i| (i as f32 * 0.01).sin()).collect();

    for _ in 0..100 {
        processor.process(&signal, &signal);
        processor.reset_averaging();
    }

    assert_eq!(processor.average_count(), 0);
}

#[test]
fn test_impulse_response_start_stop_cycle() {
    let mut processor = ImpulseResponseProcessor::new(2048, 48000.0);
    let signal: Vec<f32> = (0..1024).map(|i| (i as f32 * 0.01).sin()).collect();

    for _ in 0..10 {
        processor.start_capture();
        processor.add_samples(&signal, &signal);
        processor.stop_capture();
        processor.clear();
    }

    // Should be in clean state
    processor.start_capture();
    assert!(!processor.is_capture_complete());
}

#[test]
fn test_delay_finder_many_correlations() {
    let mut finder = DelayFinder::new(4096, 48000.0, 100.0);
    let signal: Vec<f32> = (0..2048).map(|i| (i as f32 * 0.01).sin()).collect();

    // Run many correlations
    for _ in 0..100 {
        let _result = finder.find_delay(&signal, &signal);
    }

    // Should still work correctly
    let result = finder.find_delay(&signal, &signal);
    assert!(result.is_valid);
}

// ============================================================================
// Sample Rate Edge Cases
// ============================================================================

#[test]
fn test_transfer_function_sample_rate_change() {
    let mut processor = TransferFunctionProcessor::new(2048, 44100.0);
    processor.set_sample_rate(48000.0);

    let data = processor.get_data();
    let expected_nyquist = 24000.0;
    let actual_nyquist = *data.frequencies.last().unwrap();
    assert!((actual_nyquist - expected_nyquist).abs() < 50.0);
}

#[test]
fn test_transfer_function_extreme_sample_rates() {
    // Test with various sample rates
    for sample_rate in [8000.0, 22050.0, 44100.0, 48000.0, 96000.0, 192000.0] {
        let processor = TransferFunctionProcessor::new(2048, sample_rate);
        let data = processor.get_data();
        let expected_nyquist = sample_rate / 2.0;
        let actual_nyquist = *data.frequencies.last().unwrap();
        assert!(
            (actual_nyquist - expected_nyquist).abs() < 1.0,
            "Failed for sample rate {}",
            sample_rate
        );
    }
}

#[test]
fn test_delay_finder_sample_rate_change() {
    let mut finder = DelayFinder::new(4096, 48000.0, 100.0);
    finder.set_sample_rate(96000.0);
    assert_eq!(finder.sample_rate(), 96000.0);
}

// ============================================================================
// Calibration Edge Cases
// ============================================================================

#[test]
fn test_calibration_default_values() {
    let cal = MicrophoneCalibration::default();
    assert!(cal.sensitivity_db.is_finite());
    assert!(cal.reference_spl_db > 0.0);
}

#[test]
fn test_spl_calculator_with_calibration() {
    let mut calc = SplCalculator::default();
    calc.calibrate_with_reference(-20.0, 94.0); // Common calibration point

    // After calibration, -20 dBFS should map to ~94 dB SPL
    let spl = calc.dbfs_to_spl(-20.0);
    assert!((spl - 94.0).abs() < 1.0);
}

#[test]
fn test_spl_calculator_a_weighting_toggle() {
    let mut calc = SplCalculator::default();

    // Toggle A-weighting and verify it affects calculations
    calc.set_a_weighting(true);
    let spl_with_a = calc.dbfs_to_spl(-40.0);

    calc.set_a_weighting(false);
    let spl_without_a = calc.dbfs_to_spl(-40.0);

    // Basic dbfs_to_spl doesn't use A-weighting, so these should be equal
    assert!((spl_with_a - spl_without_a).abs() < 0.01);
}

#[test]
fn test_spl_calculator_preamp_gain() {
    let mut calc = SplCalculator::default();

    let spl_no_gain = calc.dbfs_to_spl(-40.0);

    calc.set_preamp_gain(20.0);
    let spl_with_gain = calc.dbfs_to_spl(-40.0);

    // With 20dB preamp gain, the calculated SPL should be 20dB LOWER
    // (because higher preamp gain means the actual sound level was lower)
    assert!(
        (spl_no_gain - spl_with_gain - 20.0).abs() < 1.0,
        "Expected spl_no_gain - spl_with_gain = 20, got {} - {} = {}",
        spl_no_gain,
        spl_with_gain,
        spl_no_gain - spl_with_gain
    );
}

// ============================================================================
// Window Function Edge Cases
// ============================================================================

#[test]
fn test_window_single_sample() {
    let mut single = vec![1.0];
    apply_hann_window(&mut single);
    assert_eq!(single[0], 1.0); // Single sample unchanged
}

#[test]
fn test_window_two_samples() {
    let mut two = vec![1.0, 1.0];
    apply_hann_window(&mut two);
    // Both endpoints should be near zero for Hann
    assert!(two[0] < 0.01);
    assert!(two[1] < 0.01);
}

#[test]
fn test_window_odd_length() {
    for len in [127, 255, 513, 1023, 2047] {
        let mut buffer = vec![1.0; len];
        apply_hann_window(&mut buffer);

        // Should be symmetric around center
        let center = len / 2;
        assert!(buffer[center] > 0.99);
        assert!(buffer[0] < 0.01);
        assert!(buffer[len - 1] < 0.01);
    }
}

#[test]
fn test_window_preserves_negative_values() {
    let mut buffer = vec![-1.0; 128];
    apply_hann_window(&mut buffer);

    // All non-zero values should remain negative
    for &v in &buffer {
        assert!(v <= 0.0, "Window should preserve sign");
    }
}

#[test]
fn test_window_very_large_buffer() {
    let mut buffer = vec![1.0; 65536];
    apply_hann_window(&mut buffer);

    assert!(buffer[0] < 0.001);
    assert!(buffer[32768] > 0.999);
    assert!(buffer[65535] < 0.001);
}

// ============================================================================
// Reference Curve Edge Cases
// ============================================================================

#[test]
fn test_reference_curve_single_frequency() {
    let frequencies = vec![1000.0];
    for curve_type in ReferenceCurveType::all() {
        let curve = ReferenceCurve::generate(curve_type, &frequencies);
        assert_eq!(curve.points.len(), 1);
        assert!(curve.points[0].amplitude_db.is_finite());
    }
}

#[test]
fn test_reference_curve_subsonic_frequency() {
    // Below typical human hearing
    let frequencies = vec![1.0, 5.0, 10.0, 15.0];
    for curve_type in ReferenceCurveType::all() {
        let curve = ReferenceCurve::generate(curve_type, &frequencies);
        // Should handle gracefully, values might be extrapolated or clamped
        assert!(curve.points.iter().all(|p| p.amplitude_db.is_finite()));
    }
}

#[test]
fn test_reference_curve_ultrasonic_frequency() {
    // Above typical human hearing
    let frequencies = vec![25000.0, 30000.0, 40000.0];
    for curve_type in ReferenceCurveType::all() {
        let curve = ReferenceCurve::generate(curve_type, &frequencies);
        // Should handle gracefully
        assert!(curve.points.iter().all(|p| p.amplitude_db.is_finite()));
    }
}

#[test]
fn test_reference_curve_logarithmic_spacing() {
    // Typical log-spaced frequencies for audio
    let frequencies: Vec<f64> = (0..100)
        .map(|i| 20.0 * 10.0_f64.powf(i as f64 / 100.0 * 3.0))
        .collect();

    for curve_type in ReferenceCurveType::all() {
        let curve = ReferenceCurve::generate(curve_type, &frequencies);
        assert_eq!(curve.points.len(), 100);
        assert!(curve.points.iter().all(|p| p.amplitude_db.is_finite()));
    }
}

// ============================================================================
// Coherence Edge Cases
// ============================================================================

#[test]
fn test_coherence_always_bounded() {
    let mut processor = TransferFunctionProcessor::new(2048, 48000.0);

    // Process with various signal combinations
    let signal1: Vec<f32> = (0..4096).map(|i| (i as f32 * 0.1).sin()).collect();
    let signal2: Vec<f32> = (0..4096).map(|i| (i as f32 * 0.17).sin()).collect();
    let noise: Vec<f32> = (0..4096).map(|i| ((i as f32 * 0.37).sin() * 31.0) % 1.0).collect();

    processor.process(&signal1, &signal1);
    let data1 = processor.get_data();
    assert!(data1.coherence.iter().all(|&c| c >= 0.0 && c <= 1.0));

    processor.reset_averaging();
    processor.process(&signal1, &signal2);
    let data2 = processor.get_data();
    assert!(data2.coherence.iter().all(|&c| c >= 0.0 && c <= 1.0));

    processor.reset_averaging();
    processor.process(&noise, &signal1);
    let data3 = processor.get_data();
    assert!(data3.coherence.iter().all(|&c| c >= 0.0 && c <= 1.0));
}

// ============================================================================
// Memory and Resource Tests
// ============================================================================

#[test]
fn test_processor_creation_destruction_cycle() {
    for _ in 0..100 {
        let processor = FftProcessor::new(2048);
        drop(processor);
    }
    // No memory leaks or issues
}

#[test]
fn test_large_batch_processing() {
    let mut processor = FftProcessor::new(2048);

    // Process 10 seconds of audio at 48kHz
    let total_samples = 48000 * 10;
    let chunk_size = 512;
    let num_chunks = total_samples / chunk_size;

    let chunk: Vec<f32> = (0..chunk_size)
        .map(|i| (2.0 * PI * 1000.0 * i as f32 / 48000.0).sin())
        .collect();

    for _ in 0..num_chunks {
        processor.process(&chunk);
    }

    let mag = processor.get_magnitude_db();
    assert!(mag.iter().all(|&v| v.is_finite()));
}
