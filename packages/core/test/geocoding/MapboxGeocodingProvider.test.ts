import { describe, expect, it, vi } from "vitest";
import { MapboxGeocodingProvider } from "../../src/geocoding/MapboxGeocodingProvider.js";

function jsonResponse(body: unknown, init?: { ok?: boolean; status?: number }): Response {
  return {
    ok: init?.ok ?? true,
    status: init?.status ?? 200,
    statusText: "",
    json: async () => body,
  } as Response;
}

describe("MapboxGeocodingProvider", () => {
  it("returns [] without calling the network for an empty query", async () => {
    const fetchImpl = vi.fn();
    const provider = new MapboxGeocodingProvider("token", fetchImpl);
    expect(await provider.search("   ")).toEqual([]);
    expect(fetchImpl).not.toHaveBeenCalled();
  });

  it("maps Mapbox features to GeocodeResult[]", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(
      jsonResponse({
        features: [{ place_name: "Berlin, Germany", center: [13.404954, 52.5200066] }],
      }),
    );
    const provider = new MapboxGeocodingProvider("secret-token", fetchImpl);

    const results = await provider.search("Berlin");

    expect(results).toEqual([
      { label: "Berlin, Germany", point: { lon: 13.404954, lat: 52.5200066 } },
    ]);
    const [url] = fetchImpl.mock.calls[0] as [URL];
    expect(url.pathname).toContain("Berlin.json");
    expect(url.searchParams.get("access_token")).toBe("secret-token");
  });

  it("returns [] for no matches", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({ features: [] }));
    const provider = new MapboxGeocodingProvider("token", fetchImpl);
    expect(await provider.search("asdkjhasdkjhasd")).toEqual([]);
  });

  it("throws on an HTTP error response", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({}, { ok: false, status: 401 }));
    const provider = new MapboxGeocodingProvider("bad-token", fetchImpl);
    await expect(provider.search("Berlin")).rejects.toThrow(/401/);
  });

  it("skips malformed rows instead of throwing", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(jsonResponse({ features: [{ place_name: "Missing center" }] }));
    const provider = new MapboxGeocodingProvider("token", fetchImpl);
    expect(await provider.search("x")).toEqual([]);
  });
});
