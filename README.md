# Phonetic

Modern, Apple Silicon-native audio measurement system. A rival to [AFMG SysTune](https://www.afmg.eu/en/afmg-systune#features) with real-time signal processing and a modern UI.

## Features

### Core Measurements

- Real-time FFT / spectrum analysis with peak hold
- Transfer function (input vs output comparison) with auto-zoom
- Impulse response measurement with RT60 estimation
- Phase response and group delay
- SPL metering (dBFS, dB SPL with calibration)
- Delay finder (cross-correlation based)
- Coherence display

### Signal Generator

- Sine wave (20Hz - 20kHz)
- Pink noise
- White noise
- Logarithmic sweep

### Calibration

- Microphone calibration file import (.cal format)
- A-weighting and C-weighting filters
- Sensitivity and preamp gain compensation
- Reference curves overlay (ISO 226, X-curve, house curve, weighting curves)

### Export

- CSV, JSON, and SMAART-compatible TXT formats
- Transfer function and impulse response export

### UI

- Modern dark/light theme with full support across all visualizations
- 60fps WebGL or uPlot spectrum visualization (user selectable)
- Glassmorphism design
- Settings dialog with FFT size and backend selection

## Measurement Accuracy

An audio analyzer is only worth as much as your trust in its numbers. Most measurement
software asks you to take that on faith. Phonetic publishes its working.

The suite in [`apps/desktop/src-tauri/tests/dsp_accuracy.rs`](apps/desktop/src-tauri/tests/dsp_accuracy.rs)
measures signals whose correct answer is known **analytically** — derived from theory, not
captured from an earlier run of this same software. A closed-form filter, a synthesised decay
with a chosen T60, a published standards table. If the DSP drifts, these fail with a number you
can read off against the maths.

| Property under test | Analytical reference | Tolerance | Measured |
|---|---|---|---|
| Identity path (ref = meas) | 0 dB, coherence 1.0 | 0.01 dB | ✅ pass |
| Known gain (×0.5) | −6.0206 dB | 0.01 dB | ✅ pass |
| Inverted polarity | 180° | 1° | ✅ pass |
| **One-pole lowpass response** | `H(ω) = (1−a)/(1−a·e^−jω)` | **0.5 dB / 2°, 100 Hz–15 kHz** | ✅ pass |
| Coherence vs. noise | `γ² = SNR/(1+SNR)` at 0, 10, 20 dB | 0.03 | ✅ pass |
| Pure delay (1 ms) | flat 1.00 ms group delay | 0.05 ms | ✅ pass |
| A-weighting | IEC 61672-1 Table 3, 8 frequencies | 0.2 dB | ✅ pass |
| RT60 estimation | synthesised T60 = 200 ms | 10% | ✅ pass |
| Multi-window LF resolution | `H(ω)` of a one-pole at 25 Hz | 1.0 dB | ✅ pass |

The fourth row is the load-bearing one. A one-pole lowpass has an exact closed-form magnitude
and phase response. Recovering it to within 0.5 dB and 2° across the audio band means the
dual-FFT engine — windowing, overlap, cross-spectrum averaging, phase unwrapping — is correct,
not merely self-consistent.

**These assertions have teeth.** Perturbing the reference filter coefficient by 2% (0.90 → 0.88)
fails the magnitude check at 1.673 dB, so the suite detects errors well below the threshold at
which a listener or a rival analyzer would notice one.

Run them yourself:

```bash
just test-rust                                  # full suite
cargo test --test dsp_accuracy -- --nocapture   # accuracy suite only
```

## Headless measurement

Phonetic ships a scriptable CLI, which no other analyzer in this category offers.
Measure from a recording, verify an install from a script, or regression-test a room
after a change:

```bash
phonetic-cli measure --file room.wav --reference-channel 0 --measurement-channel 1
phonetic-cli measure --file room.wav --format csv > room.csv
phonetic-cli devices
phonetic-cli selftest        # verify the DSP against a known signal
```

## Tech Stack

- **Backend**: Rust (Tauri 2 + CPAL + realfft)
- **Frontend**: React 19 + TypeScript + TailwindCSS 4
- **Build**: Turborepo + Bun

## Development

```bash
# Install dependencies
just install

# Run in development mode
just dev

# Build for production
just build-release

# Run checks
just check

# Run tests
just test
```

## Project Structure

```
phonetic/
├── apps/
│   └── desktop/          # Tauri app
│       ├── src/          # React frontend
│       └── src-tauri/    # Rust backend
├── packages/             # Shared packages
├── justfile             # Task runner
└── turbo.json           # Monorepo config
```

## Requirements

- macOS 11.0+ (Apple Silicon optimized)
- Rust 1.70+
- Bun 1.0+

## License

Proprietary - Commercial software with license key activation.
