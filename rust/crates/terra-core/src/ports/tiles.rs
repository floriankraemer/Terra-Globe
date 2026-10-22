use serde::{Deserialize, Serialize};

/// An XYZ raster tile source (`{z}/{x}/{y}`, optional `{s}` subdomain, optional `{api_key}`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TileSource {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subdomains: Option<Vec<String>>,
    pub credit: String,
    pub maximum_level: u8,
}

impl TileSource {
    pub fn open_street_map() -> Self {
        Self {
            id: "osm".into(),
            name: "OpenStreetMap".into(),
            url: "https://tile.openstreetmap.org/{z}/{x}/{y}.png".into(),
            subdomains: None,
            credit: "© OpenStreetMap contributors".into(),
            maximum_level: 19,
        }
    }

    pub fn open_topo_map() -> Self {
        Self {
            id: "opentopomap".into(),
            name: "OpenTopoMap".into(),
            url: "https://{s}.tile.opentopomap.org/{z}/{x}/{y}.png".into(),
            subdomains: Some(vec!["a".into(), "b".into(), "c".into()]),
            credit: "© OpenStreetMap contributors, SRTM | © OpenTopoMap (CC-BY-SA)".into(),
            maximum_level: 17,
        }
    }

    pub fn builtin() -> Vec<Self> {
        vec![Self::open_street_map(), Self::open_topo_map()]
    }

    /// Expands the URL template for one tile. Subdomains rotate by `x + y`.
    pub fn tile_url(&self, z: u8, x: u32, y: u32) -> String {
        let mut url = self
            .url
            .replace("{z}", &z.to_string())
            .replace("{x}", &x.to_string())
            .replace("{y}", &y.to_string());
        if let Some(subs) = self.subdomains.as_ref().filter(|s| !s.is_empty()) {
            let idx = ((x as u64 + y as u64) % subs.len() as u64) as usize;
            url = url.replace("{s}", &subs[idx]);
        }
        url
    }
}
