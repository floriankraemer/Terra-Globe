import { describe, expect, it, vi } from "vitest";
import { MapboxRoutingProvider } from "../../src/routing/MapboxRoutingProvider.js";

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

describe("MapboxRoutingProvider", () => {
  it("maps Mapbox routes to RouteLeg[]", async () => {
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
            distance: 500,
            duration: 60,
          },
        ],
      }),
    );
    const provider = new MapboxRoutingProvider("token", fetchImpl);

    const legs = await provider.route([A, B], "car");

    expect(legs).toEqual([
      { geometry: { type: "LineString", path: [A, B] }, distanceMeters: 500, durationSeconds: 60 },
    ]);
    const [url] = fetchImpl.mock.calls[0] as [URL];
    expect(url.pathname).toContain("/directions/v5/mapbox/driving/13.4,52.5;13.5,52.6");
    expect(url.searchParams.get("access_token")).toBe("token");
  });

  it("throws on an HTTP error response", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({}, { ok: false, status: 401 }));
    const provider = new MapboxRoutingProvider("bad", fetchImpl);
    await expect(provider.route([A, B], "car")).rejects.toThrow(/401/);
  });

  it("surfaces malformed routes instead of silently dropping them", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({ routes: [{ distance: 500 }] }));
    const provider = new MapboxRoutingProvider("token", fetchImpl);
    await expect(provider.route([A, B], "car")).rejects.toThrow(/malformed/i);
  });
});
