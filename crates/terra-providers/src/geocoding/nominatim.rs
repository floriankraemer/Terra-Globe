//! Nominatim geocoding adapter: the public OSM Nominatim search API, no API key required.

use std::sync::Arc;

use serde_json::Value;
use terra_core::GeoPoint;

use super::{GeocodeResult, GeocodingProvider};
use crate::ProviderError;
use crate::http::{HttpClient, HttpRequest, HttpResponse};

const NOMINATIM_SEARCH_URL: &str = "https://nominatim.openstreetmap.org/search";
const NOMINATIM_REVERSE_URL: &str = "https://nominatim.openstreetmap.org/reverse";

pub struct NominatimGeocodingProvider {
    client: Arc<dyn HttpClient>,
}

impl NominatimGeocodingProvider {
    pub fn new(client: Arc<dyn HttpClient>) -> Self {
        Self { client }
    }
}

impl GeocodingProvider for NominatimGeocodingProvider {
    fn search(&self, query: &str) -> Result<Vec<GeocodeResult>, ProviderError> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let mut url = parse_base_url(NOMINATIM_SEARCH_URL)?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("q", trimmed);
            pairs.append_pair("format", "jsonv2");
        }

        let response = self.client.execute(HttpRequest::get(url.as_str()))?;
        if !response.is_ok() {
            return Err(ProviderError::Request(format!(
                "Nominatim search failed: {}",
                response.status
            )));
        }

        let body = decode_json(&response)?;
        Ok(map_nominatim_results(&body))
    }

    fn reverse(&self, point: GeoPoint) -> Result<Option<String>, ProviderError> {
        let mut url = parse_base_url(NOMINATIM_REVERSE_URL)?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("lat", &point.lat.to_string());
            pairs.append_pair("lon", &point.lon.to_string());
            pairs.append_pair("format", "jsonv2");
        }

        let response = self.client.execute(HttpRequest::get(url.as_str()))?;
        if !response.is_ok() {
            return Err(ProviderError::Request(format!(
                "Nominatim reverse geocode failed: {}",
                response.status
            )));
        }

        let body = decode_json(&response)?;
        Ok(first_nominatim_label(&body))
    }
}

/// Shared response mapping for Nominatim and Nominatim-compatible search APIs
/// (e.g. LocationIQ), mirroring `nominatimCompatible.ts`. Entries that are not
/// shaped like a Nominatim result are filtered out instead of failing the call.
pub(crate) fn map_nominatim_results(body: &Value) -> Vec<GeocodeResult> {
    let Some(entries) = body.as_array() else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let label = entry.get("display_name")?.as_str()?;
            let lat = entry.get("lat")?.as_str()?;
            let lon = entry.get("lon")?.as_str()?;
            Some(GeocodeResult {
                label: label.to_owned(),
                point: GeoPoint {
                    lon: lon.parse().ok()?,
                    lat: lat.parse().ok()?,
                    altitude: None,
                    altitude_mode: None,
                },
            })
        })
        .collect()
}

/// `mapNominatimResults([body])[0]?.label ?? null`
pub(crate) fn first_nominatim_label(body: &Value) -> Option<String> {
    map_nominatim_results(&Value::Array(vec![body.clone()]))
        .into_iter()
        .next()
        .map(|result| result.label)
}

pub(crate) fn parse_base_url(base: &str) -> Result<url::Url, ProviderError> {
    url::Url::parse(base)
        .map_err(|error| ProviderError::Request(format!("invalid provider URL: {error}")))
}

pub(crate) fn decode_json(response: &HttpResponse) -> Result<Value, ProviderError> {
    serde_json::from_slice(&response.body).map_err(|error| ProviderError::Decode(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::FakeHttpClient;

    fn provider(client: &Arc<FakeHttpClient>) -> NominatimGeocodingProvider {
        NominatimGeocodingProvider::new(client.clone())
    }

    fn pairs(url: &url::Url) -> Vec<(String, String)> {
        url.query_pairs().into_owned().collect()
    }

    #[test]
    fn search_builds_the_nominatim_url_and_maps_results() {
        let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::ok(
            r#"[
                {"display_name": "Berlin, Germany", "lat": "52.52", "lon": "13.405", "type": "city"},
                {"display_name": "Broken", "lat": "nope", "lon": "13.4"},
                {"osm_id": 123}
            ]"#,
        )]));
        let results = provider(&client)
            .search("  Berlin  ")
            .expect("search succeeds");

        let url = url::Url::parse(&client.last_request().url).expect("valid url");
        assert_eq!(
            url.as_str(),
            "https://nominatim.openstreetmap.org/search?q=Berlin&format=jsonv2"
        );
        assert_eq!(
            pairs(&url),
            vec![
                ("q".to_owned(), "Berlin".to_owned()),
                ("format".to_owned(), "jsonv2".to_owned()),
            ]
        );
        assert_eq!(
            client.last_request().headers,
            vec![("Accept".to_owned(), "application/json".to_owned())]
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].label, "Berlin, Germany");
        assert_eq!(results[0].point.lat, 52.52);
        assert_eq!(results[0].point.lon, 13.405);
    }

    #[test]
    fn reverse_builds_the_reverse_url_and_returns_the_first_label() {
        let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::ok(
            r#"{"display_name": "Brandenburger Tor, Berlin, Germany", "lat": "52.5163", "lon": "13.3777"}"#,
        )]));
        let point = GeoPoint::new(13.3777, 52.5163).expect("valid point");
        let label = provider(&client).reverse(point).expect("reverse succeeds");

        let url = url::Url::parse(&client.last_request().url).expect("valid url");
        assert_eq!(
            url.as_str(),
            "https://nominatim.openstreetmap.org/reverse?lat=52.5163&lon=13.3777&format=jsonv2"
        );
        assert_eq!(label.as_deref(), Some("Brandenburger Tor, Berlin, Germany"));
    }

    #[test]
    fn reverse_returns_none_for_a_non_result_body() {
        let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::ok(
            r#"{"error": "Unable to geocode"}"#,
        )]));
        let point = GeoPoint::new(0.0, 0.0).expect("valid point");
        let label = provider(&client).reverse(point).expect("reverse succeeds");
        assert_eq!(label, None);
    }

    #[test]
    fn search_surfaces_request_errors() {
        let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::status(500, "")]));
        let error = provider(&client).search("Berlin").expect_err("must fail");
        assert!(
            matches!(error, ProviderError::Request(message) if message == "Nominatim search failed: 500")
        );
    }

    #[test]
    fn search_surfaces_decode_errors_for_invalid_json() {
        let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::ok("<html>")]));
        let error = provider(&client).search("Berlin").expect_err("must fail");
        assert!(matches!(error, ProviderError::Decode(_)));
    }

    #[test]
    fn a_malformed_body_maps_to_no_results() {
        let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::ok(r#"{"a": 1}"#)]));
        let results = provider(&client).search("Berlin").expect("search succeeds");
        assert!(results.is_empty());
    }

    #[test]
    fn an_empty_query_makes_no_request() {
        let client = Arc::new(FakeHttpClient::default());
        let provider = provider(&client);
        assert_eq!(provider.search("").expect("empty result"), Vec::new());
        assert_eq!(provider.search("   ").expect("empty result"), Vec::new());
        assert!(client.requests().is_empty());
    }
}
