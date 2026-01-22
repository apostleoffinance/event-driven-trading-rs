use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use crate::error::Result;
use super::event::Event;

/// Handler function type for event subscribers
pub type EventHandler = Arc<dyn Fn(&Event) + Send + Sync>;

/// Event Bus - Central pub/sub mechanism for all trading events
pub struct EventBus {
    subscribers: Arc<Mutex<HashMap<String, Vec<EventHandler>>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Subscribe to events of a specific type
    pub fn subscribe<F>(&self, event_type: &str, handler: F) -> Result<()>
    where
        F: Fn(&Event) + Send + Sync + 'static,
    {
        let mut subs = self.subscribers.lock().unwrap();
        subs.entry(event_type.to_string())
            .or_insert_with(Vec::new)
            .push(Arc::new(handler));
        Ok(())
    }

    /// Publish an event to all subscribers
    pub fn publish(&self, event: Event) -> Result<()> {
        let event_type = event.event_type().to_string();
        let subs = self.subscribers.lock().unwrap();

        if let Some(handlers) = subs.get(&event_type) {
            for handler in handlers {
                handler(&event);
            }
        }

        Ok(())
    }

    /// Publish to all subscribers regardless of event type
    pub fn publish_all(&self, event: Event) -> Result<()> {
        let subs = self.subscribers.lock().unwrap();

        // Call all handlers for all event types
        for handlers in subs.values() {
            for handler in handlers {
                handler(&event);
            }
        }

        Ok(())
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            subscribers: Arc::clone(&self.subscribers),
        }
    }
}