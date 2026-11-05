//! LocationIQ geocoding adapter: Nominatim-compatible response shape, requires an API key.

use std::sync::Arc;

use serde_json::Value;
use terra_core::GeoPoint;

use super::nominatim::{decode_json, first_nominatim_label, map_nominatim_results, parse_base_url};
use super::{GeocodeResult, GeocodingProvider};
use crate::ProviderError;
use crate::http::{HttpClient, HttpRequest};

const LOCATIONIQ_SEARCH_URL: &str = "https://us1.locationiq.com/v1/search";
const LOCATIONIQ_REVERSE_URL: &str = "https://us1.locationiq.com/v1/reverse";

pub struct LocationIqGeocodingProvider {
    api_key: String,
    client: Arc<dyn HttpClient>,
}

impl LocationIqGeocodingProvider {
    pub fn new(api_key: String, client: Arc<dyn HttpClient>) -> Self {
        Self { api_key, client }
    }
}

impl GeocodingProvider for LocationIqGeocodingProvider {
    fn search(&self, query: &str) -> Result<Vec<GeocodeResult>, ProviderError> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let mut url = parse_base_url(LOCATIONIQ_SEARCH_URL)?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("q", trimmed);
            pairs.append_pair("format", "json");
            pairs.append_pair("key", &self.api_key);
        }

        let response = self.client.execute(HttpRequest::get(url.as_str()))?;
        if !response.is_ok() {
            return Err(ProviderError::Request(format!(
                "LocationIQ search failed: {}",
                response.status
            )));
        }

        let body = decode_json(&response)?;
        Ok(map_nominatim_results(&body))
    }

    fn reverse(&self, point: GeoPoint) -> Result<Option<String>, ProviderError> {
        let mut url = parse_base_url(LOCATIONIQ_REVERSE_URL)?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("lat", &point.lat.to_string());
            pairs.append_pair("lon", &point.lon.to_string());
            pairs.append_pair("format", "json");
            pairs.append_pair("key", &self.api_key);
        }

        let response = self.client.execute(HttpRequest::get(url.as_str()))?;
        if !response.is_ok() {
            return Err(ProviderError::Request(format!(
                "LocationIQ reverse geocode failed: {}",
                response.status
            )));
        }

        let body: Value = decode_json(&response)?;
        Ok(first_nominatim_label(&body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{FakeHttpClient, HttpResponse};

    fn provider(client: &Arc<FakeHttpClient>) -> LocationIqGeocodingProvider {
        LocationIqGeocodingProvider::new("liq-key".to_owned(), client.clone())
    }

    fn pairs(url: &url::Url) -> Vec<(String, String)> {
        url.query_pairs().into_owned().collect()
    }

    #[test]
    fn search_builds_the_locationiq_url_and_maps_results() {
        let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::ok(
            r#"[
                {"display_name": "Paris, France", "lat": "48.8566", "lon": "2.3522", "type": "city"},
                {"place": "not a nominatim result"}
            ]"#,
        )]));
        let results = provider(&client).search("Paris").expect("search succeeds");

        let url = url::Url::parse(&client.last_request().url).expect("valid url");
        assert_eq!(
            url.as_str(),
            "https://us1.locationiq.com/v1/search?q=Paris&format=json&key=liq-key"
        );
        assert_eq!(
            pairs(&url),
            vec![
                ("q".to_owned(), "Paris".to_owned()),
                ("format".to_owned(), "json".to_owned()),
                ("key".to_owned(), "liq-key".to_owned()),
            ]
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].label, "Paris, France");
        assert_eq!(results[0].point.lat, 48.8566);
        assert_eq!(results[0].point.lon, 2.3522);
    }

    #[test]
    fn reverse_builds_the_reverse_url_and_returns_the_first_label() {
        let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::ok(
            r#"{"display_name": "Eiffel Tower, Paris, France", "lat": "48.8584", "lon": "2.2945"}"#,
        )]));
        let point = GeoPoint::new(2.2945, 48.8584).expect("valid point");
        let label = provider(&client).reverse(point).expect("reverse succeeds");

        let url = url::Url::parse(&client.last_request().url).expect("valid url");
        assert_eq!(
            url.as_str(),
            "https://us1.locationiq.com/v1/reverse?lat=48.8584&lon=2.2945&format=json&key=liq-key"
        );
        assert_eq!(label.as_deref(), Some("Eiffel Tower, Paris, France"));
    }

    #[test]
    fn reverse_returns_none_for_a_non_result_body() {
        let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::ok("[]")]));
        let point = GeoPoint::new(0.0, 0.0).expect("valid point");
        let label = provider(&client).reverse(point).expect("reverse succeeds");
        assert_eq!(label, None);
    }

    #[test]
    fn search_surfaces_request_errors() {
        let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::status(401, "")]));
        let error = provider(&client).search("Paris").expect_err("must fail");
        assert!(
            matches!(error, ProviderError::Request(message) if message == "LocationIQ search failed: 401")
        );
    }

    #[test]
    fn reverse_surfaces_request_errors_with_the_reverse_wording() {
        let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::status(401, "")]));
        let point = GeoPoint::new(0.0, 0.0).expect("valid point");
        let error = provider(&client).reverse(point).expect_err("must fail");
        assert!(
            matches!(error, ProviderError::Request(message) if message == "LocationIQ reverse geocode failed: 401")
        );
    }

    #[test]
    fn a_malformed_body_maps_to_no_results() {
        let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::ok("null")]));
        let results = provider(&client).search("Paris").expect("search succeeds");
        assert!(results.is_empty());
    }

    #[test]
    fn an_empty_query_makes_no_request() {
        let client = Arc::new(FakeHttpClient::default());
        let provider = provider(&client);
        assert_eq!(provider.search("").expect("empty result"), Vec::new());
        assert_eq!(provider.search(" \t ").expect("empty result"), Vec::new());
        assert!(client.requests().is_empty());
    }
}
