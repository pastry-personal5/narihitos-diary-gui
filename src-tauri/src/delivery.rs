//! Bounded delivery window; acknowledgements never change domain/event cursors.
use crate::events::BridgeEvent;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;
const WINDOW: u64 = 8;
const MAX_MESSAGE_BYTES: usize = 65536;
#[derive(Serialize)]
pub struct Delivery {
    subscription_id: String,
    delivery_id: u64,
    event: BridgeEvent,
}
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum SubscriptionRequest {
    Start {
        channel: tauri::ipc::JavaScriptChannelId,
    },
    Acknowledge {
        subscription_id: String,
        delivery_id: u64,
    },
}
#[derive(Default)]
struct Counts {
    sent: u64,
    acknowledged: u64,
    stopped: bool,
    terminal: bool,
}
pub struct Flow {
    id: String,
    counts: Mutex<Counts>,
    changed: Notify,
}
impl Flow {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            id: uuid::Uuid::new_v4().to_string(),
            counts: Mutex::new(Counts::default()),
            changed: Notify::new(),
        })
    }
    pub fn acknowledge(&self, id: &str, delivery: u64) -> Result<(), &'static str> {
        if id != self.id {
            return Ok(());
        }
        let mut c = self.counts.lock().expect("delivery counts poisoned");
        if delivery > c.sent {
            return Err("invalid_delivery_id");
        }
        if delivery > c.acknowledged {
            c.acknowledged = delivery;
            #[cfg(feature = "scaffold-smoke")]
            eprintln!("SMOKE main_delivery_acknowledged");
            self.changed.notify_one();
        }
        Ok(())
    }
    pub fn stop(&self) {
        self.counts
            .lock()
            .expect("delivery counts poisoned")
            .stopped = true;
        self.changed.notify_one();
    }
    async fn reserve(&self) -> Result<u64, &'static str> {
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            loop {
                {
                    let mut c = self.counts.lock().expect("delivery counts poisoned");
                    if c.stopped {
                        return Err("delivery_stopped");
                    }
                    if c.sent - c.acknowledged < WINDOW {
                        c.sent = c.sent.checked_add(1).ok_or("delivery_limit")?;
                        return Ok(c.sent);
                    }
                }
                self.changed.notified().await;
            }
        })
        .await
        .map_err(|_| "delivery_timeout")?
    }
    pub async fn send(
        &self,
        channel: &tauri::ipc::Channel<Delivery>,
        event: BridgeEvent,
    ) -> Result<(), &'static str> {
        let delivery = Delivery {
            subscription_id: self.id.clone(),
            delivery_id: self.reserve().await?,
            event,
        };
        if serde_json::to_vec(&delivery)
            .map_err(|_| "invalid_delivery")?
            .len()
            > MAX_MESSAGE_BYTES
        {
            self.stop();
            return Err("delivery_limit");
        }
        channel.send(delivery).map_err(|_| "channel_closed")
    }
    /// One reserved control message ends a stalled stream even at a full window.
    pub fn disconnected(&self, channel: &tauri::ipc::Channel<Delivery>) {
        let mut c = self.counts.lock().expect("delivery counts poisoned");
        if c.stopped || c.terminal {
            return;
        }
        c.terminal = true;
        c.stopped = true;
        if let Some(next) = c.sent.checked_add(1) {
            c.sent = next;
            let _ = channel.send(Delivery {
                subscription_id: self.id.clone(),
                delivery_id: next,
                event: BridgeEvent::Disconnected,
            });
        }
        self.changed.notify_one();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test(start_paused = true)]
    async fn full_window_times_out_without_adding_unbounded_messages() {
        let flow = Flow::new();
        for id in 1..=8 {
            assert_eq!(flow.reserve().await, Ok(id));
        }
        assert_eq!(flow.reserve().await, Err("delivery_timeout"));
        assert_eq!(flow.counts.lock().unwrap().sent, 8);
        flow.acknowledge(&flow.id, 4).unwrap();
        for id in 9..=12 {
            assert_eq!(flow.reserve().await, Ok(id));
        }
        assert_eq!(flow.reserve().await, Err("delivery_timeout"));
    }
    #[tokio::test]
    async fn duplicate_foreign_and_future_acknowledgements_cannot_expand_window() {
        let flow = Flow::new();
        flow.reserve().await.unwrap();
        flow.acknowledge("old-subscription", u64::MAX).unwrap();
        assert_eq!(flow.acknowledge(&flow.id, 2), Err("invalid_delivery_id"));
        flow.acknowledge(&flow.id, 1).unwrap();
        flow.acknowledge(&flow.id, 1).unwrap();
        assert_eq!(flow.counts.lock().unwrap().acknowledged, 1);
        flow.stop();
        assert_eq!(flow.reserve().await, Err("delivery_stopped"));
    }
}

#[cfg(test)]
mod channel_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[tokio::test]
    async fn one_reserved_terminal_message_ends_a_full_unacknowledged_channel() {
        let received = Arc::new(AtomicUsize::new(0));
        let count = received.clone();
        let channel = tauri::ipc::Channel::new(move |_| {
            count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        });
        let flow = Flow::new();
        for _ in 0..8 {
            flow.send(&channel, BridgeEvent::Disconnected)
                .await
                .unwrap();
        }
        flow.disconnected(&channel);
        flow.disconnected(&channel);
        assert_eq!(received.load(Ordering::Relaxed), 9);
        assert_eq!(flow.reserve().await, Err("delivery_stopped"));
    }
    #[tokio::test]
    async fn encoded_message_size_is_checked_before_channel_send() {
        let received = Arc::new(AtomicUsize::new(0));
        let count = received.clone();
        let channel = tauri::ipc::Channel::new(move |_| {
            count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        });
        let flow = Flow::new();
        let event = BridgeEvent::JobSnapshot {
            instance_id: "i".into(),
            session_id: "s".into(),
            jobs: vec![crate::bridge::Job {
                id: "x".repeat(65536),
                sequence: 1,
                state: crate::bridge::JobState::Running,
                error_code: None,
            }],
            first: true,
            complete: true,
        };
        assert_eq!(flow.send(&channel, event).await, Err("delivery_limit"));
        assert_eq!(received.load(Ordering::Relaxed), 0);
    }
}
