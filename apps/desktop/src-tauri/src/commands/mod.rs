use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

use tauri::State;

use crate::audio::{
    enumerate_devices, AudioBackend, AudioDeviceList, AudioEngine, AudioState, Routing, SignalType,
};
use crate::dsp::{
    bin_frequencies, diagnose, dose, export_impulse_response_csv, export_impulse_response_json,
    export_impulse_response_txt, export_transfer_function_csv, export_transfer_function_json,
    export_transfer_function_txt, generate_standard_frequencies, octave_band_levels,
    predict_with_eq, rate, rate_rc, write_to_file, BiquadFilter, CurveFamily, DelayFinder,
    DelayResult, Diagnosis, DoseResult, ExportFormat, ImpulseResponseData,
    ImpulseResponseProcessor, LeqLogger, LeqMetrics, LimitSet, MeasurementMetadata,
    MicrophoneCalibration, NoiseLimit, RcResult, ReferenceCurve, ReferenceCurveType,
    RoomCriteriaResult, SmoothingFraction, SplCalculator, TransferFunctionData,
};
use parking_lot::Mutex;

/// Frequency range for audio generation (human hearing range)
const MIN_FREQUENCY: f64 = 20.0;
const MAX_FREQUENCY: f64 = 20000.0;

/// Amplitude range (0.0 to 1.0)
const MIN_AMPLITUDE: f64 = 0.0;
const MAX_AMPLITUDE: f64 = 1.0;

const FFT_SIZE: usize = 2048;
const DEFAULT_SAMPLE_RATE: f32 = 48000.0;

/// Tracks the background thread appending periodic LEQ snapshots to a CSV
/// log (see `start_leq_logging`). The loop wakes every 100ms rather than
/// sleeping the full `interval_seconds` so `stop_leq_logging` can join it
/// promptly instead of blocking the command for however long the configured
/// interval is -- which can be many minutes.
struct LeqLoggingSession {
    stop: Arc<AtomicBool>,
    handle: thread::JoinHandle<()>,
}

/// DSP state for delay finder, impulse response, and calibration
pub struct DspState {
    pub delay_finder: Mutex<DelayFinder>,
    pub impulse_response: Mutex<ImpulseResponseProcessor>,
    pub spl_calculator: Mutex<SplCalculator>,
    /// `None` when LEQ interval logging isn't running. File I/O never
    /// happens in the audio callback -- this thread lives entirely on the
    /// command side, reading the engine's `LeqMeter` through its shared
    /// handle instead.
    leq_logging: Mutex<Option<LeqLoggingSession>>,
}

impl DspState {
    pub fn new() -> Self {
        Self {
            delay_finder: Mutex::new(DelayFinder::new(FFT_SIZE * 2, DEFAULT_SAMPLE_RATE, 500.0)),
            impulse_response: Mutex::new(ImpulseResponseProcessor::new(
                FFT_SIZE * 4,
                DEFAULT_SAMPLE_RATE,
            )),
            spl_calculator: Mutex::new(SplCalculator::default()),
            leq_logging: Mutex::new(None),
        }
    }
}

impl Default for DspState {
    fn default() -> Self {
        Self::new()
    }
}

#[tauri::command]
pub async fn list_audio_devices() -> Result<AudioDeviceList, String> {
    Ok(enumerate_devices())
}

#[tauri::command]
pub async fn get_audio_state(engine: State<'_, AudioEngine>) -> Result<AudioState, String> {
    Ok(engine.get_state())
}

#[tauri::command]
pub async fn set_audio_backend(
    engine: State<'_, AudioEngine>,
    backend: AudioBackend,
) -> Result<(), String> {
    engine.set_backend(backend)
}

#[tauri::command]
pub async fn set_exclusive_mode(
    engine: State<'_, AudioEngine>,
    enabled: bool,
) -> Result<(), String> {
    engine.set_exclusive_mode(enabled);
    Ok(())
}

#[tauri::command]
pub async fn get_exclusive_mode(engine: State<'_, AudioEngine>) -> Result<bool, String> {
    Ok(engine.get_exclusive_mode())
}

#[tauri::command]
pub async fn start_audio_stream(
    engine: State<'_, AudioEngine>,
    input_device_id: Option<String>,
    output_device_id: Option<String>,
) -> Result<(), String> {
    // Validate device IDs if provided
    if let Some(ref id) = input_device_id {
        if id.is_empty() {
            return Err("Input device ID cannot be empty".to_string());
        }
    }
    if let Some(ref id) = output_device_id {
        if id.is_empty() {
            return Err("Output device ID cannot be empty".to_string());
        }
    }

    engine.start_stream(input_device_id.as_deref(), output_device_id.as_deref())
}

#[tauri::command]
pub async fn stop_audio_stream(engine: State<'_, AudioEngine>) -> Result<(), String> {
    engine.stop_stream();
    Ok(())
}

#[tauri::command]
pub async fn start_generator(
    engine: State<'_, AudioEngine>,
    signal_type: SignalType,
    frequency: f64,
    amplitude: f64,
) -> Result<(), String> {
    // Validate frequency
    if !frequency.is_finite() {
        return Err("Frequency must be a finite number".to_string());
    }
    if !(MIN_FREQUENCY..=MAX_FREQUENCY).contains(&frequency) {
        return Err(format!(
            "Frequency must be between {} and {} Hz",
            MIN_FREQUENCY, MAX_FREQUENCY
        ));
    }

    // Validate amplitude
    if !amplitude.is_finite() {
        return Err("Amplitude must be a finite number".to_string());
    }
    if !(MIN_AMPLITUDE..=MAX_AMPLITUDE).contains(&amplitude) {
        return Err(format!(
            "Amplitude must be between {} and {}",
            MIN_AMPLITUDE, MAX_AMPLITUDE
        ));
    }

    engine.start_generator(signal_type, frequency, amplitude);
    Ok(())
}

#[tauri::command]
pub async fn stop_generator(engine: State<'_, AudioEngine>) -> Result<(), String> {
    engine.stop_generator();
    Ok(())
}

#[tauri::command]
pub async fn get_spectrum_data(engine: State<'_, AudioEngine>) -> Result<Vec<f32>, String> {
    Ok(engine.get_spectrum_data())
}

#[tauri::command]
pub async fn get_transfer_function_data(
    engine: State<'_, AudioEngine>,
) -> Result<TransferFunctionData, String> {
    Ok(engine.get_transfer_function_data())
}

#[tauri::command]
pub async fn reset_averaging(engine: State<'_, AudioEngine>) -> Result<(), String> {
    engine.reset_averaging();
    Ok(())
}

#[tauri::command]
pub async fn set_fft_size(engine: State<'_, AudioEngine>, size: usize) -> Result<(), String> {
    // Validate FFT size
    if !size.is_power_of_two() {
        return Err("FFT size must be a power of 2".to_string());
    }
    if !(512..=16384).contains(&size) {
        return Err("FFT size must be between 512 and 16384".to_string());
    }

    engine.set_fft_size(size)
}

#[tauri::command]
pub async fn set_transfer_function_delay(
    engine: State<'_, AudioEngine>,
    delay_samples: usize,
) -> Result<(), String> {
    engine.set_transfer_function_delay(delay_samples);
    Ok(())
}

#[tauri::command]
pub async fn get_transfer_function_delay(engine: State<'_, AudioEngine>) -> Result<usize, String> {
    Ok(engine.transfer_function_delay())
}

#[tauri::command]
pub async fn set_max_averages(engine: State<'_, AudioEngine>, count: usize) -> Result<(), String> {
    engine.set_max_averages(count);
    Ok(())
}

#[tauri::command]
pub async fn set_resolution_mode(
    engine: State<'_, AudioEngine>,
    multi_window: bool,
) -> Result<(), String> {
    engine.set_resolution_mode(multi_window);
    Ok(())
}

#[tauri::command]
pub async fn is_multi_window(engine: State<'_, AudioEngine>) -> Result<bool, String> {
    Ok(engine.is_multi_window())
}

#[tauri::command]
pub async fn set_smoothing(
    engine: State<'_, AudioEngine>,
    fraction: SmoothingFraction,
) -> Result<(), String> {
    engine.set_smoothing(fraction);
    Ok(())
}

#[tauri::command]
pub async fn get_diagnostics(engine: State<'_, AudioEngine>) -> Result<Vec<Diagnosis>, String> {
    Ok(diagnose(&engine.rig_snapshot()))
}

/// Configure how many input channels the next stream should open, clamped to
/// 1-8. Real system tuning uses several mics at once.
#[tauri::command]
pub async fn set_input_channel_count(
    engine: State<'_, AudioEngine>,
    count: usize,
) -> Result<(), String> {
    engine.set_input_channel_count(count);
    Ok(())
}

/// Select which input channel feeds level metering and rig diagnostics.
#[tauri::command]
pub async fn set_measurement_channel(
    engine: State<'_, AudioEngine>,
    index: usize,
) -> Result<(), String> {
    engine.set_measurement_channel(index);
    Ok(())
}

/// Current channel routing (the set of measurement pairs and what each
/// measures against).
#[tauri::command]
pub async fn get_routing(engine: State<'_, AudioEngine>) -> Result<Routing, String> {
    Ok((*engine.routing()).clone())
}

/// Replace the channel routing. Rejected (with the previous routing left in
/// place) if it fails validation — see `Routing::validate`.
#[tauri::command]
pub async fn set_routing(engine: State<'_, AudioEngine>, routing: Routing) -> Result<(), String> {
    engine.set_routing(routing)
}

/// Maps a pair id to its slot (its index into `routing.pairs`), naming the
/// id in the error when it isn't found. Split out from the command below so
/// it can be unit tested without a Tauri `State`.
fn find_pair_slot(routing: &Routing, pair_id: &str) -> Result<usize, String> {
    routing
        .pairs
        .iter()
        .position(|pair| pair.id == pair_id)
        .ok_or_else(|| format!("no measurement pair with id '{pair_id}'"))
}

/// Transfer function data for one measurement pair, looked up by id.
///
/// Maps the id to its slot via `routing().pairs`, so the caller never needs
/// to know slot indices. Follows the commit d2ac837 pattern: the underlying
/// engine call takes a cheap snapshot under its lock, releases it, then does
/// the expensive smoothing and dB conversion outside the lock, so a UI poll
/// can never stall the real-time audio callback.
#[tauri::command]
pub async fn get_pair_transfer_function(
    engine: State<'_, AudioEngine>,
    pair_id: String,
) -> Result<TransferFunctionData, String> {
    let routing = engine.routing();
    let slot = find_pair_slot(&routing, &pair_id)?;
    Ok(engine.get_pair_transfer_function_by_slot(slot))
}

/// Average several measurement positions into one trace, in the complex
/// domain so cancellations show up instead of being averaged away.
#[tauri::command]
pub async fn average_traces(
    traces: Vec<TransferFunctionData>,
) -> Result<TransferFunctionData, String> {
    crate::dsp::average_traces(&traces)
}

// ============================================================================
// Delay Finder Commands
// ============================================================================

#[tauri::command]
pub async fn find_delay(
    dsp: State<'_, DspState>,
    engine: State<'_, AudioEngine>,
    reference: Vec<f32>,
    measured: Vec<f32>,
) -> Result<DelayResult, String> {
    let result = dsp.delay_finder.lock().find_delay(&reference, &measured);
    engine.push_measured_delay(result.delay_samples);
    Ok(result)
}

#[tauri::command]
pub async fn set_delay_finder_max_delay(
    dsp: State<'_, DspState>,
    max_delay_ms: f32,
) -> Result<(), String> {
    dsp.delay_finder.lock().set_max_delay_ms(max_delay_ms);
    Ok(())
}

#[tauri::command]
pub async fn set_delay_finder_threshold(
    dsp: State<'_, DspState>,
    threshold: f32,
) -> Result<(), String> {
    dsp.delay_finder.lock().set_correlation_threshold(threshold);
    Ok(())
}

// ============================================================================
// Impulse Response Commands
// ============================================================================

#[tauri::command]
pub async fn start_impulse_capture(dsp: State<'_, DspState>) -> Result<(), String> {
    dsp.impulse_response.lock().start_capture();
    Ok(())
}

#[tauri::command]
pub async fn stop_impulse_capture(dsp: State<'_, DspState>) -> Result<bool, String> {
    Ok(dsp.impulse_response.lock().stop_capture())
}

#[tauri::command]
pub async fn add_impulse_samples(
    dsp: State<'_, DspState>,
    reference: Vec<f32>,
    measured: Vec<f32>,
) -> Result<(), String> {
    dsp.impulse_response
        .lock()
        .add_samples(&reference, &measured);
    Ok(())
}

#[tauri::command]
pub async fn get_impulse_capture_progress(dsp: State<'_, DspState>) -> Result<f32, String> {
    Ok(dsp.impulse_response.lock().capture_progress())
}

#[tauri::command]
pub async fn is_impulse_capture_complete(dsp: State<'_, DspState>) -> Result<bool, String> {
    Ok(dsp.impulse_response.lock().is_capture_complete())
}

#[tauri::command]
pub async fn compute_impulse_response(
    dsp: State<'_, DspState>,
) -> Result<Option<ImpulseResponseData>, String> {
    Ok(dsp.impulse_response.lock().compute_impulse_response())
}

#[tauri::command]
pub async fn clear_impulse_capture(dsp: State<'_, DspState>) -> Result<(), String> {
    dsp.impulse_response.lock().clear();
    Ok(())
}

// ============================================================================
// SPL Calibration Commands
// ============================================================================

#[tauri::command]
pub async fn load_calibration_file(
    dsp: State<'_, DspState>,
    content: String,
    name: String,
) -> Result<(), String> {
    let calibration = MicrophoneCalibration::from_cal_file(&content, &name)?;
    dsp.spl_calculator.lock().set_calibration(calibration);
    Ok(())
}

#[tauri::command]
pub async fn set_calibration_sensitivity(
    dsp: State<'_, DspState>,
    sensitivity_db: f32,
) -> Result<(), String> {
    let mut calc = dsp.spl_calculator.lock();
    let mut cal = calc.calibration().clone();
    cal.sensitivity_db = sensitivity_db;
    calc.set_calibration(cal);
    Ok(())
}

#[tauri::command]
pub async fn set_preamp_gain(dsp: State<'_, DspState>, gain_db: f32) -> Result<(), String> {
    dsp.spl_calculator.lock().set_preamp_gain(gain_db);
    Ok(())
}

#[tauri::command]
pub async fn set_a_weighting(dsp: State<'_, DspState>, enabled: bool) -> Result<(), String> {
    dsp.spl_calculator.lock().set_a_weighting(enabled);
    Ok(())
}

#[tauri::command]
pub async fn dbfs_to_spl(dsp: State<'_, DspState>, dbfs: f32) -> Result<f32, String> {
    Ok(dsp.spl_calculator.lock().dbfs_to_spl(dbfs))
}

#[tauri::command]
pub async fn calibrate_with_reference(
    dsp: State<'_, DspState>,
    measured_dbfs: f32,
    reference_spl_db: f32,
) -> Result<(), String> {
    dsp.spl_calculator
        .lock()
        .calibrate_with_reference(measured_dbfs, reference_spl_db);
    Ok(())
}

#[tauri::command]
pub async fn get_calibration_info(
    dsp: State<'_, DspState>,
) -> Result<MicrophoneCalibration, String> {
    Ok(dsp.spl_calculator.lock().calibration().clone())
}

// ============================================================================
// Room Criteria Commands
// ============================================================================

/// Rate the current calibrated spectrum against an NC/NR curve family.
///
/// Testable without a `State`, as the other command bodies in this module.
fn get_room_criteria_inner(
    engine: &AudioEngine,
    dsp: &DspState,
    family: CurveFamily,
) -> Result<RoomCriteriaResult, String> {
    let spectrum_dbfs = engine.get_spectrum_data();
    // Derive the axis from the spectrum's own bin count, not from FFT_SIZE:
    // set_fft_size() resizes the spectrum processor, so a hardcoded size
    // mislabels the whole axis and puts every octave band in the wrong place.
    let frequencies = bin_frequencies(spectrum_dbfs.len(), engine.sample_rate() as f32);

    // Calibrate under the lock, then drop the guard before the octave-band
    // summing and rating: no lock held across expensive work.
    let spl_db = dsp
        .spl_calculator
        .lock()
        .calibrate_spectrum(&spectrum_dbfs, &frequencies);

    let bands = octave_band_levels(&spl_db, &frequencies);
    Ok(rate(&bands, family))
}

#[tauri::command]
pub async fn get_room_criteria(
    engine: State<'_, AudioEngine>,
    dsp: State<'_, DspState>,
    family: CurveFamily,
) -> Result<RoomCriteriaResult, String> {
    get_room_criteria_inner(&engine, &dsp, family)
}

/// Rate the current calibrated spectrum on RC Mark II.
///
/// Testable without a `State`, as `get_room_criteria_inner` above.
fn get_rc_rating_inner(engine: &AudioEngine, dsp: &DspState) -> Result<RcResult, String> {
    let spectrum_dbfs = engine.get_spectrum_data();
    // Same axis-derivation rule as get_room_criteria_inner: from the spectrum's
    // own bin count, not FFT_SIZE -- see that function's comment.
    let frequencies = bin_frequencies(spectrum_dbfs.len(), engine.sample_rate() as f32);

    // Calibrate under the lock, then drop the guard before rating: no lock
    // held across expensive work.
    let spl_db = dsp
        .spl_calculator
        .lock()
        .calibrate_spectrum(&spectrum_dbfs, &frequencies);

    Ok(rate_rc(&spl_db, &frequencies))
}

#[tauri::command]
pub async fn get_rc_rating(
    engine: State<'_, AudioEngine>,
    dsp: State<'_, DspState>,
) -> Result<RcResult, String> {
    get_rc_rating_inner(&engine, &dsp)
}

// ============================================================================
// Export Commands
// ============================================================================

#[tauri::command]
pub async fn export_transfer_function(
    engine: State<'_, AudioEngine>,
    format: ExportFormat,
    file_path: String,
    name: Option<String>,
    notes: Option<String>,
) -> Result<(), String> {
    let data = engine.get_transfer_function_data();
    let state = engine.get_state();

    let metadata = MeasurementMetadata {
        name: name.unwrap_or_else(|| "Transfer Function".to_string()),
        sample_rate: state.sample_rate as f32,
        fft_size: 2048, // TODO: get from engine
        notes,
        ..Default::default()
    };

    let content = match format {
        ExportFormat::Csv => export_transfer_function_csv(&data, &metadata),
        ExportFormat::Json => export_transfer_function_json(&data, &metadata)
            .map_err(|e| format!("JSON serialization error: {}", e))?,
        ExportFormat::Txt => export_transfer_function_txt(&data, &metadata),
    };

    write_to_file(&file_path, &content).map_err(|e| format!("Failed to write file: {}", e))
}

#[tauri::command]
pub async fn export_impulse_response(
    dsp: State<'_, DspState>,
    format: ExportFormat,
    file_path: String,
    name: Option<String>,
    notes: Option<String>,
) -> Result<(), String> {
    let data = dsp
        .impulse_response
        .lock()
        .compute_impulse_response()
        .ok_or("No impulse response data available")?;

    let metadata = MeasurementMetadata {
        name: name.unwrap_or_else(|| "Impulse Response".to_string()),
        sample_rate: data.sample_rate,
        notes,
        ..Default::default()
    };

    let content = match format {
        ExportFormat::Csv => export_impulse_response_csv(&data, &metadata),
        ExportFormat::Json => export_impulse_response_json(&data, &metadata)
            .map_err(|e| format!("JSON serialization error: {}", e))?,
        ExportFormat::Txt => export_impulse_response_txt(&data, &metadata),
    };

    write_to_file(&file_path, &content).map_err(|e| format!("Failed to write file: {}", e))
}

// ============================================================================
// Reference Curve Commands
// ============================================================================

/// Reference curve info for frontend display
#[derive(serde::Serialize)]
pub struct ReferenceCurveInfo {
    pub curve_type: ReferenceCurveType,
    pub name: String,
    pub description: String,
}

#[tauri::command]
pub async fn get_available_reference_curves() -> Result<Vec<ReferenceCurveInfo>, String> {
    Ok(ReferenceCurveType::all()
        .into_iter()
        .map(|ct| ReferenceCurveInfo {
            curve_type: ct,
            name: ct.display_name().to_string(),
            description: match ct {
                ReferenceCurveType::Flat => "Flat frequency response reference".to_string(),
                ReferenceCurveType::Iso226_40Phon => {
                    "Equal-loudness contour at 40 phon (quiet)".to_string()
                }
                ReferenceCurveType::Iso226_60Phon => {
                    "Equal-loudness contour at 60 phon (moderate)".to_string()
                }
                ReferenceCurveType::Iso226_80Phon => {
                    "Equal-loudness contour at 80 phon (loud)".to_string()
                }
                ReferenceCurveType::XCurve => "Cinema calibration curve (ISO 2969)".to_string(),
                ReferenceCurveType::HouseCurve => "Live sound target curve".to_string(),
                ReferenceCurveType::AWeighting => "A-weighting frequency response".to_string(),
                ReferenceCurveType::CWeighting => "C-weighting frequency response".to_string(),
            },
        })
        .collect())
}

#[tauri::command]
pub async fn get_reference_curve(
    curve_type: ReferenceCurveType,
    num_points: usize,
) -> Result<ReferenceCurve, String> {
    let frequencies = generate_standard_frequencies(num_points.clamp(10, 1024));
    Ok(ReferenceCurve::generate(curve_type, &frequencies))
}

#[tauri::command]
pub async fn get_reference_curve_at_frequencies(
    curve_type: ReferenceCurveType,
    frequencies: Vec<f64>,
) -> Result<ReferenceCurve, String> {
    Ok(ReferenceCurve::generate(curve_type, &frequencies))
}

#[tauri::command]
pub async fn measure_wav_file(
    path: String,
    reference_channel: usize,
    measurement_channel: usize,
) -> Result<TransferFunctionData, String> {
    crate::audio::measure_from_file(&path, reference_channel, measurement_channel)
}

#[tauri::command]
pub async fn predict_eq(
    trace: TransferFunctionData,
    filters: Vec<BiquadFilter>,
    sample_rate: f32,
) -> Result<TransferFunctionData, String> {
    Ok(predict_with_eq(&trace, &filters, sample_rate))
}

#[tauri::command]
pub async fn save_session(path: String, session: crate::session::Session) -> Result<(), String> {
    crate::session::save_session_to_path(&path, &session)
}

#[tauri::command]
pub async fn load_session(path: String) -> Result<crate::session::Session, String> {
    crate::session::load_session_from_path(&path)
}

// ============================================================================
// LEQ Commands
// ============================================================================

/// Current LAeq/LCeq/LAFmax/LApeak/LCpeak/elapsed reading, integrated since
/// the stream last (re)started -- see `AudioEngine::leq_metrics`.
#[tauri::command]
pub async fn get_leq_metrics(
    engine: State<'_, AudioEngine>,
    dsp: State<'_, DspState>,
) -> Result<LeqMetrics, String> {
    // Converted to SPL here, not in the panel: these numbers are rendered
    // directly beside an absolute noise limit, and dBFS beside 85 dB SPL reads
    // as enormous headroom when the rig may be well over it.
    let spl = dsp.spl_calculator.lock();
    Ok(engine.leq_metrics().to_spl(&spl))
}

/// The shipped noise-exposure limit presets, each sourced and dated (see
/// `dsp::noise_limits`). Selecting one is a frontend concern: the operator's
/// chosen `NoiseLimit` is passed back whole to `compute_leq_dose` rather than
/// looked up again by id here.
#[tauri::command]
pub async fn list_noise_limits() -> Result<Vec<NoiseLimit>, String> {
    Ok(LimitSet::builtin().limits)
}

/// Exposure dose against `limit`, computed from the live LEQ meter through
/// the app's *live* SPL calibration (`dsp.spl_calculator`) -- never
/// `SplCalculator::default()`'s placeholder +134 dB offset. `LeqMetrics` are
/// dBFS and `limit` is absolute SPL; passing a default instead of the
/// operator's actual calibration here would silently produce a confident,
/// wrong dose number -- the exact defect commit aef651f fixed.
#[tauri::command]
pub async fn compute_leq_dose(
    dsp: State<'_, DspState>,
    engine: State<'_, AudioEngine>,
    limit: NoiseLimit,
) -> Result<DoseResult, String> {
    let metrics = engine.leq_metrics();
    let spl = dsp.spl_calculator.lock();
    Ok(dose(&metrics, &limit, &spl))
}

/// Starts appending a snapshot of the running LEQ measurement to `path`
/// every `interval_seconds`, on a background thread -- file I/O has no place
/// in the audio callback. Each snapshot is the meter's *cumulative* reading
/// at that instant, not a re-integrated per-interval average: the same
/// meter backs `compute_leq_dose`, whose math needs `elapsed_seconds` to
/// keep growing across the whole reference period, so nothing here may reset
/// it. `interval_seconds` is written on every row precisely so a reader can
/// tell a 1-second cadence from a 15-minute one apart -- see `dsp::leq_log`.
#[tauri::command]
pub async fn start_leq_logging(
    dsp: State<'_, DspState>,
    engine: State<'_, AudioEngine>,
    path: String,
    interval_seconds: f64,
) -> Result<(), String> {
    if !interval_seconds.is_finite() || interval_seconds <= 0.0 {
        return Err("interval_seconds must be a positive, finite number".to_string());
    }

    let mut session = dsp.leq_logging.lock();
    if session.is_some() {
        return Err("LEQ logging is already running; stop it first".to_string());
    }

    let mut logger = LeqLogger::new(PathBuf::from(path), interval_seconds)?;
    let leq_meter = engine.leq_meter_handle();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_thread = Arc::clone(&stop);
    let target = Duration::from_secs_f64(interval_seconds);
    let tick = Duration::from_millis(100);

    let handle = thread::spawn(move || {
        let mut waited = Duration::ZERO;
        while !stop_for_thread.load(Ordering::Relaxed) {
            thread::sleep(tick);
            waited += tick;
            if waited < target {
                continue;
            }
            waited = Duration::ZERO;
            let metrics = leq_meter.lock().metrics();
            if let Err(err) = logger.record(&metrics, SystemTime::now()) {
                log::error!("LEQ log write failed: {err}");
            }
        }
    });

    *session = Some(LeqLoggingSession { stop, handle });
    Ok(())
}

/// Stops the background logger started by `start_leq_logging`. A no-op if
/// logging isn't running.
#[tauri::command]
pub async fn stop_leq_logging(dsp: State<'_, DspState>) -> Result<(), String> {
    let session = dsp.leq_logging.lock().take();
    if let Some(session) = session {
        session.stop.store(true, Ordering::Relaxed);
        let _ = session.handle.join();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::MAX_INPUT_CHANNELS;

    #[test]
    fn test_frequency_validation() {
        // Valid frequencies should not error
        assert!((MIN_FREQUENCY..=MAX_FREQUENCY).contains(&20.0));
        assert!((MIN_FREQUENCY..=MAX_FREQUENCY).contains(&1000.0));
        assert!((MIN_FREQUENCY..=MAX_FREQUENCY).contains(&20000.0));

        // Invalid frequencies
        assert!(!(MIN_FREQUENCY..=MAX_FREQUENCY).contains(&19.9));
        assert!(!(MIN_FREQUENCY..=MAX_FREQUENCY).contains(&20001.0));
    }

    #[test]
    fn test_amplitude_validation() {
        // Valid amplitudes
        assert!((MIN_AMPLITUDE..=MAX_AMPLITUDE).contains(&0.0));
        assert!((MIN_AMPLITUDE..=MAX_AMPLITUDE).contains(&0.5));
        assert!((MIN_AMPLITUDE..=MAX_AMPLITUDE).contains(&1.0));

        // Invalid amplitudes
        assert!(!(MIN_AMPLITUDE..=MAX_AMPLITUDE).contains(&-0.1));
        assert!(!(MIN_AMPLITUDE..=MAX_AMPLITUDE).contains(&1.1));
    }

    #[test]
    fn test_fft_size_validation() {
        // Valid FFT sizes (powers of 2 in range)
        assert!(512usize.is_power_of_two() && (512..=16384).contains(&512));
        assert!(1024usize.is_power_of_two() && (512..=16384).contains(&1024));
        assert!(2048usize.is_power_of_two() && (512..=16384).contains(&2048));
        assert!(4096usize.is_power_of_two() && (512..=16384).contains(&4096));
        assert!(8192usize.is_power_of_two() && (512..=16384).contains(&8192));
        assert!(16384usize.is_power_of_two() && (512..=16384).contains(&16384));

        // Invalid: not power of 2
        assert!(!1000usize.is_power_of_two());

        // Invalid: out of range
        assert!(!(512..=16384).contains(&256));
        assert!(!(512..=16384).contains(&32768));
    }

    #[test]
    fn delay_and_averaging_round_trip_through_the_engine() {
        let engine = AudioEngine::new();
        engine.set_transfer_function_delay(480);
        assert_eq!(engine.transfer_function_delay(), 480);

        engine.set_max_averages(4);
        // Averaging depth is observable only via behaviour; assert the setter is
        // reachable and does not panic under the engine's lock.
        engine.set_max_averages(64);
    }

    /// Snapshotting a stopped engine must not panic on its empty buffers --
    /// that was this test's original point -- and must not produce diagnoses.
    ///
    /// It previously asserted the opposite: that an idle engine reports
    /// `no_measurement_signal`. That was the behaviour, and it was wrong. On
    /// every launch the app greeted the operator with "No signal on the
    /// measurement channel. Check phantom power, the cable, and that the right
    /// input is selected" -- troubleshooting advice for a fault that did not
    /// exist, before they had started anything.
    #[test]
    fn a_stopped_engine_snapshots_without_panicking_and_reports_nothing() {
        let engine = AudioEngine::new();
        let snapshot = engine.rig_snapshot();
        assert!(!snapshot.is_running, "a fresh engine is not running");

        let found = crate::dsp::diagnose(&snapshot);
        assert!(
            found.is_empty(),
            "an idle rig has nothing to diagnose, got: {:?}",
            found.iter().map(|d| &d.code).collect::<Vec<_>>()
        );
    }

    #[test]
    fn save_and_load_session_commands_round_trip_through_a_file() {
        use crate::session::{Session, SessionSettings, SESSION_VERSION};

        let path = std::env::temp_dir()
            .join("phonetic_command_session.json")
            .to_string_lossy()
            .into_owned();

        let session = Session {
            version: SESSION_VERSION,
            created_at: "2026-09-07T12:00:00Z".into(),
            name: "Command roundtrip".into(),
            settings: SessionSettings {
                sample_rate: 48_000.0,
                fft_size: 4096,
                multi_window: false,
                smoothing: "none".into(),
                delay_samples: 0,
                max_averages: 8,
            },
            traces: vec![],
        };

        tauri::async_runtime::block_on(save_session(path.clone(), session))
            .expect("save_session should succeed");
        let loaded = tauri::async_runtime::block_on(load_session(path.clone()))
            .expect("load_session should succeed");
        assert_eq!(loaded.name, "Command roundtrip");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn channel_count_is_clamped_to_the_supported_range() {
        let engine = AudioEngine::new();
        engine.set_input_channel_count(0);
        assert_eq!(
            engine.input_channel_count(),
            1,
            "must keep at least one channel"
        );
        engine.set_input_channel_count(99);
        assert_eq!(
            engine.input_channel_count(),
            MAX_INPUT_CHANNELS,
            "thirty-two is the ceiling"
        );
        engine.set_input_channel_count(4);
        assert_eq!(engine.input_channel_count(), 4);
    }

    #[test]
    fn average_traces_command_delegates_to_the_spatial_averaging_module() {
        let trace = TransferFunctionData {
            magnitude_db: vec![0.0, -3.0],
            phase_deg: vec![0.0, 0.0],
            coherence: vec![1.0, 1.0],
            group_delay_ms: vec![0.0, 0.0],
            frequencies: vec![100.0, 200.0],
        };

        let averaged =
            tauri::async_runtime::block_on(average_traces(vec![trace.clone(), trace.clone()]))
                .expect("averaging identical traces should succeed");
        assert_eq!(averaged.magnitude_db, trace.magnitude_db);

        let err = tauri::async_runtime::block_on(average_traces(vec![]))
            .expect_err("averaging nothing must fail");
        assert_eq!(err, "nothing to average");
    }

    fn test_pair(id: &str, measurement_channel: usize) -> crate::audio::MeasurementPair {
        crate::audio::MeasurementPair {
            id: id.into(),
            name: id.into(),
            measurement_channel,
            reference: crate::audio::ReferenceSource::Generator,
            delay_samples: 0,
            calibration_offset_db: 0.0,
            enabled: true,
        }
    }

    #[test]
    fn find_pair_slot_maps_an_id_to_its_index_in_routing_pairs() {
        let routing = Routing {
            pairs: vec![test_pair("a", 0), test_pair("b", 1)],
        };
        assert_eq!(find_pair_slot(&routing, "a"), Ok(0));
        assert_eq!(find_pair_slot(&routing, "b"), Ok(1));
    }

    #[test]
    fn find_pair_slot_names_the_missing_id_in_its_error() {
        let routing = Routing {
            pairs: vec![test_pair("a", 0)],
        };
        let error = find_pair_slot(&routing, "does-not-exist").unwrap_err();
        assert!(
            error.contains("does-not-exist"),
            "the error should name the id that wasn't found: {error}"
        );
    }

    /// `list_noise_limits` takes no `State`, so unlike the LEQ commands that
    /// read `AudioEngine`/`DspState` (untestable here without a running
    /// Tauri app -- no existing command test in this file does that either),
    /// this one can be exercised directly. The dose math itself and every
    /// builtin preset's source/date are already covered exhaustively in
    /// `dsp::noise_limits`'s own tests; this just proves the command wires
    /// through to `LimitSet::builtin()` rather than something else.
    #[test]
    fn list_noise_limits_command_returns_the_builtin_presets() {
        let limits = tauri::async_runtime::block_on(list_noise_limits()).unwrap();
        assert_eq!(limits.len(), LimitSet::builtin().limits.len());
        assert!(
            limits.iter().all(|limit| !limit.source.trim().is_empty()),
            "every returned limit must be sourced"
        );
    }

    #[test]
    fn room_criteria_command_returns_a_rating() {
        let engine = AudioEngine::new();
        let dsp = DspState::new();
        let result = get_room_criteria_inner(&engine, &dsp, CurveFamily::Nc);
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn rc_rating_command_returns_a_rating() {
        let engine = AudioEngine::new();
        let dsp = DspState::new();
        let result = get_rc_rating_inner(&engine, &dsp);
        assert!(result.is_ok(), "{result:?}");
    }
}

// ============================================================================
// Platform (for picking the right update download)
// ============================================================================

/// Which installer fits this machine, so the update notice can offer the exact
/// file instead of a list of twelve.
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct PlatformInfo {
    /// "macos" or "linux".
    pub os: &'static str,
    /// "aarch64" or "x86_64".
    pub arch: &'static str,
    /// How Phonetic was installed on Linux: "appimage", "deb" or "rpm". None on macOS.
    pub linux_package: Option<&'static str>,
}

/// A running AppImage sets `$APPIMAGE`; otherwise the distribution family
/// decides between the .deb and the .rpm.
fn linux_package(appimage: bool, debian: bool, redhat: bool) -> &'static str {
    match (appimage, debian, redhat) {
        (true, _, _) => "appimage",
        (false, true, _) => "deb",
        (false, false, true) => "rpm",
        _ => "appimage",
    }
}

#[tauri::command]
pub fn get_platform() -> PlatformInfo {
    let os = std::env::consts::OS;
    let linux = (os == "linux").then(|| {
        linux_package(
            std::env::var_os("APPIMAGE").is_some(),
            std::path::Path::new("/etc/debian_version").exists(),
            std::path::Path::new("/etc/redhat-release").exists()
                || std::path::Path::new("/etc/fedora-release").exists(),
        )
    });
    PlatformInfo {
        os,
        arch: std::env::consts::ARCH,
        linux_package: linux,
    }
}

#[cfg(test)]
mod platform_tests {
    use super::linux_package;

    #[test]
    fn an_appimage_updates_as_an_appimage_even_on_debian() {
        assert_eq!(linux_package(true, true, false), "appimage");
    }

    #[test]
    fn package_installs_follow_the_distribution_family() {
        assert_eq!(linux_package(false, true, false), "deb");
        assert_eq!(linux_package(false, false, true), "rpm");
        assert_eq!(linux_package(false, false, false), "appimage");
    }
}
