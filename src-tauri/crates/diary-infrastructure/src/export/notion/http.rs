//! The transport port. The reqwest implementation lives in `diary-server` so this crate needs no
//! HTTP or async dependency; tests use an in-memory fake.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Patch,
    Delete,
}

#[derive(Debug, Clone)]
pub struct Request {
    pub method: Method,
    /// Path and query, starting with `/v1/`. The transport owns host, credential and version.
    pub path: String,
    pub body: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    /// Whole seconds from the `Retry-After` header, when present and numeric.
    pub retry_after: Option<u64>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// Nothing reached the service (DNS, connect or TLS setup).
    NotSent,
    /// The request may have been processed (timeout or a broken connection after sending).
    Ambiguous,
}

pub trait Http: Send + Sync {
    fn send(&self, request: &Request) -> Result<Response, Failure>;
}
