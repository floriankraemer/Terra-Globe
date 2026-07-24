import { describe, expect, it, vi } from "vitest";
import { NominatimGeocodingProvider } from "../../src/geocoding/NominatimGeocodingProvider.js";

function jsonResponse(body: unknown, init?: { ok?: boolean; status?: number }): Response {
  return {
    ok: init?.ok ?? true,
    status: init?.status ?? 200,
    statusText: "",
    json: async () => body,
  } as Response;
}

describe("NominatimGeocodingProvider", () => {
  it("returns [] without calling the network for an empty query", async () => {
    const fetchImpl = vi.fn();
    const provider = new NominatimGeocodingProvider(fetchImpl);
    expect(await provider.search("   ")).toEqual([]);
    expect(fetchImpl).not.toHaveBeenCalled();
  });

  it("maps Nominatim results to GeocodeResult[]", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(
        jsonResponse([{ display_name: "Berlin, Germany", lat: "52.5200066", lon: "13.4049540" }]),
      );
    const provider = new NominatimGeocodingProvider(fetchImpl);

    const results = await provider.search("Berlin");

    expect(results).toEqual([
      { label: "Berlin, Germany", point: { lon: 13.404954, lat: 52.5200066 } },
    ]);
    const [url] = fetchImpl.mock.calls[0] as [URL];
    expect(url.origin + url.pathname).toBe("https://nominatim.openstreetmap.org/search");
    expect(url.searchParams.get("q")).toBe("Berlin");
  });

  it("returns [] for no matches", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse([]));
    const provider = new NominatimGeocodingProvider(fetchImpl);
    expect(await provider.search("asdkjhasdkjhasd")).toEqual([]);
  });

  it("throws on an HTTP error response", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse([], { ok: false, status: 503 }));
    const provider = new NominatimGeocodingProvider(fetchImpl);
    await expect(provider.search("Berlin")).rejects.toThrow(/503/);
  });

  it("skips malformed rows instead of throwing", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(
        jsonResponse([{ display_name: "Missing coords" }, { lat: "1", lon: "2" }]),
      );
    const provider = new NominatimGeocodingProvider(fetchImpl);
    expect(await provider.search("x")).toEqual([]);
  });

  it("reverse() maps a single Nominatim result to a label", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(
        jsonResponse({ display_name: "Berlin, Germany", lat: "52.5200066", lon: "13.4049540" }),
      );
    const provider = new NominatimGeocodingProvider(fetchImpl);

    const label = await provider.reverse({ lon: 13.404954, lat: 52.5200066 });

    expect(label).toBe("Berlin, Germany");
    const [url] = fetchImpl.mock.calls[0] as [URL];
    expect(url.origin + url.pathname).toBe("https://nominatim.openstreetmap.org/reverse");
    expect(url.searchParams.get("lat")).toBe("52.5200066");
    expect(url.searchParams.get("lon")).toBe("13.404954");
  });

  it("reverse() returns null for no match", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({ error: "Unable to geocode" }));
    const provider = new NominatimGeocodingProvider(fetchImpl);
    expect(await provider.reverse({ lon: 0, lat: 0 })).toBeNull();
  });

  it("reverse() throws on an HTTP error response", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({}, { ok: false, status: 503 }));
    const provider = new NominatimGeocodingProvider(fetchImpl);
    await expect(provider.reverse({ lon: 0, lat: 0 })).rejects.toThrow(/503/);
  });
});
