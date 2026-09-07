use phonetic_lib::audio::write_wav;
use phonetic_lib::cli::{run_devices, run_measure, run_selftest, OutputFormat};

fn fixture(name: &str) -> String {
    let path = std::env::temp_dir()
        .join(name)
        .to_string_lossy()
        .into_owned();
    let reference: Vec<f32> = (0..48_000 * 4)
        .map(|i| {
            let mut s = (i as u32).wrapping_mul(2_654_435_761);
            s ^= s >> 16;
            (s as f32 / u32::MAX as f32) * 2.0 - 1.0
        })
        .collect();
    let measurement: Vec<f32> = reference.iter().map(|s| s * 0.5).collect();
    write_wav(&path, &[reference, measurement], 48_000.0).unwrap();
    path
}

#[test]
fn measure_emits_parseable_json_with_the_right_gain() {
    let path = fixture("phonetic_cli.wav");
    let out = run_measure(&path, 0, 1, OutputFormat::Json).expect("measure should succeed");

    let parsed: serde_json::Value = serde_json::from_str(&out).expect("output must be valid JSON");
    let mags = parsed["magnitude_db"]
        .as_array()
        .expect("magnitude_db array");
    let freqs = parsed["frequencies"].as_array().expect("frequencies array");
    assert_eq!(mags.len(), freqs.len());

    let mid: Vec<f64> = freqs
        .iter()
        .zip(mags)
        .filter(|(f, _)| {
            let f = f.as_f64().unwrap();
            (500.0..10_000.0).contains(&f)
        })
        .map(|(_, m)| m.as_f64().unwrap())
        .collect();
    let mean = mid.iter().sum::<f64>() / mid.len() as f64;
    assert!(
        (mean - -6.0206).abs() < 0.5,
        "expected -6.02 dB, got {mean:.3}"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn measure_emits_csv_with_a_header() {
    let path = fixture("phonetic_cli_csv.wav");
    let out = run_measure(&path, 0, 1, OutputFormat::Csv).unwrap();

    let mut lines = out.lines();
    assert_eq!(
        lines.next().unwrap(),
        "frequency_hz,magnitude_db,phase_deg,coherence,group_delay_ms"
    );
    assert!(
        lines.next().unwrap().split(',').count() == 5,
        "rows must have 5 fields"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn measure_reports_a_missing_file_cleanly() {
    let err = run_measure("/nope/missing.wav", 0, 1, OutputFormat::Json);
    assert!(err.is_err(), "missing file must be an error, not a panic");
}

#[test]
fn devices_lists_something_or_explains_why_not() {
    // CI runners have no audio hardware; either outcome is acceptable, a panic is not.
    let result = run_devices();
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn selftest_passes_on_a_healthy_build() {
    let out = run_selftest().expect("selftest should pass");
    assert!(
        out.contains("PASS"),
        "selftest output should report PASS, got:\n{out}"
    );
}

/// The binary must exist, run, and set a non-zero exit code on failure.
#[test]
fn binary_reports_failure_with_a_non_zero_exit_code() {
    let exe = env!("CARGO_BIN_EXE_phonetic-cli");
    let output = std::process::Command::new(exe)
        .args(["measure", "--file", "/nope/missing.wav"])
        .output()
        .expect("binary should run");

    assert!(
        !output.status.success(),
        "a missing file must exit non-zero"
    );
    assert!(
        !String::from_utf8_lossy(&output.stderr).is_empty(),
        "failures must explain themselves on stderr"
    );
}

#[test]
fn binary_selftest_exits_zero() {
    let exe = env!("CARGO_BIN_EXE_phonetic-cli");
    let output = std::process::Command::new(exe)
        .arg("selftest")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "selftest should exit 0 on a healthy build"
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("PASS"));
}
