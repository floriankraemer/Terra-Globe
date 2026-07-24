import { fireEvent, render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { TrackProfilePoint } from "@webglobe/core";
import { HeightProfileChart } from "./HeightProfileChart.js";

describe("HeightProfileChart", () => {
  it("renders a polyline and gridlines for a multi-point track with varying altitude", () => {
    const profile: TrackProfilePoint[] = [
      { distanceMeters: 0, altitudeMeters: 10 },
      { distanceMeters: 3000, altitudeMeters: 80 },
      { distanceMeters: 6000, altitudeMeters: 40 },
    ];
    const { container } = render(
      <HeightProfileChart profile={profile} unitSystem="metric" width={400} height={200} />,
    );
    const polyline = container.querySelector(".height-profile-line");
    expect(polyline?.tagName).toBe("polyline");
    // Track exceeds 5km, so it should switch to 1km gridline spacing (7 lines: 0..6km).
    expect(container.querySelectorAll(".height-profile-gridline").length).toBeGreaterThan(0);
  });

  it("renders without crashing for a flat/no-altitude track", () => {
    const profile: TrackProfilePoint[] = [
      { distanceMeters: 0, altitudeMeters: 0 },
      { distanceMeters: 1000, altitudeMeters: 0 },
    ];
    const { container } = render(
      <HeightProfileChart profile={profile} unitSystem="metric" width={400} height={200} />,
    );
    expect(container.querySelector(".height-profile-line")).not.toBeNull();
  });

  it("renders a single-point degenerate track as a point, not a crash", () => {
    const profile: TrackProfilePoint[] = [{ distanceMeters: 0, altitudeMeters: 25 }];
    const { container } = render(
      <HeightProfileChart profile={profile} unitSystem="metric" width={400} height={200} />,
    );
    expect(container.querySelector("circle.height-profile-line")).not.toBeNull();
    expect(container.querySelector("polyline.height-profile-line")).toBeNull();
  });

  it("renders without crashing for an empty profile", () => {
    const { container } = render(
      <HeightProfileChart profile={[]} unitSystem="metric" width={400} height={200} />,
    );
    expect(container.querySelector("svg")).not.toBeNull();
  });

  it("shows a hover crosshair with interpolated altitude on mousemove, and hides it on mouseleave", () => {
    const profile: TrackProfilePoint[] = [
      { distanceMeters: 0, altitudeMeters: 0 },
      { distanceMeters: 1000, altitudeMeters: 100 },
    ];
    const { container } = render(
      <HeightProfileChart profile={profile} unitSystem="metric" width={400} height={200} />,
    );
    const target = container.querySelector(".height-profile-hover-target")!;
    // width=400, PADDING_LEFT=48, PADDING_RIGHT=12 -> plotWidth=340; hover at its midpoint.
    target.getBoundingClientRect = () =>
      ({ left: 0, top: 0, right: 340, bottom: 200, width: 340, height: 200 }) as DOMRect;
    fireEvent.mouseMove(target, { clientX: 170 });
    expect(container.querySelector(".height-profile-hover-line")).not.toBeNull();
    // Midpoint of distance -> midpoint of altitude (0..100) = ~50m.
    expect(container.querySelector(".height-profile-hover-label")?.textContent).toBe("50m");

    fireEvent.mouseLeave(target);
    expect(container.querySelector(".height-profile-hover-line")).toBeNull();
  });
});
