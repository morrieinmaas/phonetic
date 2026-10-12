# Phonetic

Audio measurement for live sound and room acoustics on macOS and Linux. Dual-FFT transfer
function, impulse response, SPL and noise-dose metering, room criteria, a 32-channel matrix,
and a scriptable CLI, with a DSP core that is tested against closed-form answers rather than
against itself. An open alternative to AFMG SysTune and Smaart.

## Install

Grab the file for your machine from the
[latest release](https://github.com/morrieinmaas/phonetic/releases/latest).

| Platform | Download |
|---|---|
| macOS, Apple Silicon (M1 and later) | `Phonetic_<version>_aarch64.dmg` |
| macOS, Intel | `Phonetic_<version>_x64.dmg` |
| Linux x86_64 | `Phonetic_<version>_amd64.deb`, `Phonetic-<version>-1.x86_64.rpm` or `Phonetic_<version>_amd64.AppImage` |
| Linux arm64 | `Phonetic_<version>_arm64.deb`, `Phonetic-<version>-1.aarch64.rpm` or `Phonetic_<version>_aarch64.AppImage` |
| Headless CLI, any of the above | `phonetic-cli-<platform>` |

### macOS

Phonetic is not signed with an Apple Developer ID and not notarised, so macOS blocks the first
launch. Every release is built by the public GitHub Actions workflow from the tagged source, so
you can audit exactly what you are approving.

1. Open the `.dmg` and drag Phonetic to Applications.
2. Double-click Phonetic. macOS says it could not verify the app. Click **Done**.
3. Open **System Settings → Privacy & Security**, scroll to **Security**, click **Open Anyway**
   next to the Phonetic message, and confirm with your password or Touch ID.
4. Launch Phonetic again and allow microphone access. The approval is remembered.

Right-click → Open no longer bypasses Gatekeeper on macOS 15 and later. The Terminal
equivalent of steps 2 and 3 is to strip the quarantine flag that the download added:

```bash
xattr -dr com.apple.quarantine /Applications/Phonetic.app
xattr -d  com.apple.quarantine ~/Downloads/phonetic-cli-macos-arm64   # CLI, if you use it
```

Requires macOS 11 or later.

### Linux

```bash
sudo apt install ./Phonetic_<version>_amd64.deb          # Debian, Ubuntu
sudo dnf install ./Phonetic-<version>-1.x86_64.rpm       # Fedora, RHEL
chmod +x Phonetic_<version>_amd64.AppImage && ./Phonetic_<version>_amd64.AppImage   # anything else
```

Needs WebKitGTK 4.1 (Ubuntu 22.04+, Debian 12+, Fedora 36+). Audio goes through ALSA, which
PipeWire and PulseAudio both provide. The AppImage needs FUSE 2 (`libfuse2` on Ubuntu) or run
it with `--appimage-extract-and-run`.

Windows is not supported.

## What it measures

- **Spectrum**: real-time FFT with peak hold, 60 fps WebGL or uPlot rendering, dark and light themes.
- **Transfer function**: magnitude, phase, group delay and coherence between any reference and
  measurement input, with averaging, smoothing, multi-window low-frequency resolution and auto-zoom.
- **Impulse response** with RT60 estimate; **delay finder** by cross-correlation.
- **Signal generator**: sine, pink noise, white noise, logarithmic sweep.
- **SPL and calibration**: dBFS and calibrated dB SPL, `.cal` microphone files, sensitivity and
  preamp gain, A- and C-weighting, reference overlays (ISO 226, X-curve, house curve).
- **Channel matrix**: up to 32 inputs, several measurement pairs at once, each with its own delay
  and calibration offset, referenced to the generator or to another input.
- **Room criteria**: NC (ANSI S12.2) and NR (ISO 1996) by the tangency method, plus RC Mark II.
  Needs calibrated SPL.
- **Noise dose**: LEQ, LAeq, LCeq, LAFmax, LApeak, LCpeak with continuous CSV logging and exposure
  dose against editable limit presets (EU Directive 2003/10/EC action values ship as defaults).
- **Sessions and export**: save and reload sessions; export CSV, JSON and SMAART-compatible TXT.

The CoreAudio backend (lower latency, exclusive mode, real-time thread priority) is macOS only.
The cross-platform CPAL backend is the default on both systems and is what Linux uses.

## Headless CLI

`phonetic-cli` runs the same DSP without the window. Use it to measure from a recording,
verify an install from a script, or regression-test a room after a change.

```bash
phonetic-cli measure --file room.wav --reference-channel 0 --measurement-channel 1
phonetic-cli measure --file room.wav --format csv > room.csv
phonetic-cli devices
phonetic-cli selftest        # checks the DSP against a known signal, exit code 0 on pass
```

## Measurement accuracy

The suite in [`apps/desktop/src-tauri/tests/dsp_accuracy.rs`](apps/desktop/src-tauri/tests/dsp_accuracy.rs)
feeds the engine signals whose correct answer is known analytically, so a drift in the DSP fails
with a number you can check against the maths rather than against an earlier run of the same code.

| Property under test | Analytical reference | Tolerance |
|---|---|---|
| Identity path (ref = meas) | 0 dB, coherence 1.0 | 0.01 dB |
| Known gain (×0.5) | −6.0206 dB | 0.01 dB |
| Inverted polarity | 180° | 1° |
| **One-pole lowpass response** | `H(ω) = (1−a)/(1−a·e^−jω)` | **0.5 dB / 2°, 100 Hz–15 kHz** |
| Coherence vs. noise | `γ² = SNR/(1+SNR)` at 0, 10, 20 dB | 0.03 |
| Pure delay (1 ms) | flat 1.00 ms group delay | 0.05 ms |
| A-weighting | IEC 61672-1 Table 3, 8 frequencies | 0.2 dB |
| RT60 estimation | synthesised T60 = 200 ms | 10% |
| Multi-window LF resolution | `H(ω)` of a one-pole at 25 Hz | 1.0 dB |

The lowpass row is the load-bearing one: recovering a closed-form magnitude and phase to 0.5 dB
and 2° across the band means windowing, overlap, cross-spectrum averaging and phase unwrapping
are right, not merely self-consistent. Perturbing the reference coefficient by 2 % fails the
check at 1.67 dB, so the suite catches errors well below what a listener would notice.

```bash
just test-accuracy     # accuracy suite only
just test              # everything
```

**Standards caveats.** The A- and C-weighting filters follow the IEC 61672-1 analogue prototype
via a prewarped bilinear transform and track it within about 1 dB up to 10 kHz, but deviate by
roughly 3 dB at 16 kHz and have not been checked against the Class 1 tolerance table; treat the
weighting as nominal. The NC and NR tables are transcribed from published references, not the
paywalled standards (citations in `dsp/room_criteria.rs`). The noise limits are editable presets,
not a legal database: Phonetic shows a level against the limit you configured and issues no
compliance verdict. Confirm any figure against the regulation or contract that binds you.

## Build from source

Rust (stable), Bun 1.x and [just](https://github.com/casey/just). Rust and Bun are installed
on demand by `just install` if missing.

```bash
just install-deps-linux   # Linux only: WebKitGTK, GTK, ALSA and friends (Debian/Ubuntu names)
just install              # Rust, Bun, JS and Rust dependencies
just dev                  # run the app with hot reload
just check                # Biome, clippy, rustfmt
just test                 # Rust and TypeScript suites
just dist                 # installers for this OS: .app + .dmg, or .deb + .rpm + .AppImage
just cli-release          # the headless CLI only
```

Rust 1.70+ and Bun 1.0+ are the minimums. Tauri 2 drives the window; the DSP is plain Rust.

## Releases

Every push and pull request runs lint, both test suites, the CLI selftest and a debug bundle on
macOS arm64, macOS Intel, Linux x86_64 and Linux arm64. Pushing a tag builds release installers on
all four and publishes them with a changelog:

```bash
just tag 0.2.0            # creates and pushes v0.2.0
```

## License

See [LICENSE](LICENSE).
