use phonetic_lib::audio::ChannelBuffers;

#[test]
fn interleaved_frames_are_split_by_channel() {
    // Two channels: [L0, R0, L1, R1, L2, R2]
    let interleaved = [1.0, -1.0, 2.0, -2.0, 3.0, -3.0];
    let mut buffers = ChannelBuffers::new(2, 16);
    buffers.fill(&interleaved);

    assert_eq!(buffers.channel(0), &[1.0, 2.0, 3.0]);
    assert_eq!(buffers.channel(1), &[-1.0, -2.0, -3.0]);
}

#[test]
fn eight_channels_split_correctly() {
    let interleaved: Vec<f32> = (0..80).map(|i| i as f32).collect();
    let mut buffers = ChannelBuffers::new(8, 64);
    buffers.fill(&interleaved);

    assert_eq!(buffers.channel_count(), 8);
    for c in 0..8 {
        let expected: Vec<f32> = (0..10).map(|f| (f * 8 + c) as f32).collect();
        assert_eq!(buffers.channel(c), expected.as_slice(), "channel {c}");
    }
}

#[test]
fn refilling_does_not_grow_the_buffers() {
    let mut buffers = ChannelBuffers::new(2, 1024);
    let before = buffers.capacity_per_channel();
    for _ in 0..100 {
        buffers.fill(&[1.0, 2.0, 3.0, 4.0]);
    }
    assert_eq!(
        buffers.capacity_per_channel(),
        before,
        "fill must not reallocate"
    );
    assert_eq!(
        buffers.channel(0).len(),
        2,
        "each fill replaces, never appends"
    );
}

#[test]
fn a_ragged_final_frame_is_ignored_rather_than_panicking() {
    let mut buffers = ChannelBuffers::new(2, 16);
    buffers.fill(&[1.0, -1.0, 2.0]); // one and a half frames
    assert_eq!(buffers.channel(0), &[1.0]);
    assert_eq!(buffers.channel(1), &[-1.0]);
}

#[test]
fn requesting_a_channel_out_of_range_returns_empty() {
    let buffers = ChannelBuffers::new(2, 16);
    assert!(buffers.channel(7).is_empty());
}

use phonetic_lib::dsp::{average_traces, TransferFunctionData};

fn trace(magnitude_db: f32, phase_deg: f32) -> TransferFunctionData {
    let frequencies: Vec<f32> = (0..50).map(|i| 20.0 * 1.2f32.powi(i)).collect();
    let n = frequencies.len();
    TransferFunctionData {
        magnitude_db: vec![magnitude_db; n],
        phase_deg: vec![phase_deg; n],
        coherence: vec![0.9; n],
        group_delay_ms: vec![0.0; n],
        frequencies,
    }
}

#[test]
fn averaging_one_trace_returns_it_unchanged() {
    let input = trace(-3.0, 45.0);
    let out = average_traces(std::slice::from_ref(&input)).unwrap();
    for (a, b) in input.magnitude_db.iter().zip(&out.magnitude_db) {
        assert!((a - b).abs() < 1e-3);
    }
}

#[test]
fn averaging_identical_traces_changes_nothing() {
    let out = average_traces(&[trace(-3.0, 45.0), trace(-3.0, 45.0), trace(-3.0, 45.0)]).unwrap();
    for m in &out.magnitude_db {
        assert!((m - -3.0).abs() < 1e-3, "got {m}");
    }
}

/// The reason to average complex values rather than dB: two positions in
/// opposite polarity really do cancel, and the operator needs to see that.
#[test]
fn opposite_polarity_positions_cancel() {
    let out = average_traces(&[trace(0.0, 0.0), trace(0.0, 180.0)]).unwrap();
    for m in &out.magnitude_db {
        assert!(*m < -40.0, "opposite polarity should null, got {m} dB");
    }
}

#[test]
fn averaging_carries_the_lowest_coherence_forward() {
    let mut low = trace(0.0, 0.0);
    low.coherence = vec![0.2; low.coherence.len()];
    let out = average_traces(&[trace(0.0, 0.0), low]).unwrap();
    for c in &out.coherence {
        assert!(
            (c - 0.2).abs() < 1e-3,
            "worst-case coherence should survive, got {c}"
        );
    }
}

#[test]
fn averaging_nothing_is_an_error() {
    assert!(average_traces(&[]).is_err());
}

#[test]
fn averaging_traces_on_different_axes_is_an_error() {
    let mut other = trace(0.0, 0.0);
    other.frequencies.pop();
    other.magnitude_db.pop();
    other.phase_deg.pop();
    other.coherence.pop();
    other.group_delay_ms.pop();
    assert!(average_traces(&[trace(0.0, 0.0), other]).is_err());
}
