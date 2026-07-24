import { describe, expect, it } from "vitest";
import { createLineStringGeometry } from "../../src/domain/geometry.js";
import {
  computeTrackProfile,
  haversineMeters,
  pickGridStepMeters,
} from "../../src/domain/trackProfile.js";

describe("haversineMeters", () => {
  it("returns 0 for identical points", () => {
    expect(haversineMeters({ lon: 0, lat: 0 }, { lon: 0, lat: 0 })).toBe(0);
  });

  it("returns ~111km for one degree of latitude", () => {
    const distance = haversineMeters({ lon: 0, lat: 0 }, { lon: 0, lat: 1 });
    expect(distance).toBeGreaterThan(110_000);
    expect(distance).toBeLessThan(112_000);
  });
});

describe("computeTrackProfile", () => {
  it("accumulates distance and reads per-vertex altitude", () => {
    const geometry = createLineStringGeometry([
      { lon: 0, lat: 0, altitude: 10 },
      { lon: 0, lat: 1, altitude: 50 },
      { lon: 0, lat: 2, altitude: 30 },
    ]);
    const profile = computeTrackProfile(geometry);
    expect(profile).toHaveLength(3);
    expect(profile[0]).toEqual({ distanceMeters: 0, altitudeMeters: 10 });
    expect(profile[1]!.distanceMeters).toBeGreaterThan(110_000);
    expect(profile[1]!.altitudeMeters).toBe(50);
    expect(profile[2]!.distanceMeters).toBeGreaterThan(profile[1]!.distanceMeters);
    expect(profile[2]!.altitudeMeters).toBe(30);
  });

  it("defaults missing altitude to 0", () => {
    const geometry = createLineStringGeometry([
      { lon: 0, lat: 0 },
      { lon: 0, lat: 1 },
    ]);
    const profile = computeTrackProfile(geometry);
    expect(profile.every((p) => p.altitudeMeters === 0)).toBe(true);
  });

  it("returns a single zero-distance point for a degenerate two-point track at the same location", () => {
    const geometry = createLineStringGeometry([
      { lon: 5, lat: 5, altitude: 20 },
      { lon: 5, lat: 5, altitude: 20 },
    ]);
    const profile = computeTrackProfile(geometry);
    expect(profile[0]!.distanceMeters).toBe(0);
    expect(profile[1]!.distanceMeters).toBe(0);
  });
});

describe("pickGridStepMeters", () => {
  it("returns 500m under 5km", () => {
    expect(pickGridStepMeters(0)).toBe(500);
    expect(pickGridStepMeters(4999)).toBe(500);
  });

  it("returns 1000m at/above 5km", () => {
    expect(pickGridStepMeters(5000)).toBe(1000);
    expect(pickGridStepMeters(20_000)).toBe(1000);
  });
});
