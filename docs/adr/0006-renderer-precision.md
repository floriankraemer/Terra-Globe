# ADR 0006 — globe precision: f64 ECEF, relative-to-eye transforms, reverse-Z

Status: accepted (2026-10-31)

## Context

A globe spans about 40 000 km, while street-level inspection needs centimeter-ish stability.
Uploading absolute Earth-centred coordinates as `f32` gives roughly half-meter precision at the surface (6.4e6 m exceeds `f32`'s 24-bit mantissa), which shows up as vertex jitter that gets worse as the camera moves.
Depth precision is the second problem: a near plane of 1 m and a far plane of 40 000 km in a standard depth mapping leaves almost nothing for the surface.

## Decision

- All geodesy, camera math and scene data use `f64`.
- Each frame, the camera eye is computed in `f64` and everything on the GPU is **eye-relative**: tile translation is `f32(tile_center - eye)`, and vertices are `f32` local to the tile centre. No large coordinate reaches the GPU.
- Projection is reverse-Z with an infinite far plane (`Depth32Float`, `CompareFunction::Greater`, near plane 1 m). Near maps to depth 1.0, infinity to 0.0.
- Tiles carry a skirt extruded inwards so LOD boundaries cannot show cracks; there is no separate base globe mesh, so no coplanar z-fighting between a sphere and the imagery.
- Backface culling is disabled: tiles are viewed from outside, and winding at the antimeridian and poles is not worth the risk; the cost is negligible.

## Consequences

- A camera-sweep test exercises 1 m to 40 000 km and asserts visible targets stay visible without artifacts.
- Tile meshes are cached by tile id; only the per-tile translation changes per frame.
- If a driver shows depth precision problems, the designed fallback is a logarithmic depth shader, which is not built until needed.
- Multi-frustum rendering (Cesium's approach) is deliberately not reproduced.
