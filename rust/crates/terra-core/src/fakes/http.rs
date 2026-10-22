use std::sync::Mutex;

use crate::BoxFuture;
use crate::ports::{HttpClient, HttpError, HttpRequest, HttpResponse};

type Rule = Box<dyn Fn(&HttpRequest) -> Option<Result<HttpResponse, HttpError>> + Send + Sync>;

/// Scripted HTTP client: the first matching rule answers; unmatched requests
/// fail with a network error. Every request is recorded for assertions.
#[derive(Default)]
pub struct FakeHttpClient {
    rules: Mutex<Vec<Rule>>,
    requests: Mutex<Vec<HttpRequest>>,
}

impl std::fmt::Debug for FakeHttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeHttpClient")
            .field("requests", &self.requests)
            .finish_non_exhaustive()
    }
}

impl FakeHttpClient {
    /// Answers any request whose URL contains `url_fragment`.
    pub fn when_url_contains(
        &self,
        url_fragment: impl Into<String>,
        response: Result<HttpResponse, HttpError>,
    ) {
        let fragment = url_fragment.into();
        self.rules
            .lock()
            .expect("poisoned")
            .push(Box::new(move |req| {
                req.url.contains(&fragment).then(|| response.clone())
            }));
    }

    pub fn when(
        &self,
        rule: impl Fn(&HttpRequest) -> Option<Result<HttpResponse, HttpError>> + Send + Sync + 'static,
    ) {
        self.rules.lock().expect("poisoned").push(Box::new(rule));
    }

    pub fn requests(&self) -> Vec<HttpRequest> {
        self.requests.lock().expect("poisoned").clone()
    }
}

impl HttpClient for FakeHttpClient {
    fn send(&self, request: HttpRequest) -> BoxFuture<'_, Result<HttpResponse, HttpError>> {
        self.requests
            .lock()
            .expect("poisoned")
            .push(request.clone());
        let answer = self
            .rules
            .lock()
            .expect("poisoned")
            .iter()
            .find_map(|rule| rule(&request))
            .unwrap_or_else(|| {
                Err(HttpError::Network(format!(
                    "no fake rule for {}",
                    request.url
                )))
            });
        Box::pin(async move { answer })
    }
}
