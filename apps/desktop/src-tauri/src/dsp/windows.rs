use std::f32::consts::PI;

/// Apply Hann window to buffer in-place
pub fn apply_hann_window(buffer: &mut [f32]) {
    let n = buffer.len();
    // Handle edge case: single sample or empty buffer
    if n <= 1 {
        return;
    }
    let n_minus_1 = (n - 1) as f32;
    for (i, sample) in buffer.iter_mut().enumerate() {
        let multiplier = 0.5 * (1.0 - (2.0 * PI * i as f32 / n_minus_1).cos());
        *sample *= multiplier;
    }
}

/// Apply Blackman-Harris window to buffer in-place
#[allow(dead_code)]
pub fn apply_blackman_harris_window(buffer: &mut [f32]) {
    let n = buffer.len();
    // Handle edge case: single sample or empty buffer
    if n <= 1 {
        return;
    }
    let n_minus_1 = (n - 1) as f32;
    let a0 = 0.35875;
    let a1 = 0.48829;
    let a2 = 0.14128;
    let a3 = 0.01168;

    for (i, sample) in buffer.iter_mut().enumerate() {
        let x = 2.0 * PI * i as f32 / n_minus_1;
        let multiplier = a0 - a1 * x.cos() + a2 * (2.0 * x).cos() - a3 * (3.0 * x).cos();
        *sample *= multiplier;
    }
}

/// Apply Hamming window to buffer in-place
#[allow(dead_code)]
pub fn apply_hamming_window(buffer: &mut [f32]) {
    let n = buffer.len();
    // Handle edge case: single sample or empty buffer
    if n <= 1 {
        return;
    }
    let n_minus_1 = (n - 1) as f32;
    let alpha = 0.54;
    let beta = 1.0 - alpha;

    for (i, sample) in buffer.iter_mut().enumerate() {
        let multiplier = alpha - beta * (2.0 * PI * i as f32 / n_minus_1).cos();
        *sample *= multiplier;
    }
}

/// Apply flat-top window to buffer in-place (best for amplitude measurements)
#[allow(dead_code)]
pub fn apply_flat_top_window(buffer: &mut [f32]) {
    let n = buffer.len();
    // Handle edge case: single sample or empty buffer
    if n <= 1 {
        return;
    }
    let n_minus_1 = (n - 1) as f32;
    let a0 = 0.21557895;
    let a1 = 0.41663158;
    let a2 = 0.277_263_16;
    let a3 = 0.083578947;
    let a4 = 0.006947368;

    for (i, sample) in buffer.iter_mut().enumerate() {
        let x = 2.0 * PI * i as f32 / n_minus_1;
        let multiplier =
            a0 - a1 * x.cos() + a2 * (2.0 * x).cos() - a3 * (3.0 * x).cos() + a4 * (4.0 * x).cos();
        *sample *= multiplier;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hann_window_endpoints() {
        let mut buffer = vec![1.0; 128];
        apply_hann_window(&mut buffer);

        // Hann window should be near zero at endpoints
        assert!(buffer[0].abs() < 0.01, "Start should be near zero");
        assert!(
            buffer[buffer.len() - 1].abs() < 0.01,
            "End should be near zero"
        );
    }

    #[test]
    fn test_hann_window_center() {
        let mut buffer = vec![1.0; 128];
        apply_hann_window(&mut buffer);

        // Hann window should be near 1.0 at center
        let center = buffer.len() / 2;
        assert!(
            (buffer[center] - 1.0).abs() < 0.1,
            "Center should be near 1.0, got {}",
            buffer[center]
        );
    }

    #[test]
    fn test_hann_window_symmetry() {
        let mut buffer = vec![1.0; 128];
        apply_hann_window(&mut buffer);

        // Hann window should be symmetric
        for i in 0..buffer.len() / 2 {
            let diff = (buffer[i] - buffer[buffer.len() - 1 - i]).abs();
            assert!(diff < 0.0001, "Window should be symmetric at index {}", i);
        }
    }

    #[test]
    fn test_blackman_harris_window_endpoints() {
        let mut buffer = vec![1.0; 128];
        apply_blackman_harris_window(&mut buffer);

        // Blackman-Harris has very low sidelobes, near zero at endpoints
        assert!(buffer[0].abs() < 0.01, "Start should be near zero");
    }

    #[test]
    fn test_hamming_window_endpoints() {
        let mut buffer = vec![1.0; 128];
        apply_hamming_window(&mut buffer);

        // Hamming window has minimum at endpoints but not zero
        assert!(buffer[0] > 0.07 && buffer[0] < 0.09, "Hamming start ~0.08");
    }

    #[test]
    fn test_flat_top_window_center() {
        let mut buffer = vec![1.0; 128];
        apply_flat_top_window(&mut buffer);

        // Flat-top has flat center (best for amplitude accuracy)
        let center = buffer.len() / 2;
        assert!(
            buffer[center] > 0.9,
            "Flat-top center should be near 1.0, got {}",
            buffer[center]
        );
    }

    #[test]
    fn test_windows_reduce_discontinuity() {
        // A key property: all windows should reduce spectral leakage
        // by smoothly tapering at the edges
        let mut hann = vec![1.0; 64];
        let mut bh = vec![1.0; 64];
        let mut hamming = vec![1.0; 64];

        apply_hann_window(&mut hann);
        apply_blackman_harris_window(&mut bh);
        apply_hamming_window(&mut hamming);

        // All should have values <= 1.0
        for w in [&hann, &bh, &hamming] {
            for &v in w.iter() {
                assert!(v <= 1.01, "Window value should not exceed 1.0");
            }
        }
    }

    #[test]
    fn test_edge_case_single_sample() {
        // Single sample buffers should not panic (division by zero guard)
        let mut single = vec![1.0];
        apply_hann_window(&mut single);
        assert_eq!(single[0], 1.0, "Single sample should be unchanged");

        let mut single = vec![1.0];
        apply_blackman_harris_window(&mut single);
        assert_eq!(single[0], 1.0, "Single sample should be unchanged");

        let mut single = vec![1.0];
        apply_hamming_window(&mut single);
        assert_eq!(single[0], 1.0, "Single sample should be unchanged");

        let mut single = vec![1.0];
        apply_flat_top_window(&mut single);
        assert_eq!(single[0], 1.0, "Single sample should be unchanged");
    }

    #[test]
    fn test_edge_case_empty_buffer() {
        // Empty buffers should not panic
        let mut empty: Vec<f32> = vec![];
        apply_hann_window(&mut empty);
        apply_blackman_harris_window(&mut empty);
        apply_hamming_window(&mut empty);
        apply_flat_top_window(&mut empty);
        assert!(empty.is_empty());
    }

    #[test]
    fn test_hann_window_power_of_two_sizes() {
        for size in [32, 64, 128, 256, 512, 1024] {
            let mut buffer = vec![1.0; size];
            apply_hann_window(&mut buffer);
            assert!(buffer[0] < 0.01, "Size {} start should be near zero", size);
            assert!(buffer[size / 2] > 0.99, "Size {} center should be near 1.0", size);
        }
    }

    #[test]
    fn test_blackman_harris_symmetry() {
        let mut buffer = vec![1.0; 256];
        apply_blackman_harris_window(&mut buffer);

        for i in 0..buffer.len() / 2 {
            let diff = (buffer[i] - buffer[buffer.len() - 1 - i]).abs();
            assert!(diff < 0.0001, "Blackman-Harris should be symmetric");
        }
    }

    #[test]
    fn test_hamming_symmetry() {
        let mut buffer = vec![1.0; 256];
        apply_hamming_window(&mut buffer);

        for i in 0..buffer.len() / 2 {
            let diff = (buffer[i] - buffer[buffer.len() - 1 - i]).abs();
            assert!(diff < 0.0001, "Hamming should be symmetric");
        }
    }

    #[test]
    fn test_flat_top_symmetry() {
        let mut buffer = vec![1.0; 256];
        apply_flat_top_window(&mut buffer);

        for i in 0..buffer.len() / 2 {
            let diff = (buffer[i] - buffer[buffer.len() - 1 - i]).abs();
            assert!(diff < 0.0001, "Flat-top should be symmetric");
        }
    }

    #[test]
    fn test_window_two_samples() {
        let mut hann = vec![1.0; 2];
        apply_hann_window(&mut hann);
        assert!(hann[0] < 0.01 && hann[1] < 0.01, "Two-sample Hann endpoints near zero");

        let mut hamming = vec![1.0; 2];
        apply_hamming_window(&mut hamming);
        assert!(hamming[0] > 0.07, "Two-sample Hamming has raised endpoints");
    }

    #[test]
    fn test_window_preserves_sign() {
        let mut buffer = vec![-1.0; 128];
        apply_hann_window(&mut buffer);

        // All non-zero values should be negative (since input was negative)
        for &v in buffer.iter() {
            assert!(v <= 0.0, "Window should preserve sign");
        }
    }

    #[test]
    fn test_window_large_buffer() {
        let mut buffer = vec![1.0; 16384];
        apply_hann_window(&mut buffer);

        assert!(buffer[0] < 0.001);
        assert!(buffer[8192] > 0.99);
        assert!(buffer[16383] < 0.001);
    }

    #[test]
    fn test_window_odd_size() {
        let mut buffer = vec![1.0; 127]; // Odd size
        apply_hann_window(&mut buffer);

        // Should not panic and should be reasonably symmetric
        assert!(buffer[0] < 0.01);
        assert!(buffer[63] > 0.99); // Center
    }
}
