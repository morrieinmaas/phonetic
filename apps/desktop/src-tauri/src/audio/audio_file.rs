//! WAV reading and writing for offline analysis.
//!
//! Returns deinterleaved channels so the dual-channel DSP path can take a
//! reference and a measurement channel directly.

/// Deinterleaved audio, one `Vec<f32>` per channel.
pub struct AudioFile {
    pub channels: Vec<Vec<f32>>,
    pub sample_rate: f32,
}

impl AudioFile {
    /// Frames per channel. Zero if there are no channels.
    pub fn frames(&self) -> usize {
        self.channels.first().map_or(0, |c| c.len())
    }
}

/// Read a WAV file, converting any supported sample format to `f32` in [-1, 1].
pub fn read_wav(path: &str) -> Result<AudioFile, String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| format!("open {path}: {e}"))?;
    let spec = reader.spec();
    let channel_count = spec.channels as usize;

    if channel_count == 0 {
        return Err(format!("{path} declares zero channels"));
    }

    // Integer formats scale by their full-scale value; float is already normalised.
    let interleaved: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Float, _) => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|e| format!("read {path}: {e}"))?,
        (hound::SampleFormat::Int, bits) => {
            let scale = 1.0 / (1i64 << (bits - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 * scale))
                .collect::<Result<_, _>>()
                .map_err(|e| format!("read {path}: {e}"))?
        }
    };

    let frames = interleaved.len() / channel_count;
    let channels = (0..channel_count)
        .map(|c| {
            (0..frames)
                .map(|f| interleaved[f * channel_count + c])
                .collect()
        })
        .collect();

    Ok(AudioFile {
        channels,
        sample_rate: spec.sample_rate as f32,
    })
}

/// Write deinterleaved channels as a 32-bit float WAV.
pub fn write_wav(path: &str, channels: &[Vec<f32>], sample_rate: f32) -> Result<(), String> {
    let Some(first) = channels.first() else {
        return Err("cannot write a file with no channels".into());
    };

    if channels.iter().any(|c| c.len() != first.len()) {
        return Err("all channels must have the same length".into());
    }

    let spec = hound::WavSpec {
        channels: channels.len() as u16,
        sample_rate: sample_rate as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer =
        hound::WavWriter::create(path, spec).map_err(|e| format!("create {path}: {e}"))?;

    for frame in 0..first.len() {
        for channel in channels {
            writer
                .write_sample(channel[frame])
                .map_err(|e| format!("write {path}: {e}"))?;
        }
    }

    writer
        .finalize()
        .map_err(|e| format!("finalize {path}: {e}"))
}

use crate::dsp::{TransferFunctionData, TransferFunctionProcessor};

/// Measure a transfer function from two channels of a WAV file.
pub fn measure_from_file(
    path: &str,
    reference_channel: usize,
    measurement_channel: usize,
) -> Result<TransferFunctionData, String> {
    let file = read_wav(path)?;

    let channel = |i: usize| -> Result<&Vec<f32>, String> {
        file.channels.get(i).ok_or_else(|| {
            format!(
                "{path} has {} channels, no channel {i}",
                file.channels.len()
            )
        })
    };

    let reference = channel(reference_channel)?;
    let measurement = channel(measurement_channel)?;

    let mut processor = TransferFunctionProcessor::new(4096, file.sample_rate);
    processor.process(measurement, reference);
    Ok(processor.get_data())
}
