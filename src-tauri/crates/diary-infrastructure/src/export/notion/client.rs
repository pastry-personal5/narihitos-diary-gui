//! Credential-scoped request queue with the documented retry rules. Only reads and requests
//! Notion explicitly rejected (429 and 529) are retried; a write that timed out or failed with
//! a 5xx is never replayed. Errors carry no body, header or token text.
use super::http::{Failure, Http, Method, Request};
use serde_json::Value;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// Retry budget from the configuration contract: five attempts and 60 seconds.
pub const MAX_ATTEMPTS: u32 = 5;
pub const BUDGET_SECS: u64 = 60;
/// Documented ceiling is 500 KB; chunking budgets 400 KB, this is the hard guard.
pub const MAX_REQUEST_BYTES: usize = 500_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Notion asked to wait this many seconds, beyond the budget. Nothing was committed.
    Throttled(u64),
    Unauthorized,
    NotFound,
    /// A definitive 4xx refusal; nothing was committed.
    Rejected,
    /// Nothing was sent.
    Network,
    /// A write may or may not have been committed.
    Ambiguous,
    /// A read failed after the retry budget.
    Server,
    /// A successful read returned an unexpected shape.
    Malformed,
    TooLarge,
}

pub type Sleep = Arc<dyn Fn(Duration) + Send + Sync>;

pub struct Client {
    http: Arc<dyn Http>,
    sleep: Sleep,
    spacing: Duration,
    next: Mutex<Option<Instant>>,
}

impl Client {
    pub fn new(http: Arc<dyn Http>, sleep: Sleep, spacing: Duration) -> Self {
        Self {
            http,
            sleep,
            spacing,
            next: Mutex::new(None),
        }
    }
    pub fn production(http: Arc<dyn Http>) -> Self {
        // Roughly three requests per second, the documented average rate.
        Self::new(
            http,
            Arc::new(std::thread::sleep),
            Duration::from_millis(340),
        )
    }
    fn pace(&self) {
        let wait = {
            let Ok(mut next) = self.next.lock() else {
                return;
            };
            let now = Instant::now();
            let at = next.map_or(now, |n| n.max(now));
            *next = Some(at + self.spacing);
            at.saturating_duration_since(now)
        };
        if !wait.is_zero() {
            (self.sleep)(wait);
        }
    }
    fn jitter() -> Duration {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        Duration::from_millis(u64::from(nanos % 250))
    }

    pub fn call(&self, method: Method, path: &str, body: Option<&Value>) -> Result<Value, Error> {
        let write = method != Method::Get;
        let bytes = body.and_then(|b| serde_json::to_vec(b).ok());
        if bytes.as_ref().is_some_and(|b| b.len() > MAX_REQUEST_BYTES) {
            return Err(Error::TooLarge);
        }
        let request = Request {
            method,
            path: path.into(),
            body: bytes,
        };
        let mut waited = 0u64;
        for attempt in 1..=MAX_ATTEMPTS {
            self.pace();
            let backoff = 1u64 << attempt.min(3);
            let response = match self.http.send(&request) {
                Err(Failure::NotSent) => return Err(Error::Network),
                Err(Failure::Ambiguous) if write => return Err(Error::Ambiguous),
                Err(Failure::Ambiguous) => None,
                Ok(r) => Some(r),
            };
            let wait = match response {
                None => backoff,
                Some(r) => match r.status {
                    200..=299 => {
                        return serde_json::from_slice(&r.body).map_err(|_| {
                            if write {
                                Error::Ambiguous
                            } else {
                                Error::Malformed
                            }
                        });
                    }
                    429 | 529 => {
                        let wait = r
                            .retry_after
                            .or_else(|| body_retry_after(&r.body))
                            .unwrap_or(backoff);
                        if waited + wait > BUDGET_SECS || attempt == MAX_ATTEMPTS {
                            return Err(Error::Throttled(wait));
                        }
                        wait
                    }
                    401 | 403 => return Err(Error::Unauthorized),
                    404 => return Err(Error::NotFound),
                    500..=599 if write => return Err(Error::Ambiguous),
                    500..=599 => backoff,
                    _ => return Err(Error::Rejected),
                },
            };
            if waited + wait > BUDGET_SECS {
                return Err(Error::Server);
            }
            waited += wait;
            (self.sleep)(Duration::from_secs(wait) + Self::jitter());
        }
        Err(Error::Server)
    }
}

/// Notion repeats the wait as `additional_data.retry_after`, a string of whole seconds.
fn body_retry_after(body: &[u8]) -> Option<u64> {
    serde_json::from_slice::<Value>(body)
        .ok()?
        .get("additional_data")?
        .get("retry_after")?
        .as_str()?
        .parse()
        .ok()
}
