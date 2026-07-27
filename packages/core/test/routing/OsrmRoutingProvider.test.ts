import { describe, expect, it, vi } from "vitest";
import { OsrmRoutingProvider } from "../../src/routing/OsrmRoutingProvider.js";

function jsonResponse(body: unknown, init?: { ok?: boolean; status?: number }): Response {
  return {
    ok: init?.ok ?? true,
    status: init?.status ?? 200,
    statusText: "",
    json: async () => body,
  } as Response;
}

const A = { lon: 13.4, lat: 52.5 };
const B = { lon: 13.5, lat: 52.6 };

describe("OsrmRoutingProvider", () => {
  it("returns [] without calling the network for fewer than 2 waypoints", async () => {
    const fetchImpl = vi.fn();
    const provider = new OsrmRoutingProvider(undefined, fetchImpl);
    expect(await provider.route([A], "car")).toEqual([]);
    expect(fetchImpl).not.toHaveBeenCalled();
  });

  it("maps OSRM routes to RouteLeg[], primary first", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(
      jsonResponse({
        routes: [
          {
            geometry: {
              coordinates: [
                [13.4, 52.5],
                [13.5, 52.6],
              ],
            },
            distance: 1000,
            duration: 120,
          },
          {
            geometry: {
              coordinates: [
                [13.4, 52.5],
                [13.45, 52.55],
                [13.5, 52.6],
              ],
            },
            distance: 1200,
            duration: 150,
          },
        ],
      }),
    );
    const provider = new OsrmRoutingProvider(undefined, fetchImpl);

    const legs = await provider.route([A, B], "foot");

    expect(legs).toHaveLength(2);
    expect(legs[0]).toEqual({
      geometry: { type: "LineString", path: [A, B] },
      distanceMeters: 1000,
      durationSeconds: 120,
    });
    const [url] = fetchImpl.mock.calls[0] as [URL];
    expect(url.hostname).toBe("routing.openstreetmap.de");
    expect(url.pathname).toContain("/routed-foot/route/v1/walking/13.4,52.5;13.5,52.6");
    expect(url.searchParams.get("geometries")).toBe("geojson");
  });

  it("uses a different host per profile, since router.project-osrm.org ignores the profile segment", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({ routes: [] }));
    const provider = new OsrmRoutingProvider(undefined, fetchImpl);

    await provider.route([A, B], "car");
    await provider.route([A, B], "foot");
    await provider.route([A, B], "bike");

    const hosts = fetchImpl.mock.calls.map(([url]: [URL]) => url.pathname.split("/")[1]);
    expect(hosts).toEqual(["routed-car", "routed-foot", "routed-bike"]);
  });

  it("throws on an HTTP error response", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({}, { ok: false, status: 500 }));
    const provider = new OsrmRoutingProvider(undefined, fetchImpl);
    await expect(provider.route([A, B], "car")).rejects.toThrow(/500/);
  });

  it("surfaces malformed routes instead of silently dropping them", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(
      jsonResponse({
        routes: [{ distance: 1000 }],
      }),
    );
    const provider = new OsrmRoutingProvider(undefined, fetchImpl);
    await expect(provider.route([A, B], "car")).rejects.toThrow(/malformed/i);
  });
});
