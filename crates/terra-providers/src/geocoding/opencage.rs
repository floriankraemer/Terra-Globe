//! OpenCage Geocoder adapter, requires an API key.

use std::sync::Arc;

use serde_json::Value;
use terra_core::GeoPoint;

use super::nominatim::{decode_json, parse_base_url};
use super::{GeocodeResult, GeocodingProvider};
use crate::ProviderError;
use crate::http::{HttpClient, HttpRequest};

const OPENCAGE_SEARCH_URL: &str = "https://api.opencagedata.com/geocode/v1/json";

pub struct OpenCageGeocodingProvider {
    api_key: String,
    client: Arc<dyn HttpClient>,
}

impl OpenCageGeocodingProvider {
    pub fn new(api_key: String, client: Arc<dyn HttpClient>) -> Self {
        Self { api_key, client }
    }
}

impl GeocodingProvider for OpenCageGeocodingProvider {
    fn search(&self, query: &str) -> Result<Vec<GeocodeResult>, ProviderError> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }
        self.request(trimmed, "OpenCage search failed")
    }

    fn reverse(&self, point: GeoPoint) -> Result<Option<String>, ProviderError> {
        let query = format!("{}+{}", point.lat, point.lon);
        let label = self.request(&query, "OpenCage reverse geocode failed")?;
        Ok(label.into_iter().next().map(|result| result.label))
    }
}

impl OpenCageGeocodingProvider {
    /// Both directions share the search endpoint; the reverse query is `lat+lon`.
    fn request(
        &self,
        query: &str,
        failure_label: &'static str,
    ) -> Result<Vec<GeocodeResult>, ProviderError> {
        let mut url = parse_base_url(OPENCAGE_SEARCH_URL)?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("q", query);
            pairs.append_pair("key", &self.api_key);
        }

        let response = self.client.execute(HttpRequest::get(url.as_str()))?;
        if !response.is_ok() {
            return Err(ProviderError::Request(format!(
                "{failure_label}: {}",
                response.status
            )));
        }

        let body = decode_json(&response)?;
        Ok(map_opencage_results(&body))
    }
}

/// `mapOpenCageResults` from `OpenCageGeocodingProvider.ts`: entries that are
/// not shaped like an OpenCage result are filtered out instead of failing.
fn map_opencage_results(body: &Value) -> Vec<GeocodeResult> {
    let Some(results) = body.get("results").and_then(Value::as_array) else {
        return Vec::new();
    };
    results
        .iter()
        .filter_map(|result| {
            let label = result.get("formatted")?.as_str()?;
            let geometry = result.get("geometry")?.as_object()?;
            let lat = geometry.get("lat")?.as_f64()?;
            let lng = geometry.get("lng")?.as_f64()?;
            Some(GeocodeResult {
                label: label.to_owned(),
                point: GeoPoint {
                    lon: lng,
                    lat,
                    altitude: None,
                    altitude_mode: None,
                },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{FakeHttpClient, HttpResponse};

    fn provider(client: &Arc<FakeHttpClient>) -> OpenCageGeocodingProvider {
        OpenCageGeocodingProvider::new("oc-key".to_owned(), client.clone())
    }

    fn pairs(url: &url::Url) -> Vec<(String, String)> {
        url.query_pairs().into_owned().collect()
    }

    #[test]
    fn search_builds_the_opencage_url_and_maps_results() {
        let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::ok(
            r#"{"results": [
                {"formatted": "Brandenburger Tor, Berlin", "geometry": {"lat": 52.5163, "lng": 13.3777}},
                {"formatted": "No geometry"},
                {"total_results": 3}
            ]}"#,
        )]));
        let results = provider(&client)
            .search("  Brandenburger Tor  ")
            .expect("search succeeds");

        let url = url::Url::parse(&client.last_request().url).expect("valid url");
        assert_eq!(
            url.as_str(),
            "https://api.opencagedata.com/geocode/v1/json?q=Brandenburger+Tor&key=oc-key"
        );
        assert_eq!(
            pairs(&url),
            vec![
                ("q".to_owned(), "Brandenburger Tor".to_owned()),
                ("key".to_owned(), "oc-key".to_owned()),
            ]
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].label, "Brandenburger Tor, Berlin");
        assert_eq!(results[0].point.lat, 52.5163);
        assert_eq!(results[0].point.lon, 13.3777);
    }

    #[test]
    fn reverse_builds_a_comma_separated_query_and_returns_the_first_label() {
        let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::ok(
            r#"{"results": [{"formatted": "Berlin, Germany", "geometry": {"lat": 52.52, "lng": 13.4}}]}"#,
        )]));
        let point = GeoPoint::new(13.4, 52.52).expect("valid point");
        let label = provider(&client).reverse(point).expect("reverse succeeds");

        let url = url::Url::parse(&client.last_request().url).expect("valid url");
        assert_eq!(
            url.as_str(),
            "https://api.opencagedata.com/geocode/v1/json?q=52.52%2B13.4&key=oc-key"
        );
        assert_eq!(label.as_deref(), Some("Berlin, Germany"));
    }

    #[test]
    fn reverse_returns_none_when_there_are_no_results() {
        let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::ok(
            r#"{"results": [], "total_results": 0}"#,
        )]));
        let point = GeoPoint::new(0.0, 0.0).expect("valid point");
        let label = provider(&client).reverse(point).expect("reverse succeeds");
        assert_eq!(label, None);
    }

    #[test]
    fn search_surfaces_request_errors() {
        let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::status(402, "")]));
        let error = provider(&client).search("Berlin").expect_err("must fail");
        assert!(
            matches!(error, ProviderError::Request(message) if message == "OpenCage search failed: 402")
        );
    }

    #[test]
    fn reverse_surfaces_request_errors_with_the_reverse_wording() {
        let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::status(402, "")]));
        let point = GeoPoint::new(0.0, 0.0).expect("valid point");
        let error = provider(&client).reverse(point).expect_err("must fail");
        assert!(
            matches!(error, ProviderError::Request(message) if message == "OpenCage reverse geocode failed: 402")
        );
    }

    #[test]
    fn a_malformed_body_maps_to_no_results() {
        for body in [r#"[]"#, r#"{"results": null}"#, r#"{"results": [1, "x"]}"#] {
            let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::ok(body)]));
            let results = provider(&client).search("Berlin").expect("search succeeds");
            assert!(results.is_empty(), "body {body} must map to no results");
        }
    }

    #[test]
    fn an_empty_query_makes_no_request() {
        let client = Arc::new(FakeHttpClient::default());
        let provider = provider(&client);
        assert_eq!(provider.search("").expect("empty result"), Vec::new());
        assert_eq!(provider.search("\n ").expect("empty result"), Vec::new());
        assert!(client.requests().is_empty());
    }
}
