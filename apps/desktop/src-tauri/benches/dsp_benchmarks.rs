//! DSP Performance Benchmarks
//!
//! Benchmarks for critical DSP operations using criterion.
//! Run with: cargo bench --package phonetic

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use phonetic_lib::dsp::{
    apply_blackman_harris_window, apply_flat_top_window, apply_hamming_window, apply_hann_window,
    DelayFinder, FftProcessor, ImpulseResponseProcessor, ReferenceCurve, ReferenceCurveType,
    SplCalculator, TransferFunctionProcessor,
};
use std::f32::consts::PI;

// ============================================================================
// Window Function Benchmarks
// ============================================================================

fn bench_window_functions(c: &mut Criterion) {
    let mut group = c.benchmark_group("window_functions");

    for size in [512, 1024, 2048, 4096, 8192, 16384].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::new("hann", size), size, |b, &size| {
            let mut buffer = vec![1.0f32; size];
            b.iter(|| {
                buffer.fill(1.0);
                apply_hann_window(black_box(&mut buffer));
            });
        });

        group.bench_with_input(BenchmarkId::new("blackman_harris", size), size, |b, &size| {
            let mut buffer = vec![1.0f32; size];
            b.iter(|| {
                buffer.fill(1.0);
                apply_blackman_harris_window(black_box(&mut buffer));
            });
        });

        group.bench_with_input(BenchmarkId::new("hamming", size), size, |b, &size| {
            let mut buffer = vec![1.0f32; size];
            b.iter(|| {
                buffer.fill(1.0);
                apply_hamming_window(black_box(&mut buffer));
            });
        });

        group.bench_with_input(BenchmarkId::new("flat_top", size), size, |b, &size| {
            let mut buffer = vec![1.0f32; size];
            b.iter(|| {
                buffer.fill(1.0);
                apply_flat_top_window(black_box(&mut buffer));
            });
        });
    }

    group.finish();
}

// ============================================================================
// FFT Processor Benchmarks
// ============================================================================

fn bench_fft_processor(c: &mut Criterion) {
    let mut group = c.benchmark_group("fft_processor");

    for fft_size in [512, 1024, 2048, 4096, 8192].iter() {
        let sample_rate = 48000.0;

        // Generate test signal: 1kHz sine wave
        let samples: Vec<f32> = (0..(fft_size * 2))
            .map(|i| (2.0 * PI * 1000.0 * i as f32 / sample_rate).sin())
            .collect();

        group.throughput(Throughput::Elements(samples.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("process", fft_size),
            &samples,
            |b, samples| {
                let mut processor = FftProcessor::new(*fft_size);
                b.iter(|| {
                    processor.process(black_box(samples));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("get_magnitude", fft_size),
            fft_size,
            |b, &fft_size| {
                let processor = FftProcessor::new(fft_size);
                b.iter(|| {
                    let _mag = black_box(processor.get_magnitude_db());
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Transfer Function Processor Benchmarks
// ============================================================================

fn bench_transfer_function(c: &mut Criterion) {
    let mut group = c.benchmark_group("transfer_function");

    for fft_size in [512, 1024, 2048, 4096].iter() {
        let sample_rate = 48000.0;

        // Generate test signals
        let samples: Vec<f32> = (0..(fft_size * 4))
            .map(|i| (2.0 * PI * 1000.0 * i as f32 / sample_rate).sin())
            .collect();

        group.throughput(Throughput::Elements(samples.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("process", fft_size),
            &samples,
            |b, samples| {
                let mut processor = TransferFunctionProcessor::new(*fft_size, sample_rate);
                b.iter(|| {
                    processor.process(black_box(samples), black_box(samples));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("get_data", fft_size),
            fft_size,
            |b, &fft_size| {
                let mut processor = TransferFunctionProcessor::new(fft_size, sample_rate);
                let samples: Vec<f32> = (0..(fft_size * 4))
                    .map(|i| (2.0 * PI * 1000.0 * i as f32 / sample_rate).sin())
                    .collect();
                processor.process(&samples, &samples);
                b.iter(|| {
                    let _data = black_box(processor.get_data());
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("reset_averaging", fft_size),
            fft_size,
            |b, &fft_size| {
                let mut processor = TransferFunctionProcessor::new(fft_size, sample_rate);
                b.iter(|| {
                    processor.reset_averaging();
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Delay Finder Benchmarks
// ============================================================================

fn bench_delay_finder(c: &mut Criterion) {
    let mut group = c.benchmark_group("delay_finder");

    for fft_size in [2048, 4096, 8192].iter() {
        let sample_rate = 48000.0;

        // Generate chirp signal for better correlation
        let reference: Vec<f32> = (0..(fft_size / 2))
            .map(|i| {
                let t = i as f32 / sample_rate;
                let freq = 500.0 + (5000.0 - 500.0) * t * 20.0;
                (2.0 * PI * freq * t).sin()
            })
            .collect();

        // Delayed version
        let delay = 100;
        let mut measured = vec![0.0; reference.len()];
        measured[delay..].copy_from_slice(&reference[..reference.len() - delay]);

        group.throughput(Throughput::Elements(reference.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("find_delay", fft_size),
            &(&reference, &measured),
            |b, (reference, measured)| {
                let mut finder = DelayFinder::new(*fft_size, sample_rate, 100.0);
                b.iter(|| {
                    let _result = finder.find_delay(black_box(reference), black_box(measured));
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Impulse Response Benchmarks
// ============================================================================

fn bench_impulse_response(c: &mut Criterion) {
    let mut group = c.benchmark_group("impulse_response");

    for fft_size in [2048, 4096, 8192].iter() {
        let sample_rate = 48000.0;

        // Generate test samples
        let samples: Vec<f32> = (0..512)
            .map(|i| (2.0 * PI * 1000.0 * i as f32 / sample_rate).sin())
            .collect();

        group.bench_with_input(
            BenchmarkId::new("add_samples", fft_size),
            &samples,
            |b, samples| {
                let mut processor = ImpulseResponseProcessor::new(*fft_size, sample_rate);
                processor.start_capture();
                b.iter(|| {
                    processor.add_samples(black_box(samples), black_box(samples));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("compute_ir", fft_size),
            fft_size,
            |b, &fft_size| {
                let mut processor = ImpulseResponseProcessor::new(fft_size, sample_rate);
                processor.start_capture();
                // Fill with enough samples
                let samples: Vec<f32> = (0..(fft_size * 2))
                    .map(|i| (2.0 * PI * 1000.0 * i as f32 / sample_rate).sin())
                    .collect();
                for _ in 0..10 {
                    processor.add_samples(&samples, &samples);
                }
                processor.stop_capture();
                b.iter(|| {
                    let _ir = black_box(processor.compute_impulse_response());
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Reference Curve Benchmarks
// ============================================================================

fn bench_reference_curves(c: &mut Criterion) {
    let mut group = c.benchmark_group("reference_curves");

    let num_points = [32, 64, 128, 256, 512, 1024];
    let curve_types = [
        ReferenceCurveType::Flat,
        ReferenceCurveType::Iso226_40Phon,
        ReferenceCurveType::XCurve,
        ReferenceCurveType::AWeighting,
    ];

    for &points in &num_points {
        // Generate log-spaced frequencies
        let frequencies: Vec<f64> = (0..points)
            .map(|i| {
                let t = i as f64 / (points - 1) as f64;
                10.0_f64.powf(1.301 + t * (4.301 - 1.301)) // 20 Hz to 20 kHz
            })
            .collect();

        group.throughput(Throughput::Elements(points as u64));

        for curve_type in &curve_types {
            group.bench_with_input(
                BenchmarkId::new(format!("{:?}", curve_type), points),
                &frequencies,
                |b, frequencies| {
                    b.iter(|| {
                        let _curve =
                            black_box(ReferenceCurve::generate(*curve_type, black_box(frequencies)));
                    });
                },
            );
        }
    }

    group.finish();
}

// ============================================================================
// SPL Calculator Benchmarks
// ============================================================================

fn bench_spl_calculator(c: &mut Criterion) {
    let mut group = c.benchmark_group("spl_calculator");

    group.bench_function("dbfs_to_spl", |b| {
        let calculator = SplCalculator::default();
        b.iter(|| {
            let _spl = calculator.dbfs_to_spl(black_box(-20.0));
        });
    });

    group.bench_function("dbfs_to_spl_batch", |b| {
        let calculator = SplCalculator::default();
        let dbfs_values: Vec<f32> = (-100..0).map(|x| x as f32).collect();
        b.iter(|| {
            for &dbfs in &dbfs_values {
                let _spl = calculator.dbfs_to_spl(black_box(dbfs));
            }
        });
    });

    group.finish();
}

// ============================================================================
// Real-time Processing Simulation Benchmarks
// ============================================================================

fn bench_realtime_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("realtime_simulation");
    group.sample_size(50); // Fewer samples for longer benchmarks

    let sample_rate = 48000.0;
    let buffer_sizes = [128, 256, 512];

    for &buffer_size in &buffer_sizes {
        // Simulate 100ms of audio processing (4800 samples at 48kHz)
        let total_samples = (sample_rate * 0.1) as usize;
        let num_buffers = total_samples / buffer_size;

        // Generate test audio
        let audio_buffers: Vec<Vec<f32>> = (0..num_buffers)
            .map(|buf_idx| {
                (0..buffer_size)
                    .map(|i| {
                        let sample_idx = buf_idx * buffer_size + i;
                        (2.0 * PI * 1000.0 * sample_idx as f32 / sample_rate).sin()
                    })
                    .collect()
            })
            .collect();

        group.throughput(Throughput::Elements(total_samples as u64));

        group.bench_with_input(
            BenchmarkId::new("fft_stream", buffer_size),
            &audio_buffers,
            |b, buffers| {
                let mut processor = FftProcessor::new(2048);
                b.iter(|| {
                    for buffer in buffers {
                        processor.process(black_box(buffer));
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("transfer_function_stream", buffer_size),
            &audio_buffers,
            |b, buffers| {
                let mut processor = TransferFunctionProcessor::new(2048, sample_rate);
                b.iter(|| {
                    for buffer in buffers {
                        processor.process(black_box(buffer), black_box(buffer));
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Memory Allocation Benchmarks
// ============================================================================

fn bench_allocations(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocations");

    for fft_size in [1024, 2048, 4096, 8192].iter() {
        group.bench_with_input(
            BenchmarkId::new("fft_processor_new", fft_size),
            fft_size,
            |b, &fft_size| {
                b.iter(|| {
                    let _processor = black_box(FftProcessor::new(fft_size));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("transfer_function_new", fft_size),
            fft_size,
            |b, &fft_size| {
                b.iter(|| {
                    let _processor = black_box(TransferFunctionProcessor::new(fft_size, 48000.0));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("delay_finder_new", fft_size),
            fft_size,
            |b, &fft_size| {
                b.iter(|| {
                    let _finder = black_box(DelayFinder::new(fft_size, 48000.0, 100.0));
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_window_functions,
    bench_fft_processor,
    bench_transfer_function,
    bench_delay_finder,
    bench_impulse_response,
    bench_reference_curves,
    bench_spl_calculator,
    bench_realtime_simulation,
    bench_allocations,
);

criterion_main!(benches);
