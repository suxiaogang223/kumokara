//! Bounded, non-destructive terminal output history used for reconnect replay.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

const DEFAULT_CAPACITY: usize = 256 * 1024;

#[derive(Clone, Debug)]
pub(crate) struct HistoryChunk {
    pub seq: u64,
    pub data: Vec<u8>,
}

#[derive(Clone)]
pub(crate) struct OutputHistory {
    inner: Arc<Mutex<HistoryState>>,
}

struct HistoryState {
    chunks: VecDeque<HistoryChunk>,
    bytes: usize,
    capacity: usize,
    next_seq: u64,
    dropped_through_seq: Option<u64>,
}

impl OutputHistory {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    fn with_capacity(capacity: usize) -> Self {
        assert!(capacity > 0, "output history capacity must be positive");
        Self {
            inner: Arc::new(Mutex::new(HistoryState {
                chunks: VecDeque::new(),
                bytes: 0,
                capacity,
                next_seq: 0,
                dropped_through_seq: None,
            })),
        }
    }

    pub fn push(&self, data: &[u8]) -> u64 {
        let mut state = self.inner.lock().expect("output history lock poisoned");
        let seq = state.next_seq;
        state.next_seq += 1;

        let data = if data.len() > state.capacity {
            state.dropped_through_seq = Some(seq);
            data[data.len() - state.capacity..].to_vec()
        } else {
            data.to_vec()
        };
        state.bytes += data.len();
        state.chunks.push_back(HistoryChunk { seq, data });

        while state.bytes > state.capacity {
            if let Some(removed) = state.chunks.pop_front() {
                state.bytes -= removed.data.len();
                state.dropped_through_seq = Some(
                    state
                        .dropped_through_seq
                        .map_or(removed.seq, |previous| previous.max(removed.seq)),
                );
            }
        }

        seq
    }

    /// Returns `(chunks, next_seq, gap_detected)` without consuming history.
    pub fn since(&self, last_seq: Option<u64>) -> (Vec<HistoryChunk>, u64, bool) {
        let state = self.inner.lock().expect("output history lock poisoned");
        let requested_seq = last_seq.map_or(0, |seq| seq.saturating_add(1));
        let gap_detected = state
            .dropped_through_seq
            .is_some_and(|dropped_seq| requested_seq <= dropped_seq);
        let chunks = state
            .chunks
            .iter()
            .filter(|chunk| chunk.seq >= requested_seq)
            .cloned()
            .collect();
        (chunks, state.next_seq, gap_detected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_history_starts_live_stream_at_zero() {
        let history = OutputHistory::with_capacity(32);
        let (chunks, next_seq, gap) = history.since(None);
        assert!(chunks.is_empty());
        assert_eq!(next_seq, 0);
        assert!(!gap);
    }

    #[test]
    fn replay_is_incremental_and_non_destructive() {
        let history = OutputHistory::with_capacity(32);
        history.push(b"first");
        history.push(b"second");

        let (all, seq, gap) = history.since(None);
        assert_eq!(seq, 2);
        assert!(!gap);
        assert_eq!(all.len(), 2);

        let (incremental, _, gap) = history.since(Some(0));
        assert!(!gap);
        assert_eq!(incremental[0].data, b"second");
        assert_eq!(history.since(None).0.len(), 2);
    }

    #[test]
    fn reports_evicted_history() {
        let history = OutputHistory::with_capacity(5);
        history.push(b"old");
        history.push(b"new");

        let (chunks, _, gap) = history.since(None);
        assert!(gap);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].data, b"new");
    }

    #[test]
    fn retains_the_tail_of_an_oversized_chunk_and_reports_the_gap() {
        let history = OutputHistory::with_capacity(4);
        history.push(b"123456");

        let (chunks, _, gap) = history.since(None);
        assert!(gap);
        assert_eq!(chunks[0].data, b"3456");
    }
}
