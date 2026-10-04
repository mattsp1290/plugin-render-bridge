use std::ops::{Deref, DerefMut};

/// A multi-channel audio buffer with planar layout (one Vec per channel).
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    channels: Vec<Vec<f32>>,
    sample_rate: u32,
}

impl AudioBuffer {
    /// Create a zeroed buffer with the given channel count and sample count.
    pub fn new(num_channels: usize, num_samples: usize, sample_rate: u32) -> Self {
        Self {
            channels: vec![vec![0.0; num_samples]; num_channels],
            sample_rate,
        }
    }

    pub fn num_channels(&self) -> usize {
        self.channels.len()
    }

    pub fn num_samples(&self) -> usize {
        self.channels.first().map_or(0, |c| c.len())
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Duration in seconds.
    pub fn duration_secs(&self) -> f64 {
        if self.sample_rate == 0 {
            return 0.0;
        }
        self.num_samples() as f64 / self.sample_rate as f64
    }

    /// Get an immutable reference to a channel's samples.
    pub fn channel(&self, idx: usize) -> &[f32] {
        &self.channels[idx]
    }

    /// Get a mutable reference to a channel's samples.
    pub fn channel_mut(&mut self, idx: usize) -> &mut [f32] {
        &mut self.channels[idx]
    }

    /// Zero all channels.
    pub fn clear(&mut self) {
        for ch in &mut self.channels {
            ch.fill(0.0);
        }
    }

    /// Resize all channels to a new sample count (zero-fills if growing).
    pub fn resize(&mut self, num_samples: usize) {
        for ch in &mut self.channels {
            ch.resize(num_samples, 0.0);
        }
    }

    /// Returns true if all samples are approximately zero.
    pub fn is_silent(&self) -> bool {
        self.channels
            .iter()
            .all(|ch| ch.iter().all(|&s| s.abs() < 1e-7))
    }

    /// Convert from interleaved data (L0,R0,L1,R1,...).
    pub fn from_interleaved(data: &[f32], num_channels: usize, sample_rate: u32) -> Self {
        let num_samples = data.len() / num_channels;
        let mut channels = vec![vec![0.0; num_samples]; num_channels];
        for (i, &sample) in data.iter().enumerate() {
            let ch = i % num_channels;
            let frame = i / num_channels;
            if frame < num_samples {
                channels[ch][frame] = sample;
            }
        }
        Self {
            channels,
            sample_rate,
        }
    }

    /// Convert to interleaved data.
    pub fn to_interleaved(&self) -> Vec<f32> {
        let num_samples = self.num_samples();
        let num_channels = self.num_channels();
        let mut out = Vec::with_capacity(num_samples * num_channels);
        for frame in 0..num_samples {
            for ch in 0..num_channels {
                out.push(self.channels[ch][frame]);
            }
        }
        out
    }

    /// Slice a window from the buffer (copies data).
    pub fn slice(&self, start: usize, len: usize) -> Self {
        let channels = self
            .channels
            .iter()
            .map(|ch| {
                let end = (start + len).min(ch.len());
                let start = start.min(ch.len());
                ch[start..end].to_vec()
            })
            .collect();
        Self {
            channels,
            sample_rate: self.sample_rate,
        }
    }

    /// Remove the first `n` samples from each channel.
    /// Used for latency compensation — the plugin's internal delay
    /// produces silence at the start of the output that should be trimmed.
    pub fn trim_start(&mut self, n: usize) {
        for ch in &mut self.channels {
            if n >= ch.len() {
                ch.clear();
            } else {
                ch.drain(..n);
            }
        }
    }

    /// Get mutable references to all channel slices simultaneously.
    pub fn channels_mut(&mut self) -> Vec<&mut [f32]> {
        self.channels
            .iter_mut()
            .map(|ch| ch.as_mut_slice())
            .collect()
    }

    /// Append another buffer's samples (must have same channel count).
    pub fn append(&mut self, other: &AudioBuffer) {
        assert_eq!(self.num_channels(), other.num_channels());
        for (dst, src) in self.channels.iter_mut().zip(other.channels.iter()) {
            dst.extend_from_slice(src);
        }
    }

    /// Calculate RMS level across all channels.
    pub fn rms(&self) -> f64 {
        let total_samples: usize = self.channels.iter().map(|ch| ch.len()).sum();
        if total_samples == 0 {
            return 0.0;
        }
        let sum_sq: f64 = self
            .channels
            .iter()
            .flat_map(|ch| ch.iter())
            .map(|&s| (s as f64) * (s as f64))
            .sum();
        (sum_sq / total_samples as f64).sqrt()
    }
}

/// Wrapper for a buffer checked out from a pool.
pub struct PooledBuffer {
    inner: AudioBuffer,
    pool_id: usize,
}

impl PooledBuffer {
    pub(crate) fn new(inner: AudioBuffer, pool_id: usize) -> Self {
        Self { inner, pool_id }
    }

    pub fn pool_id(&self) -> usize {
        self.pool_id
    }

    /// Consume the wrapper and return the inner buffer (for returning to pool).
    pub fn into_inner(self) -> AudioBuffer {
        self.inner
    }
}

impl Deref for PooledBuffer {
    type Target = AudioBuffer;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for PooledBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interleaved_roundtrip() {
        let interleaved = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let buf = AudioBuffer::from_interleaved(&interleaved, 2, 44100);
        assert_eq!(buf.num_channels(), 2);
        assert_eq!(buf.num_samples(), 3);
        assert_eq!(buf.channel(0), &[1.0, 3.0, 5.0]);
        assert_eq!(buf.channel(1), &[2.0, 4.0, 6.0]);

        let back = buf.to_interleaved();
        assert_eq!(back, interleaved);
    }

    #[test]
    fn buffer_slice_and_append() {
        let mut buf = AudioBuffer::new(2, 100, 44100);
        buf.channel_mut(0)[50] = 1.0;

        let slice = buf.slice(40, 20);
        assert_eq!(slice.num_samples(), 20);
        assert_eq!(slice.channel(0)[10], 1.0);

        let mut a = AudioBuffer::new(2, 10, 44100);
        let b = AudioBuffer::new(2, 5, 44100);
        a.append(&b);
        assert_eq!(a.num_samples(), 15);
    }

    #[test]
    fn rms_calculation() {
        let mut buf = AudioBuffer::new(1, 4, 44100);
        buf.channel_mut(0).copy_from_slice(&[1.0, -1.0, 1.0, -1.0]);
        assert!((buf.rms() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn silence_detection() {
        let buf = AudioBuffer::new(2, 100, 44100);
        assert!(buf.is_silent());

        let mut buf2 = AudioBuffer::new(1, 10, 44100);
        buf2.channel_mut(0)[5] = 0.5;
        assert!(!buf2.is_silent());
    }
}
