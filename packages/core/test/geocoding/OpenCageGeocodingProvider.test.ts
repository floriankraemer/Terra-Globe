import { describe, expect, it, vi } from "vitest";
import { OpenCageGeocodingProvider } from "../../src/geocoding/OpenCageGeocodingProvider.js";

function jsonResponse(body: unknown, init?: { ok?: boolean; status?: number }): Response {
  return {
    ok: init?.ok ?? true,
    status: init?.status ?? 200,
    statusText: "",
    json: async () => body,
  } as Response;
}

describe("OpenCageGeocodingProvider", () => {
  it("returns [] without calling the network for an empty query", async () => {
    const fetchImpl = vi.fn();
    const provider = new OpenCageGeocodingProvider("key", fetchImpl);
    expect(await provider.search("   ")).toEqual([]);
    expect(fetchImpl).not.toHaveBeenCalled();
  });

  it("maps OpenCage results to GeocodeResult[]", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(
      jsonResponse({
        results: [{ formatted: "Berlin, Germany", geometry: { lat: 52.5200066, lng: 13.404954 } }],
      }),
    );
    const provider = new OpenCageGeocodingProvider("secret-key", fetchImpl);

    const results = await provider.search("Berlin");

    expect(results).toEqual([
      { label: "Berlin, Germany", point: { lon: 13.404954, lat: 52.5200066 } },
    ]);
    const [url] = fetchImpl.mock.calls[0] as [URL];
    expect(url.origin + url.pathname).toBe("https://api.opencagedata.com/geocode/v1/json");
    expect(url.searchParams.get("key")).toBe("secret-key");
  });

  it("returns [] for no matches", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({ results: [] }));
    const provider = new OpenCageGeocodingProvider("key", fetchImpl);
    expect(await provider.search("asdkjhasdkjhasd")).toEqual([]);
  });

  it("throws on an HTTP error response", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({}, { ok: false, status: 401 }));
    const provider = new OpenCageGeocodingProvider("bad-key", fetchImpl);
    await expect(provider.search("Berlin")).rejects.toThrow(/401/);
  });

  it("skips malformed rows instead of throwing", async () => {
    const fetchImpl = vi
      .fn()
      .mockResolvedValue(jsonResponse({ results: [{ formatted: "Missing geometry" }] }));
    const provider = new OpenCageGeocodingProvider("key", fetchImpl);
    expect(await provider.search("x")).toEqual([]);
  });

  it("reverse() maps the first result to a label", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(
      jsonResponse({
        results: [{ formatted: "Berlin, Germany", geometry: { lat: 52.5200066, lng: 13.404954 } }],
      }),
    );
    const provider = new OpenCageGeocodingProvider("secret-key", fetchImpl);

    const label = await provider.reverse({ lon: 13.404954, lat: 52.5200066 });

    expect(label).toBe("Berlin, Germany");
    const [url] = fetchImpl.mock.calls[0] as [URL];
    expect(url.searchParams.get("q")).toBe("52.5200066+13.404954");
  });

  it("reverse() returns null for no match", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({ results: [] }));
    const provider = new OpenCageGeocodingProvider("key", fetchImpl);
    expect(await provider.reverse({ lon: 0, lat: 0 })).toBeNull();
  });

  it("reverse() throws on an HTTP error response", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({}, { ok: false, status: 401 }));
    const provider = new OpenCageGeocodingProvider("bad-key", fetchImpl);
    await expect(provider.reverse({ lon: 0, lat: 0 })).rejects.toThrow(/401/);
  });
});
