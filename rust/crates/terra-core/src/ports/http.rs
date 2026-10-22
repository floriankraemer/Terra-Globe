use crate::BoxFuture;

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
            headers: Vec::new(),
            body: None,
        }
    }

    pub fn post(url: impl Into<String>, body: Vec<u8>) -> Self {
        Self {
            method: HttpMethod::Post,
            url: url.into(),
            headers: Vec::new(),
            body: Some(body),
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
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self {
            status: 200,
            headers: Vec::new(),
            body: body.into(),
        }
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HttpError {
    #[error("network error: {0}")]
    Network(String),
    #[error("request timed out")]
    Timeout,
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
}

/// HTTP port. Every network adapter (tiles, geocoding, routing, NetworkLinks)
/// goes through this so core stays executor- and platform-free.
pub trait HttpClient: Send + Sync {
    fn send(&self, request: HttpRequest) -> BoxFuture<'_, Result<HttpResponse, HttpError>>;
}
