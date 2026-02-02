# Phonetic

Modern, Apple Silicon-native audio measurement system. A rival to [AFMG SysTune](https://www.afmg.eu/en/afmg-systune#features) with real-time signal processing and a modern UI.

## Features

### Core Measurements
- Real-time FFT / spectrum analysis
- Transfer function (input vs output comparison)
- Impulse response measurement
- Phase response and group delay
- SPL metering (dBFS, dB SPL with calibration)
- Multi-channel support (8+ channels)

### Signal Generator
- Sine wave (20Hz - 20kHz)
- Pink noise
- White noise
- Log sweep

### UI
- Modern dark/light theme
- 60fps WebGL visualization
- Glassmorphism design

## Tech Stack

- **Backend**: Rust (Tauri 2 + CPAL + realfft)
- **Frontend**: React 19 + TypeScript + TailwindCSS 4
- **Build**: Turborepo + Bun

## Development

```bash
# Install dependencies
just install

# Run in development mode
just tauri-dev

# Build for production
just tauri-build

# Run checks
just check-all

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
