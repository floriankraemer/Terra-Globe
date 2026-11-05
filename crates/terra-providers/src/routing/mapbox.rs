//! Mapbox Directions v5 adapter, requires an API key (access token).

use std::sync::Arc;

use serde_json::Value;
use terra_core::GeoPoint;
use url::Url;

use super::osrm::decode_osrm_routes;
use super::{RouteLeg, RoutingProfile, RoutingProvider};
use crate::ProviderError;
use crate::http::{HttpClient, HttpRequest};

const MAPBOX_DIRECTIONS_BASE_URL: &str = "https://api.mapbox.com/directions/v5/mapbox";

fn profile_segment(profile: RoutingProfile) -> &'static str {
    match profile {
        RoutingProfile::Car => "driving",
        RoutingProfile::Foot => "walking",
        RoutingProfile::Bike => "cycling",
    }
}

pub struct MapboxRoutingProvider {
    api_key: String,
    client: Arc<dyn HttpClient>,
}

impl MapboxRoutingProvider {
    pub fn new(api_key: String, client: Arc<dyn HttpClient>) -> Self {
        Self { api_key, client }
    }
}

impl RoutingProvider for MapboxRoutingProvider {
    fn route(
        &self,
        waypoints: &[GeoPoint],
        profile: RoutingProfile,
    ) -> Result<Vec<RouteLeg>, ProviderError> {
        if waypoints.len() < 2 {
            return Ok(Vec::new());
        }

        let coordinates = waypoints
            .iter()
            .map(|point| format!("{},{}", point.lon, point.lat))
            .collect::<Vec<_>>()
            .join(";");
        let mut url = Url::parse(&format!(
            "{MAPBOX_DIRECTIONS_BASE_URL}/{}/{coordinates}",
            profile_segment(profile)
        ))
        .map_err(|error| {
            ProviderError::Request(format!(
                "Mapbox routing failed: invalid request URL: {error}"
            ))
        })?;
        for (name, value) in [
            ("alternatives", "true"),
            ("geometries", "geojson"),
            ("overview", "full"),
            ("access_token", self.api_key.as_str()),
        ] {
            url.query_pairs_mut().append_pair(name, value);
        }

        let response = self.client.execute(HttpRequest::get(url.as_str()))?;
        if !response.is_ok() {
            return Err(ProviderError::Request(format!(
                "Mapbox routing failed: {}",
                response.status
            )));
        }

        let body: Value = serde_json::from_slice(&response.body).map_err(|error| {
            ProviderError::Decode(format!("Mapbox returned invalid JSON: {error}"))
        })?;
        decode_osrm_routes(&body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{FakeHttpClient, HttpMethod, HttpResponse};

    fn waypoints() -> Vec<GeoPoint> {
        vec![
            GeoPoint {
                lon: 13.4,
                lat: 52.52,
                altitude: None,
                altitude_mode: None,
            },
            GeoPoint {
                lon: 13.2,
                lat: 52.1,
                altitude: None,
                altitude_mode: None,
            },
        ]
    }

    fn success_body() -> String {
        serde_json::json!({
            "routes": [
                {
                    "geometry": {
                        "coordinates": [[13.4, 52.52], [13.3, 52.3], [13.2, 52.1]]
                    },
                    "distance": 24000.5,
                    "duration": 1800.25
                }
            ]
        })
        .to_string()
    }

    fn provider_with_response(body: HttpResponse) -> (MapboxRoutingProvider, Arc<FakeHttpClient>) {
        let client = Arc::new(FakeHttpClient::new(vec![body]));
        let provider = MapboxRoutingProvider::new(
            "mapbox-token".to_string(),
            Arc::clone(&client) as Arc<dyn HttpClient>,
        );
        (provider, client)
    }

    #[test]
    fn request_carries_query_parameters_and_access_token() {
        let (provider, client) = provider_with_response(HttpResponse::ok(success_body()));

        provider
            .route(&waypoints(), RoutingProfile::Bike)
            .expect("route");

        let request = client.last_request();
        assert_eq!(request.method, HttpMethod::Get);
        assert_eq!(
            request.headers,
            vec![("Accept".to_string(), "application/json".to_string())]
        );
        assert_eq!(
            request.url,
            "https://api.mapbox.com/directions/v5/mapbox/cycling/13.4,52.52;13.2,52.1\
             ?alternatives=true&geometries=geojson&overview=full&access_token=mapbox-token"
        );
    }

    #[test]
    fn success_body_maps_to_route_legs() {
        let (provider, _client) = provider_with_response(HttpResponse::ok(success_body()));

        let legs = provider
            .route(&waypoints(), RoutingProfile::Car)
            .expect("route");

        assert_eq!(legs.len(), 1);
        assert_eq!(legs[0].distance_meters, 24000.5);
        assert_eq!(legs[0].duration_seconds, 1800.25);
        assert_eq!(legs[0].path.len(), 3);
        assert_eq!(legs[0].path[0].lon, 13.4);
        assert_eq!(legs[0].path[2].lat, 52.1);
    }

    #[test]
    fn malformed_route_rejects_the_whole_response() {
        let body = serde_json::json!({
            "routes": [{ "geometry": { "coordinates": "not-an-array" }, "distance": 1.0, "duration": 2.0 }]
        })
        .to_string();
        let (provider, _client) = provider_with_response(HttpResponse::ok(body));

        let error = provider
            .route(&waypoints(), RoutingProfile::Car)
            .expect_err("malformed body");

        assert!(
            matches!(error, ProviderError::Decode(message) if message.contains("1 route(s) missing required fields"))
        );
    }

    #[test]
    fn fewer_than_two_waypoints_makes_no_request() {
        let client = Arc::new(FakeHttpClient::default());
        let provider = MapboxRoutingProvider::new(
            "mapbox-token".to_string(),
            Arc::clone(&client) as Arc<dyn HttpClient>,
        );

        let legs = provider
            .route(&waypoints()[..1], RoutingProfile::Car)
            .expect("no-op route");

        assert!(legs.is_empty());
        assert!(client.requests().is_empty());
    }

    #[test]
    fn http_error_status_surfaces_as_request_error() {
        let (provider, _client) = provider_with_response(HttpResponse::status(401, "{}"));

        let error = provider
            .route(&waypoints(), RoutingProfile::Car)
            .expect_err("non-2xx");

        assert!(
            matches!(error, ProviderError::Request(message) if message == "Mapbox routing failed: 401")
        );
    }
}
