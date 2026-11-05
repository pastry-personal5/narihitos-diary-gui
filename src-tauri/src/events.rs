//! Bounded SSE decoder and replay filter. Raw frames never cross IPC.
use crate::bridge::Job;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BridgeEvent {
    JobSnapshot {
        instance_id: String,
        session_id: String,
        jobs: Vec<Job>,
        first: bool,
        complete: bool,
    },
    JobChanged {
        instance_id: String,
        session_id: String,
        event_id: u64,
        job_id: String,
        job_sequence: u64,
        payload: Job,
    },
    ResyncRequired {
        instance_id: String,
        session_id: String,
    },
    Disconnected,
}
#[derive(Deserialize)]
struct WireEvent {
    instance_id: String,
    session_id: String,
    event_id: u64,
    job_id: String,
    job_sequence: u64,
    kind: String,
    payload: Job,
}
#[derive(Default)]
pub struct Cursor {
    pub last: u64,
    sequences: BTreeMap<String, u64>,
}
impl Cursor {
    pub fn at(last: u64) -> Self {
        Self {
            last,
            sequences: BTreeMap::new(),
        }
    }
    pub fn decode(
        &mut self,
        frame: &str,
        instance: &str,
        session: &str,
    ) -> Result<Option<BridgeEvent>, &'static str> {
        let mut kind = None;
        let mut id = None;
        let mut data = String::new();
        for line in frame.lines() {
            if line.starts_with(':') {
                continue;
            }
            let (key, value) = line.split_once(':').unwrap_or((line, ""));
            let value = value.strip_prefix(' ').unwrap_or(value);
            match key {
                "event" => kind = Some(value),
                "id" => id = Some(value.parse::<u64>().map_err(|_| "invalid_event")?),
                "data" => {
                    if !data.is_empty() {
                        data.push('\n');
                    }
                    data.push_str(value);
                }
                _ => (),
            }
        }
        if data.is_empty() {
            return Ok(None);
        }
        if data.len() > 65536 {
            return Err("event_limit");
        }
        if kind == Some("resync_required") {
            return Ok(Some(BridgeEvent::ResyncRequired {
                instance_id: instance.into(),
                session_id: session.into(),
            }));
        }
        if kind != Some("job_changed") {
            return Err("unknown_event");
        }
        let wire: WireEvent = serde_json::from_str(&data).map_err(|_| "invalid_event")?;
        if wire.instance_id != instance || wire.session_id != session {
            return Ok(None);
        }
        if id != Some(wire.event_id)
            || wire.kind != "job_changed"
            || wire.job_id != wire.payload.id
            || wire.job_sequence != wire.payload.sequence
        {
            return Err("invalid_event");
        }
        if wire.event_id <= self.last {
            return Ok(None);
        }
        if wire.job_id.len() > 64 || wire.job_sequence == 0 {
            return Err("invalid_event");
        }
        if self.sequences.len() >= 4096 && !self.sequences.contains_key(&wire.job_id) {
            return Err("event_limit");
        }
        self.last = wire.event_id;
        if self
            .sequences
            .get(&wire.job_id)
            .is_some_and(|seq| *seq >= wire.job_sequence)
        {
            return Ok(None);
        }
        self.sequences
            .insert(wire.job_id.clone(), wire.job_sequence);
        Ok(Some(BridgeEvent::JobChanged {
            instance_id: wire.instance_id,
            session_id: wire.session_id,
            event_id: wire.event_id,
            job_id: wire.job_id,
            job_sequence: wire.job_sequence,
            payload: wire.payload,
        }))
    }
}
#[derive(Default)]
pub struct Decoder {
    buffer: Vec<u8>,
}
impl Decoder {
    pub fn feed(&mut self, chunk: &[u8]) -> Result<Vec<String>, &'static str> {
        let mut frames = Vec::new();
        let mut decoded_bytes = 0usize;
        for byte in chunk {
            self.buffer.push(*byte);
            if self.buffer.len() > 66560 {
                return Err("event_limit");
            }
            if self.buffer.ends_with(b"\n\n") || self.buffer.ends_with(b"\r\n\r\n") {
                let frame = String::from_utf8(std::mem::take(&mut self.buffer))
                    .map_err(|_| "invalid_event")?;
                decoded_bytes += frame.len();
                if decoded_bytes > 4 * 1024 * 1024 {
                    return Err("event_limit");
                }
                frames.push(frame);
                // Bound one chunk's decoded output as well as the partial frame.
                if frames.len() > 2048 {
                    return Err("event_limit");
                }
            }
        }
        Ok(frames)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn frame(instance: &str, session: &str, event: u64, sequence: u64) -> String {
        format!(
            "event: job_changed\nid: {event}\ndata: {{\"kind\":\"job_changed\",\"instance_id\":\"{instance}\",\"session_id\":\"{session}\",\"event_id\":{event},\"job_id\":\"job\",\"job_sequence\":{sequence},\"payload\":{{\"id\":\"job\",\"sequence\":{sequence},\"state\":\"running\"}}}}\n\n"
        )
    }
    #[test]
    fn split_frames_crlf_comments_and_limits() {
        let mut d = Decoder::default();
        assert!(d.feed(b": ping\r\n\r").unwrap().is_empty());
        assert_eq!(d.feed(b"\ndata: x\n\n").unwrap().len(), 2);
        assert!(d.feed(&vec![b'x'; 66561]).is_err());
    }
    #[test]
    fn suppresses_duplicates_stale_sessions_and_job_sequences() {
        let mut c = Cursor::default();
        assert!(
            c.decode(&frame("i", "s", 1, 1), "i", "s")
                .unwrap()
                .is_some()
        );
        for f in [
            frame("i", "s", 1, 1),
            frame("old", "s", 2, 2),
            frame("i", "old", 2, 2),
            frame("i", "s", 2, 1),
        ] {
            assert!(c.decode(&f, "i", "s").unwrap().is_none());
        }
        assert!(
            c.decode(&frame("i", "s", 3, 2), "i", "s")
                .unwrap()
                .is_some()
        );
    }
    #[test]
    fn validates_required_variants_and_resync() {
        let mut c = Cursor::default();
        assert!(c.decode("event: future\ndata: {}\n\n", "i", "s").is_err());
        assert!(
            c.decode(
                &frame("i", "s", 1, 1).replace("\"running\"", "\"future\""),
                "i",
                "s"
            )
            .is_err()
        );
        assert!(matches!(
            c.decode("event: resync_required\ndata: {}\n\n", "i", "s")
                .unwrap(),
            Some(BridgeEvent::ResyncRequired { .. })
        ));
    }
}
