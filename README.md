# Phonetic

Phonetic is a free audio measurement app for macOS and Linux. Plug in an audio interface and a
measurement microphone, and it shows you what a loudspeaker or a room actually does to sound:
frequency response, phase, delay, reverberation time, sound pressure level, noise exposure and
room noise ratings. It does what Smaart or SysTune do, costs nothing, and the source is open.

It is for live sound engineers, installers, studio builders, acousticians and anyone who wants
numbers instead of guesses when tuning a PA, aligning speakers or treating a room.

## How it works

1. Phonetic plays a test signal (sine, noise or sweep) through your audio interface to the
   loudspeaker, or you use the music that is already playing.
2. A measurement microphone picks up what arrives in the room.
3. Phonetic compares what went out with what came back, in real time, and draws the result:
   magnitude and phase over frequency, impulse response, coherence, delay.
4. Readings can be calibrated to real dB SPL with the calibration file that came with your
   microphone, saved as sessions, and exported as CSV, JSON or Smaart-compatible text.

No account, no internet connection, no telemetry. Everything runs on your computer.

## Install

Download one file from the
[latest release](https://github.com/morrieinmaas/phonetic/releases/latest). The numbers in the
file name are the version; pick the one for your computer.

| Your computer | File to download |
|---|---|
| Mac with Apple chip (M1, M2, M3, M4...) | `Phonetic_x.y.z_aarch64.dmg` |
| Mac with Intel chip | `Phonetic_x.y.z_x64.dmg` |
| Ubuntu, Debian, Linux Mint, Pop!_OS | `Phonetic_x.y.z_amd64.deb` |
| Fedora, RHEL, openSUSE | `Phonetic-x.y.z-1.x86_64.rpm` |
| Any other Linux | `Phonetic_x.y.z_amd64.AppImage` |
| Linux on an ARM machine (Raspberry Pi 5, Asahi, Ampere) | the `arm64` or `aarch64` variant of the above |

Not sure which Mac you have? Apple menu → **About This Mac**. The line that says "Chip" means
Apple chip; "Processor ... Intel" means Intel. Not sure about Linux? Open a terminal and run
`uname -m`: `x86_64` means amd64, `aarch64` means arm64.

Windows is not supported.

### macOS

Phonetic is not signed with an Apple Developer ID, because that costs 99 dollars a year for
free software. macOS therefore blocks the first launch and you have to approve it once. Every
release is built in public by GitHub from the tagged source, so what you approve is exactly
what is in this repository.

1. Open the `.dmg` and drag **Phonetic** into **Applications**.
2. Double-click Phonetic in Applications. macOS says it could not verify the app is free of
   malware. Click **Done** (not Move to Trash).
3. Open **System Settings → Privacy & Security**, scroll down to **Security**. There is a line
   saying Phonetic was blocked. Click **Open Anyway** and confirm with your password or Touch ID.
4. Double-click Phonetic again and click **Open**. When it asks for microphone access, allow it.

You only do this once. Updates installed the same way need the same four steps again.
Needs macOS 11 or later. Right-click → Open does not bypass this on macOS 15 and later.

<details>
<summary>Terminal alternative to steps 2 and 3</summary>

```bash
xattr -dr com.apple.quarantine /Applications/Phonetic.app
```
</details>

### Linux

**Ubuntu, Debian, Mint, Pop!_OS**: double-click the `.deb` file. Your software installer opens
and shows an **Install** button. Click it, enter your password, then find Phonetic in your
application menu.

**Fedora, RHEL, openSUSE**: double-click the `.rpm` file and click **Install** in the window
that opens.

**AppImage, any distribution**: right-click the downloaded file, choose **Properties**, and
allow it to run as a program (GNOME: Permissions tab; KDE: "Is executable"). Then double-click
it. Nothing gets installed; the file is the app. Move it wherever you like.

<details>
<summary>Terminal alternative</summary>

```bash
sudo apt install ./Phonetic_x.y.z_amd64.deb                         # Debian, Ubuntu
sudo dnf install ./Phonetic-x.y.z-1.x86_64.rpm                      # Fedora, RHEL
chmod +x Phonetic_x.y.z_amd64.AppImage && ./Phonetic_x.y.z_amd64.AppImage
```

The AppImage needs FUSE 2 (`sudo apt install libfuse2` on Ubuntu 22.04 and later) or run it
with `--appimage-extract-and-run`.
</details>

Works on Ubuntu 22.04, Debian 12, Fedora 36 and anything newer. Sound goes through ALSA, which
PipeWire and PulseAudio both provide, so any audio interface your desktop can see will work.

### First measurement

1. Open Phonetic and pick your audio interface as input and output.
2. Connect the interface output to the loudspeaker and the measurement microphone to an input.
3. Start the generator with pink noise at a modest level.
4. Set the reference channel to the generator and the measurement channel to the microphone.
   The transfer function appears; use the delay finder once so the phase trace makes sense.

## What it measures

- **Spectrum**: real-time FFT with peak hold, 60 fps rendering, dark and light themes.
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
- **Sessions and export**: save and reload sessions; export CSV, JSON and Smaart-compatible TXT.

On macOS a CoreAudio backend with lower latency and exclusive mode is available in addition to
the default cross-platform backend, which is what Linux uses.

## Headless CLI

`phonetic-cli` is a separate download in every release (`phonetic-cli-macos-arm64`,
`phonetic-cli-linux-x64` and so on). It runs the same DSP without the window, for measuring
from a recording or scripting a check.

```bash
phonetic-cli measure --file room.wav --reference-channel 0 --measurement-channel 1
phonetic-cli measure --file room.wav --format csv > room.csv
phonetic-cli devices
phonetic-cli selftest        # checks the DSP against a known signal, exit code 0 on pass
```

On macOS strip the quarantine flag first: `xattr -d com.apple.quarantine phonetic-cli-macos-arm64`.

## How accurate is it

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

**Standards caveats.** The A- and C-weighting filters follow the IEC 61672-1 analogue prototype
and track it within about 1 dB up to 10 kHz, but deviate by roughly 3 dB at 16 kHz and have not
been checked against the Class 1 tolerance table; treat the weighting as nominal. The NC and NR
tables are transcribed from published references, not the paywalled standards. The noise limits
are editable presets, not a legal database: Phonetic shows a level against the limit you
configured and issues no compliance verdict. Confirm any figure against the regulation or
contract that binds you.

## Build from source

For developers. Rust (stable), Bun 1.x and [just](https://github.com/casey/just); Rust and Bun
are installed on demand by `just install` if missing.

```bash
just install-deps-linux   # Linux only: WebKitGTK, GTK, ALSA and friends (Debian/Ubuntu names)
just install              # Rust, Bun, JS and Rust dependencies
just dev                  # run the app with hot reload
just check                # Biome, clippy, rustfmt
just test                 # Rust and TypeScript suites
just dist                 # installers for this OS: .app + .dmg, or .deb + .rpm + .AppImage
just cli-release          # the headless CLI only
```

Every push runs lint, both test suites, the CLI selftest and a debug bundle on macOS arm64,
macOS Intel, Linux x86_64 and Linux arm64. `just tag x.y.z` pushes a tag, which builds the
release installers on all four and publishes them with a changelog. See
[CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request.

## License

Apache License 2.0 with the [Commons Clause](https://commonsclause.com) condition; see
[LICENSE](LICENSE). In short: use it, modify it, redistribute it, use it on paid jobs, but
you may not sell Phonetic itself or a product or service whose value comes substantially
from it. Want to? Or anything else? Write to mo [at] traect [dot] dev.

Phonetic is free software and comes with **no warranty or guarantee of any kind**. Its
readings are not certified; verify independently before relying on them for compliance,
safety or legal purposes.
