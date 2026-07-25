# Terra Globe — Product Gap Analysis & USP

Owner: Product
Status: Draft
Scope: Compare Terra Globe against Google Earth / Google Maps, identify critical gaps, and define a defensible USP given the constraint that we cannot use commercially licensed map data (Google/Bing/HERE imagery, Street View, Google Places) without a paid agreement.

## 1. Where we stand today

Terra Globe is a CesiumJS-based 3D globe app, running identically in the browser and as a Tauri desktop app (Windows/macOS/Linux).
Current capabilities: draw/edit markers, circles, rectangles, polygons, lines; folder organization; KML/KMZ import/export; area/radius/circumference measurement; ruler; route planning; geocoding; elevation profile; undo/redo; multi-language UI (en/de/es/fr/uk).
Storage is local-first: IndexedDB in the browser, SQLite on desktop, behind a `PlacesRepository` port meant for future sync.
Base imagery today is OpenStreetMap / OpenTopoMap, with a pluggable provider system (tile, geocoding, routing) where users add their own API keys for presets like Mapbox.
There is no user account, no telemetry, and no first-party backend — everything runs against the user's own machine or the user's own third-party API keys.

## 2. The licensing constraint, stated plainly

Google Earth's imagery (global high-res satellite/aerial, 3D photogrammetry meshes, Street View panoramas) is licensed content Google pays billions to acquire and cannot legally be mirrored, proxied, or redistributed by a third party.
Google Maps' POI/business data, live traffic, and transit schedules are likewise licensed or scraped at Google's scale — not something we can reproduce without a commercial data agreement.
This means head-to-head feature parity with Google Earth/Maps is not a viable target.
Any roadmap item that implicitly assumes "just add what Google has" needs to be re-scoped around open or user-supplied data instead.

## 3. Critical gaps vs. Google Earth / Maps

| Area | Google Earth/Maps | Terra Globe today | Verdict |
|---|---|---|---|
| Satellite/aerial imagery | Global, high-res, proprietary | OSM/OpenTopoMap only (vector-derived raster, no satellite base layer) | **Critical gap** — no visually compelling "globe" experience without imagery |
| 3D terrain & buildings | Photogrammetry mesh globally | None (flat imagery draped on WGS84 ellipsoid/terrain) | Gap, but Cesium supports Cesium World Terrain / open DEMs — solvable |
| Street View | Proprietary panorama network | None, and not legally replicable | **Out of scope**, not a gap to close |
| POI / business search | Google Places (millions of businesses, reviews, hours) | Geocoding only (address → coordinate) | Gap — closable via OSM Overpass/Nominatim POI data, no reviews/hours parity possible |
| Turn-by-turn / live traffic | Proprietary traffic model, voice nav | Static route planning via user-supplied routing provider | Partial gap — live traffic not replicable; static routing is fine |
| Offline maps | Built-in offline area download | None | Gap — real feature request for a desktop-first tool, and independent of licensing |
| Transit directions | Full GTFS integration globally | None | Out of scope for v1, revisit later via open GTFS feeds |
| Search-as-you-explore ("what's near me") | Deeply integrated | Address geocoding only | Gap, closable with OSM data |
| Sharing / collaboration | Google account, links, Drive | Local-first, no sync yet (`PlacesRepository` designed for it, not built) | Gap — but also our differentiator if done as *optional*, encrypted, self-hostable sync |
| Mobile app | iOS/Android native | None (browser works, no packaged mobile app) | Gap, lower priority than desktop/offline |
| Account / cross-device | Google account required | None required | **Not a gap — this is a feature, not a hole** |

## 4. What actually matters (priority order)

1. **A real satellite/aerial base layer option.** Without this, the "3D globe" pitch feels empty next to Google Earth. Use open/permissively-licensed sources: Sentinel-2 (ESA, free), NASA GIBS/Blue Marble, or let users plug in their own commercial imagery API key (Bing Maps, Mapbox Satellite, Maxar) through the existing provider system — same BYOK pattern already used for tiles/geocoding/routing. This sidesteps the licensing problem entirely: we ship zero proprietary imagery ourselves, users who want commercial-grade imagery bring their own licensed access.
2. **Offline area download & caching.** Desktop-native, local-first, no account — offline support is a natural extension of what we already are, and something Google Earth Web *doesn't* do well. High leverage, no licensing risk.
3. **Open POI search** (Overpass API / Nominatim) so "search nearby" works without needing Google Places.
4. **Optional, user-controlled sync** — self-hosted (the `storage-remote` package already exists) or a lightweight hosted relay users opt into, end-to-end encrypted, never a requirement. This turns our biggest apparent gap (no cross-device sync) into proof of the privacy/independence USP rather than a weakness.
5. **3D terrain** via Cesium World Terrain or open DEM sources (Copernicus GLO-30) — cheap win, Cesium supports it natively, no licensing conflict.

Explicitly **not pursuing**: Street View equivalents, proprietary POI/business databases, live traffic, anything requiring us to hold or redistribute someone else's licensed dataset.

## 5. USP

**Terra Globe is the globe app that works entirely on your terms: no account, no tracking, no vendor lock-in — your places live on your disk (or your own server), and every data source is one you chose, not one we imposed.**

Three pillars:

- **Privacy by construction, not by policy.** No telemetry, no account, no first-party server in the default path. We can't leak what we never collect. This is a structural property of the architecture (local-first storage, BYOK providers), not a settings toggle that can regress.
- **Independence from any single map vendor.** Every data source — basemap tiles, geocoding, routing, imagery — is a pluggable provider the user configures with their own key or points at an open source. Nobody can raise our prices, revoke our access, or shut off a feature we depend on, because we don't depend on any one of them exclusively.
- **You own your data, literally.** KML/KMZ in and out, SQLite/IndexedDB on your own machine, an explicit repository boundary designed for self-hosted sync. Your placemarks are a file you hold, not a row in someone else's database.

Positioning line: *"Google Earth for people who don't want to be the product."*

## 6. Risks to this positioning

- If offline/imagery gaps stay open too long, "privacy-first" reads as "worse product" rather than "principled trade-off." Priority #1 and #2 above exist to close that gap before it becomes the dominant narrative.
- BYOK for imagery (letting users plug in Bing/Mapbox Satellite keys) must be presented carefully — we're not redistributing anything, but the UI should make clear that key belongs to the user and its use is subject to that vendor's terms.
- Self-hosted sync, if it ships half-built, undermines the "independence" pillar it's meant to prove. Better to ship it later and well than early and flaky.
