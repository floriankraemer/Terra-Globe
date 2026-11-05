//! OpenRouteService Directions adapter, requires an API key.
//!
//! The key travels in the raw `Authorization` header (no `Bearer` prefix),
//! exactly as the web app sends it.

use std::sync::Arc;

use serde_json::Value;
use terra_core::GeoPoint;
use url::Url;

use super::{RouteLeg, RoutingProfile, RoutingProvider};
use crate::ProviderError;
use crate::http::{HttpClient, HttpRequest};

const ORS_DIRECTIONS_BASE_URL: &str = "https://api.openrouteservice.org/v2/directions";

fn profile_name(profile: RoutingProfile) -> &'static str {
    match profile {
        RoutingProfile::Car => "driving-car",
        RoutingProfile::Foot => "foot-walking",
        RoutingProfile::Bike => "cycling-regular",
    }
}

pub struct OpenRouteServiceRoutingProvider {
    api_key: String,
    client: Arc<dyn HttpClient>,
}

impl OpenRouteServiceRoutingProvider {
    pub fn new(api_key: String, client: Arc<dyn HttpClient>) -> Self {
        Self { api_key, client }
    }
}

impl RoutingProvider for OpenRouteServiceRoutingProvider {
    fn route(
        &self,
        waypoints: &[GeoPoint],
        profile: RoutingProfile,
    ) -> Result<Vec<RouteLeg>, ProviderError> {
        if waypoints.len() < 2 {
            return Ok(Vec::new());
        }

        let url = Url::parse(&format!(
            "{ORS_DIRECTIONS_BASE_URL}/{}/geojson",
            profile_name(profile)
        ))
        .map_err(|error| {
            ProviderError::Request(format!(
                "OpenRouteService routing failed: invalid request URL: {error}"
            ))
        })?;
        let body = serde_json::json!({
            "coordinates": waypoints
                .iter()
                .map(|point| serde_json::json!([point.lon, point.lat]))
                .collect::<Vec<_>>(),
            "alternative_routes": { "target_count": 3 },
        });
        let request = HttpRequest::post_json(url.as_str(), body.to_string())
            .header("Authorization", &self.api_key);

        let response = self.client.execute(request)?;
        if !response.is_ok() {
            return Err(ProviderError::Request(format!(
                "OpenRouteService routing failed: {}",
                response.status
            )));
        }

        let body: Value = serde_json::from_slice(&response.body).map_err(|error| {
            ProviderError::Decode(format!("OpenRouteService returned invalid JSON: {error}"))
        })?;
        decode_ors_features(&body)
    }
}

fn decode_ors_features(body: &Value) -> Result<Vec<RouteLeg>, ProviderError> {
    if !body.is_object() {
        return Err(ProviderError::Decode(
            "Malformed OpenRouteService response: expected a JSON object".to_string(),
        ));
    }
    let Some(Value::Array(features)) = body.get("features") else {
        return Err(ProviderError::Decode(
            "Malformed OpenRouteService response: expected a `features` array".to_string(),
        ));
    };

    let mut legs = Vec::with_capacity(features.len());
    let mut malformed = 0usize;
    for feature in features {
        match ors_feature_to_leg(feature) {
            Some(leg) => legs.push(leg),
            None => malformed += 1,
        }
    }
    if malformed > 0 {
        return Err(ProviderError::Decode(format!(
            "Malformed OpenRouteService response: {malformed} feature(s) missing required fields"
        )));
    }
    Ok(legs)
}

fn ors_feature_to_leg(feature: &Value) -> Option<RouteLeg> {
    let object = feature.as_object()?;
    let properties = object.get("properties")?;
    let summary = properties.get("summary")?;
    let distance_meters = summary.get("distance")?.as_f64()?;
    let duration_seconds = summary.get("duration")?.as_f64()?;
    let coordinates = object.get("geometry")?.get("coordinates")?.as_array()?;
    let path = coordinates
        .iter()
        .map(|coordinate| {
            let pair = coordinate.as_array()?;
            Some(GeoPoint {
                lon: pair.first()?.as_f64()?,
                lat: pair.get(1)?.as_f64()?,
                altitude: None,
                altitude_mode: None,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(RouteLeg {
        path,
        distance_meters,
        duration_seconds,
    })
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
            "type": "FeatureCollection",
            "features": [
                {
                    "type": "Feature",
                    "geometry": {
                        "type": "LineString",
                        "coordinates": [[13.4, 52.52], [13.3, 52.3], [13.2, 52.1]]
                    },
                    "properties": {
                        "summary": { "distance": 24000.5, "duration": 1800.25 }
                    }
                }
            ]
        })
        .to_string()
    }

    fn provider_with_response(
        body: HttpResponse,
    ) -> (OpenRouteServiceRoutingProvider, Arc<FakeHttpClient>) {
        let client = Arc::new(FakeHttpClient::new(vec![body]));
        let provider = OpenRouteServiceRoutingProvider::new(
            "ors-key".to_string(),
            Arc::clone(&client) as Arc<dyn HttpClient>,
        );
        (provider, client)
    }

    #[test]
    fn request_posts_coordinates_with_authorization_header() {
        let (provider, client) = provider_with_response(HttpResponse::ok(success_body()));

        provider
            .route(&waypoints(), RoutingProfile::Bike)
            .expect("route");

        let request = client.last_request();
        assert_eq!(request.method, HttpMethod::Post);
        assert_eq!(
            request.url,
            "https://api.openrouteservice.org/v2/directions/cycling-regular/geojson"
        );
        assert_eq!(
            request.headers,
            vec![
                ("Accept".to_string(), "application/json".to_string()),
                ("Content-Type".to_string(), "application/json".to_string()),
                ("Authorization".to_string(), "ors-key".to_string()),
            ]
        );
        let body: Value =
            serde_json::from_slice(request.body.as_deref().expect("post body")).expect("json body");
        assert_eq!(
            body,
            serde_json::json!({
                "coordinates": [[13.4, 52.52], [13.2, 52.1]],
                "alternative_routes": { "target_count": 3 }
            })
        );
    }

    #[test]
    fn success_body_maps_feature_summaries_to_route_legs() {
        let (provider, _client) = provider_with_response(HttpResponse::ok(success_body()));

        let legs = provider
            .route(&waypoints(), RoutingProfile::Car)
            .expect("route");

        assert_eq!(legs.len(), 1);
        assert_eq!(legs[0].distance_meters, 24000.5);
        assert_eq!(legs[0].duration_seconds, 1800.25);
        assert_eq!(legs[0].path.len(), 3);
        assert_eq!(legs[0].path[0].lon, 13.4);
        assert_eq!(legs[0].path[0].lat, 52.52);
        assert_eq!(legs[0].path[2].lon, 13.2);
        assert_eq!(legs[0].path[2].lat, 52.1);
    }

    #[test]
    fn malformed_feature_rejects_the_whole_response() {
        let body = serde_json::json!({
            "features": [
                {
                    "geometry": { "coordinates": [[13.4, 52.52]] },
                    "properties": { "summary": { "distance": 1.0, "duration": 2.0 } }
                },
                {
                    "geometry": { "coordinates": [[13.4, 52.52]] },
                    "properties": { "summary": { "distance": 1.0 } }
                }
            ]
        })
        .to_string();
        let (provider, _client) = provider_with_response(HttpResponse::ok(body));

        let error = provider
            .route(&waypoints(), RoutingProfile::Car)
            .expect_err("malformed body");

        assert!(
            matches!(error, ProviderError::Decode(message) if message.contains("1 feature(s) missing required fields"))
        );
    }

    #[test]
    fn fewer_than_two_waypoints_makes_no_request() {
        let client = Arc::new(FakeHttpClient::default());
        let provider = OpenRouteServiceRoutingProvider::new(
            "ors-key".to_string(),
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
        let (provider, _client) = provider_with_response(HttpResponse::status(429, "{}"));

        let error = provider
            .route(&waypoints(), RoutingProfile::Car)
            .expect_err("non-2xx");

        assert!(
            matches!(error, ProviderError::Request(message) if message == "OpenRouteService routing failed: 429")
        );
    }
}
