import { describe, expect, it, vi } from "vitest";
import { OpenRouteServiceRoutingProvider } from "../../src/routing/OpenRouteServiceRoutingProvider.js";

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

describe("OpenRouteServiceRoutingProvider", () => {
  it("maps ORS geojson features to RouteLeg[]", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(
      jsonResponse({
        features: [
          {
            geometry: {
              coordinates: [
                [13.4, 52.5],
                [13.5, 52.6],
              ],
            },
            properties: { summary: { distance: 800, duration: 90 } },
          },
        ],
      }),
    );
    const provider = new OpenRouteServiceRoutingProvider("key", fetchImpl);

    const legs = await provider.route([A, B], "foot");

    expect(legs).toEqual([
      { geometry: { type: "LineString", path: [A, B] }, distanceMeters: 800, durationSeconds: 90 },
    ]);
    const [url, init] = fetchImpl.mock.calls[0] as [URL, RequestInit];
    expect(url.pathname).toContain("/directions/foot-walking/geojson");
    expect((init.headers as Record<string, string>).Authorization).toBe("key");
    expect(JSON.parse(init.body as string).coordinates).toEqual([
      [13.4, 52.5],
      [13.5, 52.6],
    ]);
  });

  it("throws on an HTTP error response", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({}, { ok: false, status: 429 }));
    const provider = new OpenRouteServiceRoutingProvider("bad", fetchImpl);
    await expect(provider.route([A, B], "car")).rejects.toThrow(/429/);
  });

  it("surfaces malformed features instead of silently dropping them", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(
        jsonResponse({ features: [{ properties: { summary: { distance: 800 } } }] }),
      );
    const provider = new OpenRouteServiceRoutingProvider("key", fetchImpl);
    await expect(provider.route([A, B], "car")).rejects.toThrow(/malformed/i);
  });
});
