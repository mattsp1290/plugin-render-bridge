use std::sync::Mutex;

use crate::buffer::{AudioBuffer, PooledBuffer};

/// A thread-safe pool of pre-allocated audio buffers to avoid
/// allocation on the audio processing path.
pub struct BufferPool {
    buffers: Mutex<Vec<AudioBuffer>>,
    num_channels: usize,
    buffer_size: usize,
    sample_rate: u32,
    next_id: Mutex<usize>,
}

impl BufferPool {
    /// Create a new pool pre-allocated with `initial_count` buffers.
    pub fn new(
        num_channels: usize,
        buffer_size: usize,
        sample_rate: u32,
        initial_count: usize,
    ) -> Self {
        let buffers: Vec<AudioBuffer> = (0..initial_count)
            .map(|_| AudioBuffer::new(num_channels, buffer_size, sample_rate))
            .collect();
        Self {
            buffers: Mutex::new(buffers),
            num_channels,
            buffer_size,
            sample_rate,
            next_id: Mutex::new(0),
        }
    }

    /// Check out a buffer from the pool. If the pool is empty, allocates a new one.
    pub fn checkout(&self) -> PooledBuffer {
        let mut pool = self.buffers.lock().unwrap();
        let mut buf = pool.pop().unwrap_or_else(|| {
            AudioBuffer::new(self.num_channels, self.buffer_size, self.sample_rate)
        });
        buf.clear();

        let mut id = self.next_id.lock().unwrap();
        let pool_id = *id;
        *id += 1;

        PooledBuffer::new(buf, pool_id)
    }

    /// Return a buffer to the pool for reuse.
    pub fn return_buffer(&self, buf: PooledBuffer) {
        let inner = buf.into_inner();
        let mut pool = self.buffers.lock().unwrap();
        pool.push(inner);
    }

    /// Number of buffers currently available in the pool.
    pub fn available(&self) -> usize {
        self.buffers.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkout_and_return() {
        let pool = BufferPool::new(2, 512, 44100, 4);
        assert_eq!(pool.available(), 4);

        let buf1 = pool.checkout();
        assert_eq!(pool.available(), 3);
        assert_eq!(buf1.num_channels(), 2);
        assert_eq!(buf1.num_samples(), 512);

        pool.return_buffer(buf1);
        assert_eq!(pool.available(), 4);
    }

    #[test]
    fn checkout_beyond_capacity_allocates() {
        let pool = BufferPool::new(1, 256, 44100, 1);
        let _b1 = pool.checkout();
        let _b2 = pool.checkout(); // should allocate a new one
        assert_eq!(pool.available(), 0);
    }
}
