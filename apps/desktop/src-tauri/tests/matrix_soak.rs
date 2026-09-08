//! Sustained-load test for the 32-channel matrix.
//!
//! This runs the real per-pair processing path at a realistic block rate for a
//! sustained period and asserts that per-block processing time stays well
//! inside the block period. It does NOT prove the app runs dropout-free on real
//! hardware — no synthetic test can, because it does not contend with a real
//! driver, a real device clock, or the rest of the system. Spec §6 criterion 5
//! is only satisfied by running on a real interface.
//!
//! The block-budget test only enforces its timing assertion on an optimized
//! (release) build — a debug build is 2-3x slower and would fail a budget
//! sized for what actually ships. Run it with `just soak-matrix` (or
//! `cargo test --test matrix_soak --release -- --nocapture`) to see it
//! measure and assert; under a plain debug `cargo test` it is skipped.

use phonetic_lib::audio::{MeasurementPair, ReferenceSource, Routing};
use phonetic_lib::dsp::TransferFunctionProcessor;

const FS: f32 = 48_000.0;
const BLOCK: usize = 512;
const PAIRS: usize = 32;

/// The block period at 48 kHz / 512 samples is 10.67 ms. Processing must stay
/// far enough inside it to leave room for the driver and the rest of the system.
const BLOCK_PERIOD_MS: f64 = BLOCK as f64 / FS as f64 * 1000.0;
const BUDGET_FRACTION: f64 = 0.5;

// A debug build's per-block time is not the number that ships, and is
// slow enough (~2-3x) that asserting the release budget against it would
// fail on unoptimized code rather than on a real regression.
#[cfg_attr(
    debug_assertions,
    ignore = "timing budget is only meaningful on a release build; run `just soak-matrix`"
)]
#[test]
fn thirty_two_pairs_process_inside_the_block_budget() {
    let mut processors: Vec<TransferFunctionProcessor> = (0..PAIRS)
        .map(|_| TransferFunctionProcessor::new(2048, FS))
        .collect();

    let block: Vec<f32> = (0..BLOCK)
        .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / FS).sin())
        .collect();

    // Warm up so the first block's allocation does not count.
    for p in processors.iter_mut() {
        p.process(&block, &block);
    }

    let blocks = (FS as usize / BLOCK) * 30; // 30 seconds of audio
    let start = std::time::Instant::now();
    for _ in 0..blocks {
        for p in processors.iter_mut() {
            p.process(&block, &block);
        }
    }
    let elapsed = start.elapsed();

    let per_block_ms = elapsed.as_secs_f64() * 1000.0 / blocks as f64;
    let budget = BLOCK_PERIOD_MS * BUDGET_FRACTION;
    println!(
        "{PAIRS} pairs: {per_block_ms:.3} ms/block measured, {budget:.3} ms budget \
         ({BLOCK_PERIOD_MS:.2} ms block period)"
    );
    assert!(
        per_block_ms < budget,
        "{PAIRS} pairs took {per_block_ms:.3} ms per block against a {budget:.3} ms budget \
         ({BLOCK_PERIOD_MS:.2} ms block period)"
    );
}

/// Routing swaps under sustained processing must never block the processing
/// side — the property the whole snapshot design exists for.
#[test]
fn routing_swaps_do_not_stall_processing() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let engine = Arc::new(phonetic_lib::audio::AudioEngine::new());
    let stop = Arc::new(AtomicBool::new(false));

    let writer = {
        let engine = Arc::clone(&engine);
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            let mut n = 0usize;
            let mut successes = 0usize;
            while !stop.load(Ordering::Relaxed) {
                let routing = Routing {
                    pairs: vec![MeasurementPair {
                        id: format!("p{}", n % 4),
                        name: "swap".into(),
                        measurement_channel: n % 8,
                        reference: ReferenceSource::Generator,
                        delay_samples: 0,
                        calibration_offset_db: 0.0,
                        enabled: true,
                    }],
                };
                if engine.set_routing(routing).is_ok() {
                    successes += 1;
                }
                n += 1;
            }
            (n, successes)
        })
    };

    // Let the writer thread actually get scheduled and start contending
    // before measuring. Without this, the worst-case sample is dominated by
    // one-off thread-startup scheduling noise rather than the steady-state
    // contention the test exists to bound.
    std::thread::sleep(std::time::Duration::from_millis(10));

    let mut worst = std::time::Duration::ZERO;
    for _ in 0..200_000 {
        let start = std::time::Instant::now();
        let _routing = engine.routing();
        worst = worst.max(start.elapsed());
    }

    stop.store(true, Ordering::Relaxed);
    let (swaps, successes) = writer.join().unwrap();

    println!("{swaps} routing swaps while reading; worst-case read was {worst:?}");

    assert!(swaps > 0, "the writer never ran");
    assert_eq!(
        successes, swaps,
        "some routing swaps were rejected by validation, so the swap loop was not \
         actually exercising the write path"
    );
    assert!(
        worst.as_micros() < 1000,
        "worst-case routing read was {worst:?} while routing was being swapped"
    );
}
