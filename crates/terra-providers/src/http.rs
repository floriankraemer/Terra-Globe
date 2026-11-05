//! Minimal HTTP port used by the provider adapters.
//!
//! Keeping this a plain synchronous trait (rather than pulling `reqwest` or a
//! runtime into this crate) means providers are trivially testable with a fake
//! client, and the desktop/browser shells pick the concrete transport.

use std::collections::VecDeque;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

impl HttpRequest {
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            method: HttpMethod::Get,
            url: url.into(),
            headers: vec![("Accept".to_string(), "application/json".to_string())],
            body: None,
        }
    }

    pub fn post_json(url: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            method: HttpMethod::Post,
            url: url.into(),
            headers: vec![
                ("Accept".to_string(), "application/json".to_string()),
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
            body: Some(body.into().into_bytes()),
        }
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self {
            status: 200,
            body: body.into(),
        }
    }

    pub fn status(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            body: body.into(),
        }
    }

    pub fn is_ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("network error: {0}")]
    Network(String),
    #[error("HTTP transport is not available on this target")]
    Unsupported,
}

pub trait HttpClient: Send + Sync {
    fn execute(&self, request: HttpRequest) -> Result<HttpResponse, HttpError>;
}

/// Native transport on top of `ehttp` (the same crate the globe uses for tiles,
/// so there is one HTTP stack per target). Browser support arrives with the
/// wasm target in M7 via the same trait.
#[derive(Debug, Default, Clone, Copy)]
pub struct EhttpClient;

impl HttpClient for EhttpClient {
    fn execute(&self, request: HttpRequest) -> Result<HttpResponse, HttpError> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut raw = match request.method {
                HttpMethod::Get => ehttp::Request::get(request.url),
                HttpMethod::Post => ehttp::Request {
                    body: request.body.clone().unwrap_or_default(),
                    ..ehttp::Request::post(request.url, Vec::new())
                },
            };
            for (name, value) in &request.headers {
                raw.headers.headers.push((name.clone(), value.clone()));
            }
            raw.timeout = Some(std::time::Duration::from_secs(30));

            let response = ehttp::fetch_blocking(&raw)
                .map_err(|error| HttpError::Network(error.to_string()))?;
            Ok(HttpResponse {
                status: response.status,
                body: response.bytes,
            })
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = request;
            Err(HttpError::Unsupported)
        }
    }
}

/// Deterministic client for tests: returns queued responses in order and
/// records every request so URL/query construction can be asserted.
#[derive(Debug, Default)]
pub struct FakeHttpClient {
    responses: Mutex<VecDeque<HttpResponse>>,
    requests: Mutex<Vec<HttpRequest>>,
}

impl FakeHttpClient {
    pub fn new(responses: Vec<HttpResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
            requests: Mutex::new(Vec::new()),
        }
    }

    pub fn requests(&self) -> Vec<HttpRequest> {
        self.requests
            .lock()
            .expect("fake client lock poisoned")
            .clone()
    }

    pub fn last_request(&self) -> HttpRequest {
        self.requests()
            .last()
            .cloned()
            .expect("no request was recorded")
    }
}

impl HttpClient for FakeHttpClient {
    fn execute(&self, request: HttpRequest) -> Result<HttpResponse, HttpError> {
        self.requests
            .lock()
            .expect("fake client lock poisoned")
            .push(request);
        self.responses
            .lock()
            .expect("fake client lock poisoned")
            .pop_front()
            .ok_or_else(|| HttpError::Network("no response queued".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_client_records_requests_and_returns_queued_responses() {
        let client = FakeHttpClient::new(vec![HttpResponse::ok("{}")]);
        let response = client
            .execute(HttpRequest::get("https://example.test/"))
            .expect("queued response");
        assert!(response.is_ok());
        assert_eq!(client.requests().len(), 1);
        assert_eq!(client.last_request().url, "https://example.test/");
    }

    #[test]
    fn fake_client_fails_when_the_queue_is_empty() {
        let client = FakeHttpClient::default();
        assert!(
            client
                .execute(HttpRequest::get("https://example.test/"))
                .is_err()
        );
    }

    #[test]
    fn status_helpers_are_consistent() {
        assert!(HttpResponse::ok("").is_ok());
        assert!(!HttpResponse::status(404, "").is_ok());
    }
}
