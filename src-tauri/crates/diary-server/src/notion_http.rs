//! The production Notion transport: reqwest over the OS TLS stack, driven from the blocking
//! threads the export service already uses. The host is fixed; it is never configurable, so the
//! credential cannot be sent anywhere else.
use diary_infrastructure::export::notion::http::{Failure, Http, Method, Request, Response};
use std::time::Duration;

const BASE: &str = "https://api.notion.com";
/// Compatibility target from the export contract; revalidated for P1-M27.
const VERSION: &str = "2026-03-11";
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

pub struct Reqwest {
    client: reqwest::Client,
    token: String,
    handle: tokio::runtime::Handle,
    base: String,
}

impl Reqwest {
    /// Must be called inside the server's runtime; requests later run from blocking threads.
    pub fn new(token: &str) -> Option<Self> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .ok()?;
        Some(Self {
            client,
            token: token.to_string(),
            handle: tokio::runtime::Handle::try_current().ok()?,
            base: BASE.into(),
        })
    }
    /// Blocks for the reply. Called from a runtime worker (startup recovery), it first hands the
    /// worker's queue to another thread; `block_on` there would otherwise panic.
    fn block<T>(&self, f: impl std::future::Future<Output = T>) -> T {
        if tokio::runtime::Handle::try_current().is_ok() {
            tokio::task::block_in_place(|| self.handle.block_on(f))
        } else {
            self.handle.block_on(f)
        }
    }
}

impl Http for Reqwest {
    fn download(&self, url: &str, limit: usize) -> Result<Vec<u8>, Failure> {
        let parsed = reqwest::Url::parse(url).map_err(|_| Failure::NotSent)?;
        if parsed.scheme() != "https"
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.port().is_some_and(|p| p != 443)
            || !matches!(
                parsed.host_str(),
                Some(
                    "prod-files-secure.s3.us-west-2.amazonaws.com"
                        | "s3.us-west-2.amazonaws.com"
                        | "file.notion.so"
                )
            )
        {
            return Err(Failure::NotSent);
        }
        self.block(async {
            let mut response = self
                .client
                .get(parsed)
                .send()
                .await
                .map_err(|_| Failure::NotSent)?;
            if response.status() != reqwest::StatusCode::OK
                || response.content_length().is_some_and(|n| n > limit as u64)
            {
                return Err(Failure::NotSent);
            }
            let mut out = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| Failure::NotSent)? {
                if out.len() + chunk.len() > limit {
                    return Err(Failure::NotSent);
                }
                out.extend_from_slice(&chunk);
            }
            Ok(out)
        })
    }
    fn send(&self, request: &Request) -> Result<Response, Failure> {
        let method = match request.method {
            Method::Get => reqwest::Method::GET,
            Method::Post => reqwest::Method::POST,
            Method::Patch => reqwest::Method::PATCH,
            Method::Delete => reqwest::Method::DELETE,
        };
        let mut builder = self
            .client
            .request(method, format!("{}{}", self.base, request.path))
            .header("Notion-Version", VERSION);
        let mut auth = reqwest::header::HeaderValue::from_str(&format!("Bearer {}", self.token))
            .map_err(|_| Failure::NotSent)?;
        auth.set_sensitive(true);
        builder = builder.header(reqwest::header::AUTHORIZATION, auth);
        if let Some(body) = &request.body {
            builder = builder
                .header(
                    reqwest::header::CONTENT_TYPE,
                    if request.multipart {
                        "multipart/form-data; boundary=diary-jpeg-6a8026b247344aa087726ffb04db8229"
                    } else {
                        "application/json"
                    },
                )
                .body(body.clone());
        }
        self.block(async {
            let response = builder.send().await.map_err(|e| {
                // Connection setup failures mean nothing reached the service.
                if e.is_connect() || e.is_builder() {
                    Failure::NotSent
                } else {
                    Failure::Ambiguous
                }
            })?;
            let status = response.status().as_u16();
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse().ok());
            let body = response.bytes().await.map_err(|_| Failure::Ambiguous)?;
            if body.len() > MAX_RESPONSE_BYTES {
                return Err(Failure::Ambiguous);
            }
            Ok(Response {
                status,
                retry_after,
                body: body.to_vec(),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    // Regression: startup recovery calls `send` on a runtime worker thread.
    #[tokio::test(flavor = "multi_thread")]
    async fn send_works_on_a_runtime_worker() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut c, _) = listener.accept().unwrap();
            let _ = c.read(&mut [0; 2048]);
            c.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
                .unwrap();
        });
        let mut http = Reqwest::new("t").unwrap();
        http.base = format!("http://127.0.0.1:{port}");
        let r = http
            .send(&Request {
                multipart: false,
                method: Method::Get,
                path: "/v1/users/me".into(),
                body: None,
            })
            .unwrap();
        assert_eq!(r.status, 200);
    }
}
