import { describe, expect, it } from "vitest";
import { circleToPolygonRing } from "../../../src/kml/mapping/circleApproximation.js";

describe("circleToPolygonRing", () => {
  it("returns a closed ring with segments + 1 points", () => {
    const ring = circleToPolygonRing({ lon: 0, lat: 0 }, 1000, 8);
    expect(ring).toHaveLength(9);
    expect(ring[8]!.lon).toBeCloseTo(ring[0]!.lon, 9);
    expect(ring[8]!.lat).toBeCloseTo(ring[0]!.lat, 9);
  });

  it("places the bearing-0 point directly north of the center at ~radius distance", () => {
    const radiusMeters = 111_195; // ~1 degree of latitude
    const ring = circleToPolygonRing({ lon: 0, lat: 0 }, radiusMeters, 4);
    expect(ring[0]!.lon).toBeCloseTo(0, 6);
    expect(ring[0]!.lat).toBeCloseTo(1, 2);
  });

  it("keeps every point at approximately the same distance from the center", () => {
    const ring = circleToPolygonRing({ lon: 10, lat: 20 }, 5000, 16);
    const distances = ring.map((p) => {
      const dLat = ((p.lat - 20) * Math.PI) / 180;
      const dLon = ((p.lon - 10) * Math.PI) / 180;
      const lat1 = (20 * Math.PI) / 180;
      const a =
        Math.sin(dLat / 2) ** 2 + Math.cos(lat1) * Math.cos(lat1 + dLat) * Math.sin(dLon / 2) ** 2;
      return 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a)) * 6_371_000;
    });
    for (const d of distances) {
      expect(d).toBeCloseTo(5000, -1);
    }
  });
});
