//! Mapbox Geocoding v5 adapter, requires an API key (access token).

use std::fmt::Write as _;
use std::sync::Arc;

use serde_json::Value;
use terra_core::GeoPoint;

use super::nominatim::{decode_json, parse_base_url};
use super::{GeocodeResult, GeocodingProvider};
use crate::ProviderError;
use crate::http::{HttpClient, HttpRequest};

const MAPBOX_GEOCODING_BASE_URL: &str = "https://api.mapbox.com/geocoding/v5/mapbox.places";

pub struct MapboxGeocodingProvider {
    api_key: String,
    client: Arc<dyn HttpClient>,
}

impl MapboxGeocodingProvider {
    pub fn new(api_key: String, client: Arc<dyn HttpClient>) -> Self {
        Self { api_key, client }
    }
}

impl GeocodingProvider for MapboxGeocodingProvider {
    fn search(&self, query: &str) -> Result<Vec<GeocodeResult>, ProviderError> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let mut url = parse_base_url(&format!(
            "{MAPBOX_GEOCODING_BASE_URL}/{}.json",
            encode_uri_component(trimmed)
        ))?;
        url.query_pairs_mut()
            .append_pair("access_token", &self.api_key);

        let response = self.client.execute(HttpRequest::get(url.as_str()))?;
        if !response.is_ok() {
            return Err(ProviderError::Request(format!(
                "Mapbox geocoding search failed: {}",
                response.status
            )));
        }

        let body = decode_json(&response)?;
        Ok(map_mapbox_results(&body))
    }

    fn reverse(&self, point: GeoPoint) -> Result<Option<String>, ProviderError> {
        let mut url = parse_base_url(&format!(
            "{MAPBOX_GEOCODING_BASE_URL}/{},{}.json",
            point.lon, point.lat
        ))?;
        url.query_pairs_mut()
            .append_pair("access_token", &self.api_key);

        let response = self.client.execute(HttpRequest::get(url.as_str()))?;
        if !response.is_ok() {
            return Err(ProviderError::Request(format!(
                "Mapbox reverse geocode failed: {}",
                response.status
            )));
        }

        let body = decode_json(&response)?;
        Ok(map_mapbox_results(&body)
            .into_iter()
            .next()
            .map(|result| result.label))
    }
}

/// Percent-encodes every byte outside the `encodeURIComponent` unreserved set
/// (`A-Za-z0-9-_.!~*'()`), UTF-8 bytes included, exactly like the web app.
fn encode_uri_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => encoded.push(*byte as char),
            other => {
                let _ = write!(encoded, "%{other:02X}");
            }
        }
    }
    encoded
}

/// `mapMapboxResults` from `MapboxGeocodingProvider.ts`: entries that are not
/// shaped like a Mapbox feature are filtered out instead of failing the call.
fn map_mapbox_results(body: &Value) -> Vec<GeocodeResult> {
    let Some(features) = body.get("features").and_then(Value::as_array) else {
        return Vec::new();
    };
    features
        .iter()
        .filter_map(|feature| {
            let label = feature.get("place_name")?.as_str()?;
            let center = feature.get("center")?.as_array()?;
            let lon = center.first()?.as_f64()?;
            let lat = center.get(1)?.as_f64()?;
            Some(GeocodeResult {
                label: label.to_owned(),
                point: GeoPoint {
                    lon,
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

    fn provider(client: &Arc<FakeHttpClient>) -> MapboxGeocodingProvider {
        MapboxGeocodingProvider::new("mb-token".to_owned(), client.clone())
    }

    fn pairs(url: &url::Url) -> Vec<(String, String)> {
        url.query_pairs().into_owned().collect()
    }

    #[test]
    fn search_percent_encodes_the_query_like_encodeuricomponent() {
        let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::ok(
            r#"{"features": [
                {"place_name": "Brandenburger Tor, Berlin", "center": [13.3777, 52.5163], "type": "Feature"},
                {"place_name": "Broken", "center": [1]},
                {"place_name": "No center"}
            ]}"#,
        )]));
        let results = provider(&client)
            .search("  Brandenburger Tor & Torstraße ")
            .expect("search succeeds");

        let url = url::Url::parse(&client.last_request().url).expect("valid url");
        assert_eq!(
            url.as_str(),
            "https://api.mapbox.com/geocoding/v5/mapbox.places/Brandenburger%20Tor%20%26%20Torstra%C3%9Fe.json?access_token=mb-token"
        );
        assert_eq!(
            pairs(&url),
            vec![("access_token".to_owned(), "mb-token".to_owned())]
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].label, "Brandenburger Tor, Berlin");
        assert_eq!(results[0].point.lon, 13.3777);
        assert_eq!(results[0].point.lat, 52.5163);
    }

    #[test]
    fn reverse_builds_the_reverse_url_and_returns_the_first_label() {
        let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::ok(
            r#"{"features": [{"place_name": "Berlin, Germany", "center": [13.4, 52.52]}]}"#,
        )]));
        let point = GeoPoint::new(13.4, 52.52).expect("valid point");
        let label = provider(&client).reverse(point).expect("reverse succeeds");

        let url = url::Url::parse(&client.last_request().url).expect("valid url");
        assert_eq!(
            url.as_str(),
            "https://api.mapbox.com/geocoding/v5/mapbox.places/13.4,52.52.json?access_token=mb-token"
        );
        assert_eq!(label.as_deref(), Some("Berlin, Germany"));
    }

    #[test]
    fn reverse_returns_none_when_there_are_no_features() {
        let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::ok(
            r#"{"features": []}"#,
        )]));
        let point = GeoPoint::new(0.0, 0.0).expect("valid point");
        let label = provider(&client).reverse(point).expect("reverse succeeds");
        assert_eq!(label, None);
    }

    #[test]
    fn search_surfaces_request_errors() {
        let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::status(401, "")]));
        let error = provider(&client).search("Berlin").expect_err("must fail");
        assert!(
            matches!(error, ProviderError::Request(message) if message == "Mapbox geocoding search failed: 401")
        );
    }

    #[test]
    fn reverse_surfaces_request_errors_with_the_reverse_wording() {
        let client = Arc::new(FakeHttpClient::new(vec![HttpResponse::status(403, "")]));
        let point = GeoPoint::new(0.0, 0.0).expect("valid point");
        let error = provider(&client).reverse(point).expect_err("must fail");
        assert!(
            matches!(error, ProviderError::Request(message) if message == "Mapbox reverse geocode failed: 403")
        );
    }

    #[test]
    fn a_malformed_body_maps_to_no_results() {
        for body in [r#"[]"#, r#"null"#, r#"{"features": "nope"}"#, r#"42"#] {
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
        assert_eq!(provider.search("  ").expect("empty result"), Vec::new());
        assert!(client.requests().is_empty());
    }
}
