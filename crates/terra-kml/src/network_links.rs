//! One-time resolution of `<NetworkLink>` references.
//!
//! Mirrors `packages/core/src/kml/networkLink.ts`: fetch each href once,
//! inline the result into the folder that held the link, cap recursion, and
//! record failures as warnings instead of failing the import.

use std::collections::HashSet;

use crate::parse::{KmlNetworkLinkRef, ParseKmlResult, parse_kml};

pub const DEFAULT_MAX_DEPTH: usize = 5;

pub trait KmlFetcher {
    fn fetch_text(&self, url: &str) -> Result<String, String>;
}

/// Resolves every NetworkLink found by the parser and returns the merged result
/// with `network_links` empty.
pub fn resolve_network_links(
    result: ParseKmlResult,
    fetcher: &dyn KmlFetcher,
    max_depth: usize,
) -> ParseKmlResult {
    let mut folders = result.folders;
    let mut placemarks = result.placemarks;
    let mut styles = result.styles;
    let mut screen_overlays = result.screen_overlays;
    let mut warnings = result.warnings;
    let mut visited: HashSet<String> = HashSet::new();

    let mut pending: Vec<KmlNetworkLinkRef> = result.network_links;
    let mut depth = 0;

    while !pending.is_empty() && depth < max_depth {
        let mut next_pending = Vec::new();

        for link in pending {
            if !visited.insert(link.href.clone()) {
                warnings.push(format!(
                    "Skipped NetworkLink \"{}\": already resolved (cycle).",
                    link.href
                ));
                continue;
            }

            match fetcher.fetch_text(&link.href) {
                Ok(xml) => match parse_kml(&xml) {
                    Ok(child) => {
                        folders.extend(child.folders.into_iter().map(|mut folder| {
                            if folder.parent_id.is_none() {
                                folder.parent_id = link.folder_id.clone();
                            }
                            folder
                        }));
                        placemarks.extend(child.placemarks.into_iter().map(|mut placemark| {
                            if placemark.folder_id.is_none() {
                                placemark.folder_id = link.folder_id.clone();
                            }
                            placemark
                        }));
                        styles.extend(child.styles);
                        screen_overlays.extend(child.screen_overlays.into_iter().map(
                            |mut overlay| {
                                if overlay.folder_id.is_none() {
                                    overlay.folder_id = link.folder_id.clone();
                                }
                                overlay
                            },
                        ));
                        warnings.extend(child.warnings);
                        next_pending.extend(child.network_links.into_iter().map(
                            |mut network_link| {
                                if network_link.folder_id.is_none() {
                                    network_link.folder_id = link.folder_id.clone();
                                }
                                network_link
                            },
                        ));
                    }
                    Err(error) => warnings.push(format!(
                        "Failed to resolve NetworkLink \"{}\": {error}",
                        link.href
                    )),
                },
                Err(error) => warnings.push(format!(
                    "Failed to resolve NetworkLink \"{}\": {error}",
                    link.href
                )),
            }
        }

        pending = next_pending;
        depth += 1;
    }

    if !pending.is_empty() {
        warnings.push(format!(
            "Stopped resolving NetworkLinks after reaching max depth ({max_depth})."
        ));
    }

    ParseKmlResult {
        folders,
        placemarks,
        styles,
        screen_overlays,
        network_links: Vec::new(),
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeFetcher {
        responses: std::collections::HashMap<String, String>,
    }

    impl KmlFetcher for FakeFetcher {
        fn fetch_text(&self, url: &str) -> Result<String, String> {
            self.responses
                .get(url)
                .cloned()
                .ok_or_else(|| format!("no route for {url}"))
        }
    }

    fn link(href: &str) -> KmlNetworkLinkRef {
        KmlNetworkLinkRef {
            folder_id: Some("folder-1".to_string()),
            href: href.to_string(),
        }
    }

    #[test]
    fn inlines_child_documents_into_the_link_folder() {
        let xml = r#"<kml><Document><Placemark id="child"><name>Child</name>
            <Point><coordinates>1,2</coordinates></Point></Placemark></Document></kml>"#;
        let mut responses = std::collections::HashMap::new();
        responses.insert(
            "https://example.test/child.kml".to_string(),
            xml.to_string(),
        );
        let fetcher = FakeFetcher { responses };

        let resolved = resolve_network_links(
            ParseKmlResult {
                network_links: vec![link("https://example.test/child.kml")],
                ..Default::default()
            },
            &fetcher,
            DEFAULT_MAX_DEPTH,
        );

        assert_eq!(resolved.placemarks.len(), 1);
        assert_eq!(
            resolved.placemarks[0].folder_id.as_deref(),
            Some("folder-1")
        );
        assert!(resolved.network_links.is_empty());
        assert!(resolved.warnings.is_empty());
    }

    #[test]
    fn cycles_and_failures_become_warnings() {
        let mut responses = std::collections::HashMap::new();
        responses.insert(
            "https://example.test/a.kml".to_string(),
            r#"<kml><Document><NetworkLink><Link><href>https://example.test/a.kml</href></Link></NetworkLink></Document></kml>"#
                .to_string(),
        );
        let fetcher = FakeFetcher { responses };

        let resolved = resolve_network_links(
            ParseKmlResult {
                network_links: vec![link("https://example.test/a.kml")],
                ..Default::default()
            },
            &fetcher,
            DEFAULT_MAX_DEPTH,
        );
        assert_eq!(resolved.warnings.len(), 1);
        assert!(resolved.warnings[0].contains("already resolved"));

        let empty = FakeFetcher {
            responses: Default::default(),
        };
        let failed = resolve_network_links(
            ParseKmlResult {
                network_links: vec![link("https://example.test/missing.kml")],
                ..Default::default()
            },
            &empty,
            DEFAULT_MAX_DEPTH,
        );
        assert!(failed.warnings[0].contains("Failed to resolve NetworkLink"));
    }

    #[test]
    fn depth_limit_is_reported() {
        let mut responses = std::collections::HashMap::new();
        responses.insert(
            "https://example.test/a.kml".to_string(),
            r#"<kml><Document><NetworkLink><Link><href>https://example.test/b.kml</href></Link></NetworkLink></Document></kml>"#
                .to_string(),
        );
        let fetcher = FakeFetcher { responses };
        let resolved = resolve_network_links(
            ParseKmlResult {
                network_links: vec![link("https://example.test/a.kml")],
                ..Default::default()
            },
            &fetcher,
            1,
        );
        assert!(
            resolved
                .warnings
                .last()
                .is_some_and(|warning| warning.contains("max depth"))
        );
    }
}
