//! GraphHopper Directions API adapter, requires an API key.

use std::sync::Arc;

use serde_json::Value;
use terra_core::GeoPoint;
use url::Url;

use super::{RouteLeg, RoutingProfile, RoutingProvider};
use crate::ProviderError;
use crate::http::{HttpClient, HttpRequest};

const GRAPHHOPPER_ROUTE_URL: &str = "https://graphhopper.com/api/1/route";

fn profile_name(profile: RoutingProfile) -> &'static str {
    match profile {
        RoutingProfile::Car => "car",
        RoutingProfile::Foot => "foot",
        RoutingProfile::Bike => "bike",
    }
}

pub struct GraphHopperRoutingProvider {
    api_key: String,
    client: Arc<dyn HttpClient>,
}

impl GraphHopperRoutingProvider {
    pub fn new(api_key: String, client: Arc<dyn HttpClient>) -> Self {
        Self { api_key, client }
    }
}

impl RoutingProvider for GraphHopperRoutingProvider {
    fn route(
        &self,
        waypoints: &[GeoPoint],
        profile: RoutingProfile,
    ) -> Result<Vec<RouteLeg>, ProviderError> {
        if waypoints.len() < 2 {
            return Ok(Vec::new());
        }

        let mut url = Url::parse(GRAPHHOPPER_ROUTE_URL).map_err(|error| {
            ProviderError::Request(format!(
                "GraphHopper routing failed: invalid request URL: {error}"
            ))
        })?;
        {
            let mut query = url.query_pairs_mut();
            for point in waypoints {
                query.append_pair("point", &format!("{},{}", point.lat, point.lon));
            }
            query.append_pair("profile", profile_name(profile));
            query.append_pair("points_encoded", "false");
            query.append_pair("algorithm", "alternative_route");
            query.append_pair("alternative_route.max_paths", "3");
            query.append_pair("key", &self.api_key);
        }

        let response = self.client.execute(HttpRequest::get(url.as_str()))?;
        if !response.is_ok() {
            return Err(ProviderError::Request(format!(
                "GraphHopper routing failed: {}",
                response.status
            )));
        }

        let body: Value = serde_json::from_slice(&response.body).map_err(|error| {
            ProviderError::Decode(format!("GraphHopper returned invalid JSON: {error}"))
        })?;
        decode_graphhopper_paths(&body)
    }
}

fn decode_graphhopper_paths(body: &Value) -> Result<Vec<RouteLeg>, ProviderError> {
    if !body.is_object() {
        return Err(ProviderError::Decode(
            "Malformed GraphHopper response: expected a JSON object".to_string(),
        ));
    }
    let Some(Value::Array(paths)) = body.get("paths") else {
        return Err(ProviderError::Decode(
            "Malformed GraphHopper response: expected a `paths` array".to_string(),
        ));
    };

    let mut legs = Vec::with_capacity(paths.len());
    let mut malformed = 0usize;
    for path in paths {
        match graphhopper_path_to_leg(path) {
            Some(leg) => legs.push(leg),
            None => malformed += 1,
        }
    }
    if malformed > 0 {
        return Err(ProviderError::Decode(format!(
            "Malformed GraphHopper response: {malformed} path(s) missing required fields"
        )));
    }
    Ok(legs)
}

fn graphhopper_path_to_leg(path: &Value) -> Option<RouteLeg> {
    let object = path.as_object()?;
    let distance_meters = object.get("distance")?.as_f64()?;
    // GraphHopper reports `time` in milliseconds; RouteLeg is in seconds.
    let duration_seconds = object.get("time")?.as_f64()? / 1000.0;
    let coordinates = object.get("points")?.get("coordinates")?.as_array()?;
    let path_points = coordinates
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
        path: path_points,
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
                lon: 9.99,
                lat: 53.55,
                altitude: None,
                altitude_mode: None,
            },
            GeoPoint {
                lon: 10.0,
                lat: 53.56,
                altitude: None,
                altitude_mode: None,
            },
        ]
    }

    fn success_body() -> String {
        serde_json::json!({
            "paths": [
                {
                    "points": {
                        "coordinates": [[13.4, 52.52], [11.7, 53.0], [9.99, 53.55]]
                    },
                    "distance": 289000.25,
                    "time": 10_803_500.0
                },
                {
                    "points": {
                        "coordinates": [[13.4, 52.52], [9.99, 53.55]]
                    },
                    "distance": 295000.0,
                    "time": 11_100_000.0
                }
            ]
        })
        .to_string()
    }

    fn provider_with_response(
        body: HttpResponse,
    ) -> (GraphHopperRoutingProvider, Arc<FakeHttpClient>) {
        let client = Arc::new(FakeHttpClient::new(vec![body]));
        let provider = GraphHopperRoutingProvider::new(
            "gh-key".to_string(),
            Arc::clone(&client) as Arc<dyn HttpClient>,
        );
        (provider, client)
    }

    #[test]
    fn request_appoints_waypoints_in_order_before_the_fixed_parameters() {
        let (provider, client) = provider_with_response(HttpResponse::ok(success_body()));

        provider
            .route(&waypoints(), RoutingProfile::Foot)
            .expect("route");

        let request = client.last_request();
        assert_eq!(request.method, HttpMethod::Get);
        assert_eq!(
            request.headers,
            vec![("Accept".to_string(), "application/json".to_string())]
        );
        assert!(request.body.is_none());

        let url = Url::parse(&request.url).expect("recorded url");
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("graphhopper.com"));
        assert_eq!(url.path(), "/api/1/route");
        let pairs: Vec<(String, String)> = url
            .query_pairs()
            .map(|(name, value)| (name.into_owned(), value.into_owned()))
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("point".to_string(), "52.52,13.4".to_string()),
                ("point".to_string(), "53.55,9.99".to_string()),
                ("point".to_string(), "53.56,10".to_string()),
                ("profile".to_string(), "foot".to_string()),
                ("points_encoded".to_string(), "false".to_string()),
                ("algorithm".to_string(), "alternative_route".to_string()),
                ("alternative_route.max_paths".to_string(), "3".to_string()),
                ("key".to_string(), "gh-key".to_string()),
            ]
        );
    }

    #[test]
    fn success_body_maps_time_from_milliseconds_to_seconds() {
        let (provider, _client) = provider_with_response(HttpResponse::ok(success_body()));

        let legs = provider
            .route(&waypoints(), RoutingProfile::Car)
            .expect("route");

        assert_eq!(legs.len(), 2);
        assert_eq!(legs[0].distance_meters, 289000.25);
        assert_eq!(legs[0].duration_seconds, 10803.5);
        assert_eq!(legs[0].path.len(), 3);
        assert_eq!(legs[0].path[0].lon, 13.4);
        assert_eq!(legs[0].path[0].lat, 52.52);
        assert_eq!(legs[0].path[2].lon, 9.99);
        assert_eq!(legs[0].path[2].lat, 53.55);
        assert_eq!(legs[1].distance_meters, 295000.0);
        assert_eq!(legs[1].duration_seconds, 11100.0);
    }

    #[test]
    fn malformed_path_rejects_the_whole_response() {
        let body = serde_json::json!({
            "paths": [
                { "points": { "coordinates": [[13.4, 52.52]] }, "distance": 1.0, "time": 2.0 },
                { "points": { "coordinates": [[13.4, 52.52]] }, "distance": 1.0 }
            ]
        })
        .to_string();
        let (provider, _client) = provider_with_response(HttpResponse::ok(body));

        let error = provider
            .route(&waypoints(), RoutingProfile::Car)
            .expect_err("malformed body");

        assert!(
            matches!(error, ProviderError::Decode(message) if message.contains("1 path(s) missing required fields"))
        );
    }

    #[test]
    fn fewer_than_two_waypoints_makes_no_request() {
        let client = Arc::new(FakeHttpClient::default());
        let provider = GraphHopperRoutingProvider::new(
            "gh-key".to_string(),
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
            matches!(error, ProviderError::Request(message) if message == "GraphHopper routing failed: 401")
        );
    }
}
