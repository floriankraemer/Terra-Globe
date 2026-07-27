import { describe, expect, it, vi } from "vitest";
import { GraphHopperRoutingProvider } from "../../src/routing/GraphHopperRoutingProvider.js";

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

describe("GraphHopperRoutingProvider", () => {
  it("maps GraphHopper paths to RouteLeg[], converting time ms to seconds", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(
      jsonResponse({
        paths: [
          {
            points: {
              coordinates: [
                [13.4, 52.5],
                [13.5, 52.6],
              ],
            },
            distance: 2000,
            time: 300000,
          },
        ],
      }),
    );
    const provider = new GraphHopperRoutingProvider("key", fetchImpl);

    const legs = await provider.route([A, B], "bike");

    expect(legs).toEqual([
      {
        geometry: { type: "LineString", path: [A, B] },
        distanceMeters: 2000,
        durationSeconds: 300,
      },
    ]);
    const [url] = fetchImpl.mock.calls[0] as [URL];
    expect(url.searchParams.getAll("point")).toEqual(["52.5,13.4", "52.6,13.5"]);
    expect(url.searchParams.get("profile")).toBe("bike");
    expect(url.searchParams.get("key")).toBe("key");
  });

  it("throws on an HTTP error response", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({}, { ok: false, status: 403 }));
    const provider = new GraphHopperRoutingProvider("bad", fetchImpl);
    await expect(provider.route([A, B], "car")).rejects.toThrow(/403/);
  });

  it("surfaces malformed paths instead of silently dropping them", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({ paths: [{ distance: 2000 }] }));
    const provider = new GraphHopperRoutingProvider("key", fetchImpl);
    await expect(provider.route([A, B], "car")).rejects.toThrow(/malformed/i);
  });
});
