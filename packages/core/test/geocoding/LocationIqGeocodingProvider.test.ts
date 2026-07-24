import { describe, expect, it, vi } from "vitest";
import { LocationIqGeocodingProvider } from "../../src/geocoding/LocationIqGeocodingProvider.js";

function jsonResponse(body: unknown, init?: { ok?: boolean; status?: number }): Response {
  return {
    ok: init?.ok ?? true,
    status: init?.status ?? 200,
    statusText: "",
    json: async () => body,
  } as Response;
}

describe("LocationIqGeocodingProvider", () => {
  it("returns [] without calling the network for an empty query", async () => {
    const fetchImpl = vi.fn();
    const provider = new LocationIqGeocodingProvider("key", fetchImpl);
    expect(await provider.search("   ")).toEqual([]);
    expect(fetchImpl).not.toHaveBeenCalled();
  });

  it("maps Nominatim-shaped results and sends the key as a query param", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(
        jsonResponse([{ display_name: "Berlin, Germany", lat: "52.5200066", lon: "13.4049540" }]),
      );
    const provider = new LocationIqGeocodingProvider("secret-key", fetchImpl);

    const results = await provider.search("Berlin");

    expect(results).toEqual([
      { label: "Berlin, Germany", point: { lon: 13.404954, lat: 52.5200066 } },
    ]);
    const [url] = fetchImpl.mock.calls[0] as [URL];
    expect(url.origin + url.pathname).toBe("https://us1.locationiq.com/v1/search");
    expect(url.searchParams.get("q")).toBe("Berlin");
    expect(url.searchParams.get("key")).toBe("secret-key");
  });

  it("throws on an HTTP error response", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse([], { ok: false, status: 401 }));
    const provider = new LocationIqGeocodingProvider("bad-key", fetchImpl);
    await expect(provider.search("Berlin")).rejects.toThrow(/401/);
  });

  it("skips malformed rows instead of throwing", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse([{ display_name: "Missing coords" }]));
    const provider = new LocationIqGeocodingProvider("key", fetchImpl);
    expect(await provider.search("x")).toEqual([]);
  });

  it("reverse() maps a single Nominatim-shaped result to a label", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(
        jsonResponse({ display_name: "Berlin, Germany", lat: "52.5200066", lon: "13.4049540" }),
      );
    const provider = new LocationIqGeocodingProvider("secret-key", fetchImpl);

    const label = await provider.reverse({ lon: 13.404954, lat: 52.5200066 });

    expect(label).toBe("Berlin, Germany");
    const [url] = fetchImpl.mock.calls[0] as [URL];
    expect(url.origin + url.pathname).toBe("https://us1.locationiq.com/v1/reverse");
    expect(url.searchParams.get("key")).toBe("secret-key");
  });

  it("reverse() throws on an HTTP error response", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({}, { ok: false, status: 401 }));
    const provider = new LocationIqGeocodingProvider("bad-key", fetchImpl);
    await expect(provider.reverse({ lon: 0, lat: 0 })).rejects.toThrow(/401/);
  });
});
