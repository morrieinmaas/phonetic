use phonetic_lib::audio::{read_wav, write_wav};

fn tmp(name: &str) -> String {
    std::env::temp_dir()
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn stereo_roundtrip_preserves_samples_and_rate() {
    let path = tmp("phonetic_roundtrip.wav");
    let left: Vec<f32> = (0..1000).map(|i| (i as f32 / 1000.0) * 2.0 - 1.0).collect();
    let right: Vec<f32> = left.iter().map(|s| -s).collect();

    write_wav(&path, &[left.clone(), right.clone()], 48_000.0).expect("write should succeed");
    let read = read_wav(&path).expect("read should succeed");

    assert_eq!(read.sample_rate, 48_000.0);
    assert_eq!(read.channels.len(), 2);
    assert_eq!(read.frames(), 1000);

    for (i, (a, b)) in left.iter().zip(&read.channels[0]).enumerate() {
        assert!((a - b).abs() < 1e-6, "left sample {i}: wrote {a}, read {b}");
    }
    for (i, (a, b)) in right.iter().zip(&read.channels[1]).enumerate() {
        assert!(
            (a - b).abs() < 1e-6,
            "right sample {i}: wrote {a}, read {b}"
        );
    }

    std::fs::remove_file(&path).ok();
}

#[test]
fn mono_roundtrip_works() {
    let path = tmp("phonetic_mono.wav");
    let mono: Vec<f32> = (0..500).map(|i| (i as f32 * 0.01).sin()).collect();

    write_wav(&path, std::slice::from_ref(&mono), 44_100.0).unwrap();
    let read = read_wav(&path).unwrap();

    assert_eq!(read.channels.len(), 1);
    assert_eq!(read.sample_rate, 44_100.0);
    assert_eq!(read.frames(), 500);

    std::fs::remove_file(&path).ok();
}

#[test]
fn missing_file_reports_an_error_rather_than_panicking() {
    let err = read_wav("/nonexistent/definitely-not-here.wav");
    assert!(err.is_err(), "a missing file must return Err, not panic");
}

#[test]
fn channel_length_mismatch_is_rejected() {
    let path = tmp("phonetic_mismatch.wav");
    let err = write_wav(&path, &[vec![0.0; 10], vec![0.0; 5]], 48_000.0);
    assert!(err.is_err(), "ragged channels must be rejected");
}

use phonetic_lib::audio::measure_from_file;

/// End to end: synthesise a two-channel file whose second channel is the first
/// attenuated by exactly 6 dB, write it, measure it back.
#[test]
fn measuring_a_file_recovers_a_known_gain() {
    let path = tmp("phonetic_measure.wav");
    let reference: Vec<f32> = (0..48_000 * 4)
        .map(|i| {
            let mut s = (i as u32).wrapping_mul(2_654_435_761);
            s ^= s >> 16;
            (s as f32 / u32::MAX as f32) * 2.0 - 1.0
        })
        .collect();
    let measurement: Vec<f32> = reference.iter().map(|s| s * 0.5).collect();

    write_wav(&path, &[reference, measurement], 48_000.0).unwrap();

    let data = measure_from_file(&path, 0, 1).expect("measurement should succeed");

    let mid: Vec<f32> = data
        .frequencies
        .iter()
        .zip(&data.magnitude_db)
        .filter(|(f, _)| (500.0..10_000.0).contains(*f))
        .map(|(_, m)| *m)
        .collect();
    let mean = mid.iter().sum::<f32>() / mid.len() as f32;

    assert!(
        (mean - (-6.0206)).abs() < 0.5,
        "expected -6.02 dB from the file, got {mean:.3} dB"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn measuring_a_missing_channel_is_an_error() {
    let path = tmp("phonetic_onechannel.wav");
    write_wav(&path, &[vec![0.1; 1000]], 48_000.0).unwrap();
    assert!(
        measure_from_file(&path, 0, 1).is_err(),
        "channel 1 does not exist"
    );
    std::fs::remove_file(&path).ok();
}
