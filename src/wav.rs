use std::path::Path;

use crate::config::BitDepth;
use hound::{SampleFormat, WavSpec, WavWriter};

use crate::buffer::AudioBuffer;

/// Error type for WAV operations.
#[derive(Debug, thiserror::Error)]
pub enum WavError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("hound error: {0}")]
    Hound(#[from] hound::Error),

    #[error("empty buffer")]
    EmptyBuffer,
}

/// Write an AudioBuffer to a WAV file.
pub fn write_wav(buffer: &AudioBuffer, path: &Path, bit_depth: BitDepth) -> Result<(), WavError> {
    if buffer.num_samples() == 0 {
        return Err(WavError::EmptyBuffer);
    }

    let spec = WavSpec {
        channels: buffer.num_channels() as u16,
        sample_rate: buffer.sample_rate(),
        bits_per_sample: match bit_depth {
            BitDepth::Int16 => 16,
            BitDepth::Int24 => 24,
            BitDepth::Float32 => 32,
        },
        sample_format: match bit_depth {
            BitDepth::Int16 | BitDepth::Int24 => SampleFormat::Int,
            BitDepth::Float32 => SampleFormat::Float,
        },
    };

    let mut writer = WavWriter::create(path, spec)?;

    let num_samples = buffer.num_samples();
    let num_channels = buffer.num_channels();

    // Write interleaved samples
    for frame in 0..num_samples {
        for ch in 0..num_channels {
            let sample = buffer.channel(ch)[frame];
            match bit_depth {
                BitDepth::Int16 => {
                    let val = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                    writer.write_sample(val)?;
                }
                BitDepth::Int24 => {
                    let val = (sample.clamp(-1.0, 1.0) * 8_388_607.0) as i32;
                    writer.write_sample(val)?;
                }
                BitDepth::Float32 => {
                    writer.write_sample(sample)?;
                }
            }
        }
    }

    writer.finalize()?;
    Ok(())
}

/// Read a WAV file into an AudioBuffer.
pub fn read_wav(path: &Path) -> Result<AudioBuffer, WavError> {
    let reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let num_channels = spec.channels as usize;
    let sample_rate = spec.sample_rate;

    let samples: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Float, _) => reader
            .into_samples::<f32>()
            .collect::<Result<Vec<_>, _>>()?,
        (SampleFormat::Int, 16) => reader
            .into_samples::<i16>()
            .map(|s| s.map(|v| v as f32 / i16::MAX as f32))
            .collect::<Result<Vec<_>, _>>()?,
        (SampleFormat::Int, 24) => reader
            .into_samples::<i32>()
            .map(|s| s.map(|v| v as f32 / 8_388_607.0))
            .collect::<Result<Vec<_>, _>>()?,
        (SampleFormat::Int, bits) => reader
            .into_samples::<i32>()
            .map(|s| s.map(|v| v as f32 / (1 << (bits - 1)) as f32))
            .collect::<Result<Vec<_>, _>>()?,
    };

    Ok(AudioBuffer::from_interleaved(
        &samples,
        num_channels,
        sample_rate,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn write_and_read_16bit() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.wav");

        let mut buf = AudioBuffer::new(2, 100, 44100);
        // Write a sine-like pattern
        for i in 0..100 {
            let val = (i as f32 / 100.0 * std::f32::consts::TAU).sin();
            buf.channel_mut(0)[i] = val;
            buf.channel_mut(1)[i] = val * 0.5;
        }

        write_wav(&buf, &path, BitDepth::Int16).unwrap();
        let read_buf = read_wav(&path).unwrap();

        assert_eq!(read_buf.num_channels(), 2);
        assert_eq!(read_buf.num_samples(), 100);
        assert_eq!(read_buf.sample_rate(), 44100);

        // 16-bit quantization loses some precision
        for i in 0..100 {
            assert!((read_buf.channel(0)[i] - buf.channel(0)[i]).abs() < 0.001);
        }
    }

    #[test]
    fn write_and_read_float32() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test_f32.wav");

        let mut buf = AudioBuffer::new(1, 50, 48000);
        for i in 0..50 {
            buf.channel_mut(0)[i] = i as f32 / 50.0;
        }

        write_wav(&buf, &path, BitDepth::Float32).unwrap();
        let read_buf = read_wav(&path).unwrap();

        assert_eq!(read_buf.num_samples(), 50);
        for i in 0..50 {
            assert!((read_buf.channel(0)[i] - buf.channel(0)[i]).abs() < 1e-6);
        }
    }

    #[test]
    fn write_empty_buffer_fails() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("empty.wav");
        let buf = AudioBuffer::new(1, 0, 44100);
        assert!(write_wav(&buf, &path, BitDepth::Int16).is_err());
    }
}
