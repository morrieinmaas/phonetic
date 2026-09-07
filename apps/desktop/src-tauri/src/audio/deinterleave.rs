//! Splitting an interleaved CPAL buffer into per-channel slices.
//!
//! Buffers are allocated once and reused. Allocating inside an audio callback
//! risks a lock in the allocator and an audible dropout.

pub struct ChannelBuffers {
    channels: Vec<Vec<f32>>,
}

impl ChannelBuffers {
    /// Allocate `channel_count` buffers, each able to hold `capacity` samples.
    pub fn new(channel_count: usize, capacity: usize) -> Self {
        Self {
            channels: (0..channel_count)
                .map(|_| Vec::with_capacity(capacity))
                .collect(),
        }
    }

    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Capacity of one channel buffer. Used by tests to prove no reallocation.
    pub fn capacity_per_channel(&self) -> usize {
        self.channels.first().map_or(0, |c| c.capacity())
    }

    /// Replace the contents with a freshly deinterleaved buffer. `clear` keeps
    /// the allocation, so this does not touch the allocator in steady state.
    pub fn fill(&mut self, interleaved: &[f32]) {
        let count = self.channels.len();
        if count == 0 {
            return;
        }

        for buffer in &mut self.channels {
            buffer.clear();
        }

        // A trailing partial frame is dropped: a half-frame has no channel.
        for frame in interleaved.chunks_exact(count) {
            for (channel, sample) in self.channels.iter_mut().zip(frame) {
                channel.push(*sample);
            }
        }
    }

    /// Samples for one channel, or an empty slice if the index is out of range.
    pub fn channel(&self, index: usize) -> &[f32] {
        self.channels.get(index).map_or(&[], |c| c.as_slice())
    }
}
