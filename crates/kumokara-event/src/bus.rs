//! Per-workspace event broadcast using `tokio::sync::broadcast`.
//!
//! Events are published on the bus and fanned out to:
//! - Connected WebSocket clients (live event stream)
//! - SQLite event log (persistence)
//! - UI badge/notification triggers
//!
//! Slow consumers are detected via `lagged` errors and removed;
//! they can catch up from the event log.

use kumokara_protocol::event::EventEntry;
use tokio::sync::broadcast;

/// Default broadcast channel capacity per workspace.
const DEFAULT_BUS_CAPACITY: usize = 256;

/// Per-workspace event bus.
pub struct EventBus {
    /// Broadcast sender — events published here go to all subscribers.
    tx: broadcast::Sender<EventEntry>,
}

impl EventBus {
    /// Create a new event bus with default capacity.
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(DEFAULT_BUS_CAPACITY);
        Self { tx }
    }

    /// Create a new event bus with a custom capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Publish an event to all subscribers.
    ///
    /// Returns an error if all receivers have been dropped.
    pub fn publish(&self, event: EventEntry) -> Result<usize, broadcast::error::SendError<EventEntry>> {
        self.tx.send(event)
    }

    /// Subscribe to live events.
    ///
    /// Returns a receiver that will get all events published after this call.
    /// For historical events, query the EventLog.
    pub fn subscribe(&self) -> broadcast::Receiver<EventEntry> {
        self.tx.subscribe()
    }

    /// Get the number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kumokara_protocol::event::{Event, EventEntry};

    fn make_event(seq: i64) -> EventEntry {
        EventEntry {
            seq,
            timestamp: chrono::Utc::now().to_rfc3339(),
            session_id: None,
            workspace_id: "ws-1".to_string(),
            source: "test".to_string(),
            event: Event::WorkspaceEvent {
                workspace_id: "ws-1".to_string(),
                description: format!("test event {}", seq),
            },
        }
    }

    #[test]
    fn test_publish_and_subscribe() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        bus.publish(make_event(1)).unwrap();
        bus.publish(make_event(2)).unwrap();

        let received: Vec<_> = rx.try_recv().into_iter().chain(std::iter::from_fn(|| rx.try_recv().ok())).collect();
        assert_eq!(received.len(), 2);
        assert_eq!(received[0].seq, 1);
        assert_eq!(received[1].seq, 2);
    }

    #[test]
    fn test_multiple_subscribers() {
        let bus = EventBus::new();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        bus.publish(make_event(1)).unwrap();

        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_ok());
    }
}
