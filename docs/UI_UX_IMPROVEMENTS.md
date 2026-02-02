# Phonetic UI/UX Improvements

## Overview

This document outlines the UI/UX issues identified in the current Phonetic implementation and the recommended fixes to achieve professional audio measurement software standards (comparable to SMAART, SysTune, REW).

**Last Updated**: 2026-02-02

---

## Implementation Progress Summary

### ✅ Completed (Phase 1 & 2)

| Feature | Priority | Files | Tests |
|---------|----------|-------|-------|
| Delay Finder (Cross-correlation) | P0 | `dsp/delay_finder.rs` | 5 |
| Impulse Response (Deconvolution) | P1 | `dsp/impulse_response.rs` | 4 |
| SPL Calibration (.cal parsing, A/C-weighting) | P2 | `dsp/calibration.rs` | 7 |
| Real-time Thread Priority (macOS Mach API) | P2 | `audio/realtime.rs` | 1 |
| Measurement Export (CSV/JSON/TXT) | P2 | `dsp/export.rs` | 3 |
| UI Axis Fixes | P1 | `spectrum-analyzer.tsx`, `transfer-function-display.tsx` | - |
| Generator Command Fix | P1 | `services/audio-api.ts` | - |
| CoreAudio Backend (direct HAL access) | P1 | `audio/coreaudio_backend.rs` | 2 |
| WebGL Spectrum Visualization | P3 | `webgl-spectrum.tsx` | - |

**Total**: 66 tests passing, all checks (lint, clippy, format) passing

### 🔄 Remaining Work

| Feature | Priority | Status |
|---------|----------|--------|
| CoreAudio stream integration | P1 | Backend ready, needs AudioEngine integration |
| Exclusive mode audio access | P2 | Planned |
| WebGL waterfall display | P3 | Scaffold exists, needs implementation |

---

## Implemented DSP APIs

### Delay Finder Commands
```typescript
findDelay(reference: number[], measured: number[]) → DelayResult
setDelayFinderMaxDelay(maxDelayMs: number)
setDelayFinderThreshold(threshold: number)
```

### Impulse Response Commands
```typescript
startImpulseCapture() / stopImpulseCapture() → boolean
addImpulseSamples(reference: number[], measured: number[])
getImpulseCaptureProgress() → number
isImpulseCaptureComplete() → boolean
computeImpulseResponse() → ImpulseResponseData
clearImpulseCapture()
```

### SPL Calibration Commands
```typescript
loadCalibrationFile(content: string, name: string)
setCalibrationSensitivity(sensitivityDb: number)
setPreampGain(gainDb: number)
setAWeighting(enabled: boolean)
dbfsToSpl(dbfs: number) → number
calibrateWithReference(measuredDbfs: number, referenceSplDb: number)
getCalibrationInfo() → MicrophoneCalibration
```

### Export Commands
```typescript
exportTransferFunction(format: "csv"|"json"|"txt", filePath: string, name?: string, notes?: string)
exportImpulseResponse(format: "csv"|"json"|"txt", filePath: string, name?: string, notes?: string)
```

---

## Current Issues (Status)

### 1. Spectrum Analyzer Problems ✅ FIXED

| Issue | Status | Solution |
|-------|--------|----------|
| ~~Y-axis auto-scaling~~ | ✅ Fixed | Added `auto: false` to scales |
| ~~X-axis labels missing~~ | ✅ Fixed | Added explicit `splits` array + `labelSize`, `size`, `gap` |
| ~~Container height~~ | ✅ Fixed | Updated padding and min heights |
| Data display | ✅ Working | Real-time spectrum with log frequency scale |

### 2. Transfer Function Display Problems ✅ FIXED

| Issue | Status | Solution |
|-------|--------|----------|
| ~~Y-axis range~~ | ✅ Fixed | Force `-40 to +20 dB` with `auto: false` |
| ~~Phase axis~~ | ✅ Fixed | Force `-180° to +180°` with `auto: false` |
| ~~Axis labels cut off~~ | ✅ Fixed | Added proper axis sizing config |
| Coherence display | ✅ Working | Visual progress bar + percentage |

### 3. Layout Problems - Partially Addressed

| Issue | Current State | Expected |
|-------|---------------|----------|
| Flex height propagation | Improved | Full height utilization |
| Panel proportions | Working | Spectrum 55%, Transfer 45% |
| Wasted space | Better | Maximized visualization area |
| Responsive behavior | Needs work | Graceful degradation |

---

## Technical Fixes Required

### Fix 1: Spectrum Analyzer Y-Axis (spectrum-analyzer.tsx)

**Problem**: uPlot is auto-scaling despite `range: [-120, 0]` being set.

**Solution**: Add `auto: false` to force manual range.

```typescript
scales: {
  x: {
    time: false,
    distr: 2,
    log: 10,
    min: 20,
    max: 20000,
  },
  y: {
    auto: false,        // ADD THIS
    range: [-120, 0],   // Now enforced
  },
},
```

### Fix 2: Explicit Frequency Tick Labels

**Problem**: Log scale doesn't show standard audio frequency labels.

**Solution**: Add explicit `splits` array for X-axis.

```typescript
axes: [
  {
    label: "Frequency (Hz)",
    stroke: "var(--color-muted-foreground)",
    grid: { stroke: "var(--color-border)", width: 1 },
    ticks: { stroke: "var(--color-border)", width: 1 },
    splits: [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000],
    values: (_, ticks) =>
      ticks.map((v) => {
        if (v >= 1000) return `${(v / 1000).toFixed(v >= 10000 ? 0 : 1)}k`;
        return String(Math.round(v));
      }),
  },
  // Y-axis config...
],
```

### Fix 3: Flex Container Height Propagation (app.tsx)

**Problem**: `flex-1` children don't receive proper height without `min-h-0` on parent.

**Solution**: Add `min-h-0` to flex containers and explicit heights to panels.

```typescript
{/* Main visualization area */}
<div className="flex flex-1 flex-col gap-4 min-h-0">
  {/* Spectrum analyzer - takes 55% */}
  <div className="glass rounded-lg p-4 flex-[1.2] min-h-[350px]">
    <SpectrumAnalyzer ... />
  </div>

  {/* Transfer function - takes 45% */}
  <div className="glass rounded-lg p-4 flex-1 min-h-[280px]">
    <TransferFunctionDisplay ... />
  </div>
</div>
```

### Fix 4: Transfer Function Scale Fixes (transfer-function-display.tsx)

**Problem**: Magnitude and phase scales may auto-scale.

**Solution**: Add `auto: false` to both scales.

```typescript
scales: {
  x: {
    time: false,
    distr: 2,
    log: 10,
    min: 20,
    max: 20000,
  },
  magnitude: {
    auto: false,        // ADD THIS
    range: [-40, 20],
  },
  phase: {
    auto: false,        // ADD THIS
    range: [-180, 180],
  },
},
```

### Fix 5: Chart Container Height Calculation

**Problem**: Height fallback `|| 300` doesn't account for header/padding.

**Solution**: Calculate available height properly.

```typescript
// In ResizeObserver callback
const height = Math.max(entry.contentRect.height, 250);
```

### Fix 6: CSS Minimum Heights (index.css)

**Problem**: No enforced minimum heights for visualization panels.

**Solution**: Add utility classes for visualization containers.

```css
/* Visualization panel constraints */
.viz-panel {
  min-height: 280px;
}

.viz-panel-primary {
  min-height: 350px;
}
```

---

## Recommended Layout Structure

### Professional Audio Analyzer Layout Pattern

```
┌─────────────────────────────────────────────────────────────┐
│ Phonetic                            [Meters] [Settings] [◐] │
├─────────────────────────────────────────────────────────────┤
│ ┌─────────┐ ┌─────────────────────────────────────────────┐ │
│ │ DEVICES │ │ SPECTRUM ANALYZER            [-120 to 0 dB] │ │
│ │         │ │                                             │ │
│ │ In: [▼] │ │  0 ─┬─────────────────────────────────────  │ │
│ │ Out:[▼] │ │     │                                       │ │
│ │         │ │-30 ─┼─────────────────────────────────────  │ │
│ │ [Start] │ │     │              ╱╲                       │ │
│ ├─────────┤ │-60 ─┼────────────╱──╲─────────────────────  │ │
│ │ SIGNAL  │ │     │          ╱     ╲                      │ │
│ │         │ │-90 ─┼────────╱────────╲───────────────────  │ │
│ │ ○ Sine  │ │     │     ╱            ╲                    │ │
│ │ ○ Pink  │ │-120─┴───╱───────────────╲─────────────────  │ │
│ │ ○ White │ │        20  100  500  1k  5k  10k  20k       │ │
│ │ ○ Sweep │ ├─────────────────────────────────────────────┤ │
│ │         │ │ TRANSFER FUNCTION       Coh: 94% ▓▓▓▓▓▓░░  │ │
│ │ Freq:1k │ │                                             │ │
│ │ ═══○═══ │ │+20 ─┬───────────────────────────── +180° ─  │ │
│ │         │ │  0 ─┼─══════════════════════════════ 0° ──  │ │
│ │ Amp:50% │ │-20 ─┼─── ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ── -180° ─  │ │
│ │ ═══○═══ │ │-40 ─┴─────────────────────────────────────  │ │
│ │         │ │        20  100  500  1k  5k  10k  20k       │ │
│ │ [Start] │ └─────────────────────────────────────────────┘ │
│ └─────────┘                                                 │
├─────────────────────────────────────────────────────────────┤
│ 48kHz │ 2048 FFT │ 23.4Hz/bin │ Running │ Avg: 16 frames   │
└─────────────────────────────────────────────────────────────┘
```

### Design Principles

1. **Fixed Y-axis scales** - NEVER auto-scale during operation
   - Spectrum: -120 to 0 dB (or -90 to +10 dB for live sound)
   - Transfer magnitude: -40 to +20 dB
   - Phase: -180° to +180°

2. **Standard frequency labels** (logarithmic scale):
   ```
   20  50  100  200  500  1k  2k  5k  10k  20k
   ```

3. **Minimum display heights**:
   - Spectrum analyzer: 350px minimum
   - Transfer function: 280px minimum

4. **Grid lines**:
   - Y-axis: every 10 dB (or 6 dB for finer resolution)
   - X-axis: at decade intervals

5. **Color scheme**:
   - Dark theme default (easier on eyes in live sound environments)
   - High-contrast trace colors
   - Subtle but visible grid lines

---

## Files to Modify

| File | Changes |
|------|---------|
| `apps/desktop/src/components/spectrum-analyzer.tsx` | Y-axis `auto: false`, explicit frequency splits, height calculation |
| `apps/desktop/src/components/transfer-function-display.tsx` | Scale `auto: false`, frequency splits, height fixes |
| `apps/desktop/src/app.tsx` | Flex container `min-h-0`, panel height proportions |
| `apps/desktop/src/index.css` | Visualization panel utility classes |

---

## Future Improvements (Phase 2)

### Missing Professional Features

1. **Delay Finder** - Critical for live sound alignment
2. **Impulse Response** - Time-domain analysis
3. **SPL Calibration** - Microphone sensitivity compensation
4. **Measurement Export** - CSV, SMAART format
5. **Reference Curves** - ISO, X-curve overlays
6. **Multi-trace Comparison** - A/B measurements
7. **Peak Hold** - For spectrum analyzer
8. **Spectrogram/Waterfall** - Time vs frequency display

### Performance Considerations

1. Replace CPAL with CoreAudio for macOS (lower latency)
2. Consider WebGL-based rendering for spectrograms
3. Real-time thread priority for audio processing
4. Evaluate Solid.js or Rust-native UI for smoother updates

---

## Stack Assessment

### Good Choices ✅

| Component | Choice | Why It's Good |
|-----------|--------|---------------|
| **Backend** | Rust | Memory safety, performance, no GC pauses during audio |
| **Framework** | Tauri 2 | Native binaries, Apple Silicon support, smaller than Electron |
| **State** | Zustand | Minimal boilerplate, performant for real-time updates |
| **Monorepo** | Turborepo | Appropriate for this scale |
| **FFT** | realfft | Reasonable Rust FFT library |

### Questionable Choices ⚠️

#### 1. CPAL for Professional Audio

CPAL is a cross-platform abstraction. For a professional macOS audio tool competing with SysTune:

```
CPAL → Core Audio HAL (added latency, less control)
```

SysTune and similar tools use **direct platform APIs** (Core Audio on Mac, WASAPI/ASIO on Windows) for:
- Lower latency
- Better device enumeration
- Aggregate device support
- Exclusive mode access
- Sample-accurate timing

**Recommendation**: Consider [coreaudio-rs](https://github.com/RustAudio/coreaudio-rs) for macOS with CPAL as fallback for Linux.

#### 2. uPlot for Real-Time Audio Visualization

uPlot is good for static charts but professional audio analyzers need:
- 60fps smooth scrolling spectrograms
- WebGL/GPU-accelerated rendering
- Waterfall displays
- Phase correlation meters

**Better alternatives**:
- **WebGL-based**: [regl](https://github.com/regl-project/regl), custom WebGL shaders
- **Canvas 2D optimized**: Fine for now, but you'll hit limits with spectrograms
- **Consider**: egui (Rust native UI) for zero-latency rendering

#### 3. React for Real-Time DSP UI

React's reconciliation adds latency. For professional audio tools, consider:
- **Solid.js** - No virtual DOM, fine-grained reactivity
- **Rust-native UI** (egui/iced) - Direct GPU rendering, no JS bridge

*This isn't a blocker now*, but at scale (spectrograms, multiple traces), you'll feel it.

#### 4. Thread Priority / Real-Time Scheduling

No real-time thread priority is being set. Professional audio apps need:

```rust
// macOS: Set audio thread to real-time priority
use mach::thread_policy::{thread_policy_set, THREAD_TIME_CONSTRAINT_POLICY};
```

Without this, your audio thread competes with system tasks → glitches under load.

---

## Critical Missing Architecture

### 1. No ASIO/Exclusive Mode Support
Professional audio interfaces expect ASIO on Windows, exclusive mode on macOS. CPAL's default shared mode adds latency.

### 2. No Reference Microphone Calibration
SysTune imports `.cal` files for measurement mics (Earthworks, UMIK-1, etc.). Without this, measurements aren't accurate.

### 3. No Delay Compensation
The most critical SysTune feature: automatically finding propagation delay between output and input to align transfer function measurements.

```
Your generator → PA system → room → microphone → your input
                         ↑
                   ~10-50ms delay
```

Without delay compensation, your transfer function is phase garbage.

---

## Test Coverage Assessment

**Current: ~37 tests total**

For professional audio software, this is **insufficient**:

| Category | Current | Needed |
|----------|---------|--------|
| DSP unit tests | 23 | 100+ |
| Integration tests | ~14 | 50+ |
| Audio edge cases | 0 | 20+ (buffer underruns, sample rate changes) |
| Regression tests | 0 | Per-bug fixes |
| Performance benchmarks | 0 | Critical for real-time |

**Missing test scenarios**:
- Buffer underrun recovery
- Sample rate switching mid-stream
- Device hot-plug
- FFT accuracy at different sizes
- Phase unwrapping edge cases (near ±180°)
- Coherence with various SNR levels

---

## Overall Verdict

| Aspect | Status |
|--------|--------|
| Core DSP in Rust | ✅ Correct choice |
| Tauri for desktop | ✅ Good for MVP, may need native UI later |
| CPAL audio I/O | ⚠️ Replace with direct Core Audio for pro use |
| React/uPlot UI | ⚠️ Will hit performance limits with spectrograms |
| Test coverage | ❌ Needs 3-5x more coverage |
| Feature completeness | ❌ ~30% of SysTune feature set |

---

## Roadmap to Professional Viability

### Phase 1: Critical Fixes ✅ COMPLETE
- [x] Fix spectrum analyzer Y-axis auto-scaling
- [x] Fix flex container height propagation
- [x] Add explicit frequency tick labels
- [x] Fix transfer function display visibility
- [x] Update CSS with minimum heights
- [x] Fix generator command serialization (camelCase)

### Phase 2: Core Features ✅ COMPLETE
- [x] **Delay Finder** - Cross-correlation based, configurable threshold
- [x] **Impulse Response Capture** - Deconvolution with RT60 estimation
- [x] **SPL Calibration** - .cal file parsing, A/C-weighting filters
- [x] **Measurement Export** - CSV, JSON, SMAART-compatible TXT
- [x] **Real-time Thread Priority** - macOS Mach thread policy

### Phase 3: Architecture Improvements ✅ MOSTLY COMPLETE
- [x] CoreAudio backend implementation (coreaudio-rs) - device enumeration, input/output streams
- [x] Set real-time thread priority for audio threads
- [ ] Integrate CoreAudio streams into AudioEngine (optional toggle)
- [ ] Add exclusive mode audio device access
- [ ] Implement proper buffer management with underrun handling

### Phase 4: Professional Features (Medium-term)
- [x] WebGL-based spectrum visualization with renderer toggle
- [ ] Reference curves (ISO, X-curve overlays)
- [ ] Multi-trace comparison (A/B measurements)
- [ ] Peak hold for spectrum analyzer
- [ ] Spectrogram/waterfall display
- [x] Calibration file import (.cal, .txt formats)

### Phase 5: Performance & Polish (Long-term)
- [ ] Solid.js or Rust-native UI (egui) migration evaluation
- [ ] Add 100+ DSP unit tests (currently at 64)
- [ ] Add performance benchmarks
- [ ] Add audio edge case tests (buffer underrun, device hot-plug)

---

## Implementation Priority Matrix

| Feature | Impact | Effort | Priority |
|---------|--------|--------|----------|
| Delay Finder | Critical | Medium | 🔴 P0 |
| Impulse Response | High | Medium | 🟠 P1 |
| CoreAudio Migration | High | High | 🟠 P1 |
| SPL Calibration | High | Low | 🟡 P2 |
| Real-time Thread Priority | Medium | Low | 🟡 P2 |
| Measurement Export | Medium | Low | 🟡 P2 |
| WebGL Visualization | Medium | High | 🟢 P3 |
| UI Framework Migration | Low | Very High | 🟢 P3 |

---

## References

- [AFMG SysTune Features](https://www.afmg.eu/en/afmg-systune#features)
- [SMAART by Rational Acoustics](https://www.rationalacoustics.com/smaart/)
- [Room EQ Wizard (REW)](https://www.roomeqwizard.com/)
- [coreaudio-rs](https://github.com/RustAudio/coreaudio-rs)
- [egui](https://github.com/emilk/egui)
