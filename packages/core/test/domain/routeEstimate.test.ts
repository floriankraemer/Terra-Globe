import { describe, expect, it } from "vitest";
import { estimateStraightLineRoute } from "../../src/domain/routeEstimate.js";

const berlin = { lon: 13.404954, lat: 52.5200066 };
const paris = { lon: 2.3522219, lat: 48.856614 };

describe("estimateStraightLineRoute", () => {
  it("computes straight-line distance and a mode-dependent duration", () => {
    const train = estimateStraightLineRoute([berlin, paris], "train");
    const plane = estimateStraightLineRoute([berlin, paris], "plane");

    expect(train.geometry).toEqual({ type: "LineString", path: [berlin, paris] });
    expect(train.distanceMeters).toBeCloseTo(plane.distanceMeters, 0);
    expect(train.distanceMeters).toBeGreaterThan(800_000);
    // Plane is faster, so covering the same distance takes less time.
    expect(plane.durationSeconds).toBeLessThan(train.durationSeconds);
  });

  it("sums distance across multiple waypoints in order", () => {
    const midpoint = { lon: 8, lat: 50 };
    const direct = estimateStraightLineRoute([berlin, paris], "train");
    const viaMidpoint = estimateStraightLineRoute([berlin, midpoint, paris], "train");
    expect(viaMidpoint.distanceMeters).toBeGreaterThan(direct.distanceMeters);
  });
});
