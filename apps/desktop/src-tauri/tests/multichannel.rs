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
