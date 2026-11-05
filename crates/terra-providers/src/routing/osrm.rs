//! OSRM routing adapter.
//!
//! FOSSGIS/OpenStreetMap Germany runs one OSRM instance per profile
//! (`routed-car`/`routed-foot`/`routed-bike`); a single-host OSRM such as
//! router.project-osrm.org ignores the profile segment and always serves the
//! driving graph, so the host has to change with the profile for foot/bike
//! routing to be correct.

use std::sync::Arc;

use serde_json::Value;
use terra_core::GeoPoint;
use url::Url;

use super::{RouteLeg, RoutingProfile, RoutingProvider};
use crate::ProviderError;
use crate::http::{HttpClient, HttpRequest};

fn osm_de_host(profile: RoutingProfile) -> &'static str {
    match profile {
        RoutingProfile::Car => "https://routing.openstreetmap.de/routed-car",
        RoutingProfile::Foot => "https://routing.openstreetmap.de/routed-foot",
        RoutingProfile::Bike => "https://routing.openstreetmap.de/routed-bike",
    }
}

fn profile_segment(profile: RoutingProfile) -> &'static str {
    match profile {
        RoutingProfile::Car => "driving",
        RoutingProfile::Foot => "walking",
        RoutingProfile::Bike => "cycling",
    }
}

pub struct OsrmRoutingProvider {
    client: Arc<dyn HttpClient>,
}

impl OsrmRoutingProvider {
    pub fn new(client: Arc<dyn HttpClient>) -> Self {
        Self { client }
    }
}

impl RoutingProvider for OsrmRoutingProvider {
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
            "{}/route/v1/{}/{}",
            osm_de_host(profile),
            profile_segment(profile),
            coordinates
        ))
        .map_err(|error| {
            ProviderError::Request(format!("OSRM routing failed: invalid request URL: {error}"))
        })?;
        for (name, value) in [
            ("alternatives", "true"),
            ("geometries", "geojson"),
            ("overview", "full"),
        ] {
            url.query_pairs_mut().append_pair(name, value);
        }

        let response = self.client.execute(HttpRequest::get(url.as_str()))?;
        if !response.is_ok() {
            return Err(ProviderError::Request(format!(
                "OSRM routing failed: {}",
                response.status
            )));
        }

        let body: Value = serde_json::from_slice(&response.body).map_err(|error| {
            ProviderError::Decode(format!("OSRM returned invalid JSON: {error}"))
        })?;
        decode_osrm_routes(&body)
    }
}

/// Shared decoder for OSRM and Mapbox Directions: both are OSRM-derived APIs
/// with the same `{ routes: [{ geometry: { coordinates }, distance, duration }] }`
/// shape (the port of `mapOsrmCompatibleResults` in the web app).
pub(super) fn decode_osrm_routes(body: &Value) -> Result<Vec<RouteLeg>, ProviderError> {
    if !body.is_object() {
        return Err(ProviderError::Decode(
            "Malformed routing response: expected a JSON object".to_string(),
        ));
    }
    let Some(Value::Array(routes)) = body.get("routes") else {
        return Err(ProviderError::Decode(
            "Malformed routing response: expected a `routes` array".to_string(),
        ));
    };

    let mut legs = Vec::with_capacity(routes.len());
    let mut malformed = 0usize;
    for route in routes {
        match osrm_route_to_leg(route) {
            Some(leg) => legs.push(leg),
            None => malformed += 1,
        }
    }
    if malformed > 0 {
        return Err(ProviderError::Decode(format!(
            "Malformed routing response: {malformed} route(s) missing required fields"
        )));
    }
    Ok(legs)
}

fn osrm_route_to_leg(route: &Value) -> Option<RouteLeg> {
    let object = route.as_object()?;
    let distance_meters = object.get("distance")?.as_f64()?;
    let duration_seconds = object.get("duration")?.as_f64()?;
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
            "code": "Ok",
            "routes": [
                {
                    "geometry": {
                        "coordinates": [[13.4, 52.52], [13.3, 52.3], [13.2, 52.1]]
                    },
                    "distance": 24000.5,
                    "duration": 1800.25
                },
                {
                    "geometry": {
                        "coordinates": [[13.4, 52.52], [13.25, 52.4], [13.2, 52.1]]
                    },
                    "distance": 26000.0,
                    "duration": 2000.0
                }
            ]
        })
        .to_string()
    }

    fn provider_with_response(body: HttpResponse) -> (OsrmRoutingProvider, Arc<FakeHttpClient>) {
        let client = Arc::new(FakeHttpClient::new(vec![body]));
        let provider = OsrmRoutingProvider::new(Arc::clone(&client) as Arc<dyn HttpClient>);
        (provider, client)
    }

    #[test]
    fn request_targets_the_profile_host_with_osrm_query_parameters() {
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
        assert_eq!(
            request.url,
            "https://routing.openstreetmap.de/routed-foot/route/v1/walking/13.4,52.52;13.2,52.1\
             ?alternatives=true&geometries=geojson&overview=full"
        );
        assert!(Url::parse(&request.url).is_ok());
    }

    #[test]
    fn each_profile_uses_its_own_host_and_url_segment() {
        let cases = [
            (
                RoutingProfile::Car,
                "https://routing.openstreetmap.de/routed-car/route/v1/driving/",
            ),
            (
                RoutingProfile::Foot,
                "https://routing.openstreetmap.de/routed-foot/route/v1/walking/",
            ),
            (
                RoutingProfile::Bike,
                "https://routing.openstreetmap.de/routed-bike/route/v1/cycling/",
            ),
        ];
        for (profile, prefix) in cases {
            let (provider, client) = provider_with_response(HttpResponse::ok(success_body()));
            provider.route(&waypoints(), profile).expect("route");
            assert!(
                client.last_request().url.starts_with(prefix),
                "unexpected URL for {profile:?}: {}",
                client.last_request().url
            );
        }
    }

    #[test]
    fn success_body_maps_to_route_legs() {
        let (provider, _client) = provider_with_response(HttpResponse::ok(success_body()));

        let legs = provider
            .route(&waypoints(), RoutingProfile::Car)
            .expect("route");

        assert_eq!(legs.len(), 2);
        assert_eq!(legs[0].distance_meters, 24000.5);
        assert_eq!(legs[0].duration_seconds, 1800.25);
        assert_eq!(legs[0].path.len(), 3);
        assert_eq!(legs[0].path[0].lon, 13.4);
        assert_eq!(legs[0].path[0].lat, 52.52);
        assert_eq!(legs[0].path[2].lon, 13.2);
        assert_eq!(legs[0].path[2].lat, 52.1);
        assert_eq!(legs[1].distance_meters, 26000.0);
        assert_eq!(legs[1].duration_seconds, 2000.0);
    }

    #[test]
    fn malformed_route_rejects_the_whole_response() {
        let body = serde_json::json!({
            "routes": [
                { "geometry": { "coordinates": [[13.4, 52.52]] }, "distance": 1.0, "duration": 2.0 },
                { "geometry": { "coordinates": [[13.4, 52.52]] }, "distance": "many" }
            ]
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
        let provider = OsrmRoutingProvider::new(Arc::clone(&client) as Arc<dyn HttpClient>);

        let legs = provider
            .route(&waypoints()[..1], RoutingProfile::Car)
            .expect("no-op route");

        assert!(legs.is_empty());
        assert!(client.requests().is_empty());
    }

    #[test]
    fn http_error_status_surfaces_as_request_error() {
        let (provider, _client) = provider_with_response(HttpResponse::status(503, "unavailable"));

        let error = provider
            .route(&waypoints(), RoutingProfile::Car)
            .expect_err("non-2xx");

        assert!(
            matches!(error, ProviderError::Request(message) if message == "OSRM routing failed: 503")
        );
    }
}
