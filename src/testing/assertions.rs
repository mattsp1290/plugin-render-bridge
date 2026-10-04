//! Audio comparison and assertion utilities for tests.

use crate::buffer::AudioBuffer;

/// Compare two audio buffers sample-by-sample within a tolerance.
///
/// Panics with a detailed message if any sample differs by more than `tolerance`.
pub fn assert_audio_eq(a: &AudioBuffer, b: &AudioBuffer, tolerance: f32) {
    assert_eq!(
        a.num_channels(),
        b.num_channels(),
        "channel count mismatch: {} vs {}",
        a.num_channels(),
        b.num_channels(),
    );
    assert_eq!(
        a.num_samples(),
        b.num_samples(),
        "sample count mismatch: {} vs {}",
        a.num_samples(),
        b.num_samples(),
    );
    assert_eq!(
        a.sample_rate(),
        b.sample_rate(),
        "sample rate mismatch: {} vs {}",
        a.sample_rate(),
        b.sample_rate(),
    );

    for ch in 0..a.num_channels() {
        for i in 0..a.num_samples() {
            let diff = (a.channel(ch)[i] - b.channel(ch)[i]).abs();
            assert!(
                diff <= tolerance,
                "audio mismatch at ch={ch} sample={i}: {} vs {} (diff={diff}, tolerance={tolerance})",
                a.channel(ch)[i],
                b.channel(ch)[i],
            );
        }
    }
}

/// Assert that an audio buffer's RMS level is within a range.
pub fn assert_rms_in_range(buf: &AudioBuffer, min: f64, max: f64) {
    let rms = buf.rms();
    assert!(
        rms >= min && rms <= max,
        "RMS {rms} outside expected range [{min}, {max}]",
    );
}

/// Assert that a buffer is completely silent (all samples below threshold).
pub fn assert_silent(buf: &AudioBuffer) {
    assert!(
        buf.is_silent(),
        "expected silence but buffer has audio content (RMS={})",
        buf.rms()
    );
}

/// Assert that a buffer is NOT silent.
pub fn assert_not_silent(buf: &AudioBuffer) {
    assert!(
        !buf.is_silent(),
        "expected audio content but buffer is silent"
    );
}

/// Assert that buffer `a` is louder than buffer `b` (by RMS).
pub fn assert_louder_than(a: &AudioBuffer, b: &AudioBuffer) {
    let rms_a = a.rms();
    let rms_b = b.rms();
    assert!(
        rms_a > rms_b,
        "expected first buffer to be louder: RMS {rms_a} vs {rms_b}",
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_buffers_pass() {
        let mut buf = AudioBuffer::new(2, 100, 44100);
        for i in 0..100 {
            let val = (i as f32 * 0.1).sin();
            buf.channel_mut(0)[i] = val;
            buf.channel_mut(1)[i] = val;
        }
        assert_audio_eq(&buf, &buf.clone(), 0.0);
    }

    #[test]
    #[should_panic(expected = "audio mismatch")]
    fn different_buffers_fail() {
        let mut a = AudioBuffer::new(1, 10, 44100);
        let mut b = AudioBuffer::new(1, 10, 44100);
        a.channel_mut(0)[5] = 1.0;
        b.channel_mut(0)[5] = 0.0;
        assert_audio_eq(&a, &b, 0.001);
    }

    #[test]
    fn rms_range_check() {
        let mut buf = AudioBuffer::new(1, 100, 44100);
        for i in 0..100 {
            buf.channel_mut(0)[i] = 0.5;
        }
        assert_rms_in_range(&buf, 0.4, 0.6);
    }

    #[test]
    fn silent_assertion() {
        let buf = AudioBuffer::new(2, 100, 44100);
        assert_silent(&buf);
    }
}
