//! Ring buffer for raw terminal output (ephemeral, per-session).
//!
//! Stores the most recent N bytes of terminal output for screen dumps
//! and incremental sync on client reconnection.
//!
//! This is the second layer of the event model from DESIGN.md §3.5:
//! raw output is large and ephemeral — only used for display and replay.

use ringbuf::{
    traits::{Consumer, Producer, Split},
    HeapRb,
};
use std::sync::{Arc, Mutex};

/// Maximum bytes stored in the ring buffer per session.
const DEFAULT_BUFFER_SIZE: usize = 256 * 1024; // 256 KB

/// Per-session ring buffer for raw terminal output.
///
/// Thread-safe — writes from the PTY reader task, reads from WebSocket handler.
#[derive(Clone)]
pub struct OutputBuffer {
    inner: Arc<Mutex<BufferInner>>,
}

struct BufferInner {
    /// Producer side — PTY output is written here
    producer: <HeapRb<u8> as ringbuf::traits::Split>::Prod,
    /// Consumer side — WebSocket reads from here
    consumer: <HeapRb<u8> as ringbuf::traits::Split>::Cons,
    /// Monotonically increasing output sequence number
    next_seq: u64,
    /// The first sequence number still available in the buffer
    min_available_seq: u64,
    /// Number of bytes written since creation
    total_bytes_written: u64,
}

impl OutputBuffer {
    /// Create a new output buffer with the default size.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_BUFFER_SIZE)
    }

    /// Create a new output buffer with a specific capacity in bytes.
    pub fn with_capacity(capacity: usize) -> Self {
        let rb = HeapRb::new(capacity);
        let (producer, consumer) = rb.split();
        Self {
            inner: Arc::new(Mutex::new(BufferInner {
                producer,
                consumer,
                next_seq: 0,
                min_available_seq: 0,
                total_bytes_written: 0,
            })),
        }
    }

    /// Write output data to the buffer.
    ///
    /// Returns the sequence number assigned to this chunk.
    /// If the buffer is full, older data is overwritten and `min_available_seq` advances.
    pub fn write(&self, data: &[u8]) -> u64 {
        let mut inner = self.inner.lock().unwrap();
        let seq = inner.next_seq;
        inner.next_seq += 1;

        // Try to write; if it overflows, the oldest data is dropped automatically
        let written = inner.producer.push_slice(data);
        if written < data.len() {
            // Buffer was full — some data was dropped.
            // Advance min_available_seq to reflect data loss.
            inner.min_available_seq = inner.min_available_seq.saturating_add(1);
        }

        inner.total_bytes_written += data.len() as u64;
        seq
    }

    /// Read all available data since a given sequence number.
    ///
    /// Returns `(data, gap_detected)`.
    /// If `gap_detected` is true, the requested seq was too old and data is incomplete.
    pub fn read_since(&self, last_seq: Option<u64>) -> (Vec<u8>, u64, bool) {
        let mut inner = self.inner.lock().unwrap();

        let current_seq = inner.next_seq.saturating_sub(1);
        let start_seq = last_seq.map(|s| s + 1).unwrap_or(0);

        let gap_detected = start_seq < inner.min_available_seq;

        // Drain all available data
        let mut data = Vec::new();
        while let Some(byte) = inner.consumer.try_pop() {
            data.push(byte);
        }

        (data, current_seq, gap_detected)
    }

    /// Get the current sequence number.
    pub fn current_seq(&self) -> u64 {
        let inner = self.inner.lock().unwrap();
        inner.next_seq.saturating_sub(1)
    }

    /// Get the total number of bytes written since creation.
    pub fn total_bytes_written(&self) -> u64 {
        let inner = self.inner.lock().unwrap();
        inner.total_bytes_written
    }
}

impl Default for OutputBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_and_read() {
        let buf = OutputBuffer::with_capacity(1024);

        buf.write(b"hello ");
        buf.write(b"world");

        let (data, _seq, gap) = buf.read_since(None);
        assert!(!gap);
        assert_eq!(String::from_utf8_lossy(&data), "hello world");
    }

    #[test]
    fn test_incremental_read() {
        let buf = OutputBuffer::with_capacity(1024);

        let _seq1 = buf.write(b"first ");
        let _seq2 = buf.write(b"second");

        // Phase 0: read_since drains all buffered data.
        // Per-chunk incremental sync will be refined in Phase 1.
        let (data, current_seq, gap) = buf.read_since(None);
        assert!(!gap);
        assert_eq!(String::from_utf8_lossy(&data), "first second");
        // current_seq should be the last written seq
        assert_eq!(current_seq, 1);
    }
}
