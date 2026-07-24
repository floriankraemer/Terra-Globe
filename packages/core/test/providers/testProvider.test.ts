import { describe, expect, it, vi } from "vitest";
import {
  testGeocodingProviderConfig,
  testTileProviderConfig,
} from "../../src/providers/testProvider.js";
import type {
  GeocodingProviderConfig,
  TileProviderConfig,
} from "../../src/providers/ProviderConfig.js";

function imageResponse(init?: { ok?: boolean; status?: number; contentType?: string }): Response {
  return {
    ok: init?.ok ?? true,
    status: init?.status ?? 200,
    statusText: "",
    headers: { get: () => init?.contentType ?? "image/png" },
  } as unknown as Response;
}

function jsonResponse(body: unknown, init?: { ok?: boolean; status?: number }): Response {
  return {
    ok: init?.ok ?? true,
    status: init?.status ?? 200,
    statusText: "",
    json: async () => body,
  } as Response;
}

const tileConfig: TileProviderConfig = {
  id: "t1",
  kind: "tile",
  preset: "mapbox-streets",
  name: "My Mapbox",
  enabled: false,
};

const geocodingConfig: GeocodingProviderConfig = {
  id: "g1",
  kind: "geocoding",
  preset: "opencage",
  name: "My OpenCage",
  enabled: false,
};

describe("testTileProviderConfig", () => {
  it("succeeds for a 200 image response", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(imageResponse());
    const result = await testTileProviderConfig(tileConfig, "key", fetchImpl);
    expect(result).toEqual({ ok: true });
    const [url] = fetchImpl.mock.calls[0] as [string];
    expect(url).toContain("/0/0/0");
    expect(url).toContain("access_token=key");
  });

  it("fails for a non-OK HTTP response", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(imageResponse({ ok: false, status: 404 }));
    const result = await testTileProviderConfig(tileConfig, "key", fetchImpl);
    expect(result.ok).toBe(false);
    expect(result.error).toContain("404");
  });

  it("fails for a non-image content-type", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(imageResponse({ contentType: "text/html" }));
    const result = await testTileProviderConfig(tileConfig, "key", fetchImpl);
    expect(result.ok).toBe(false);
    expect(result.error).toContain("text/html");
  });

  it("fails when fetch throws", async () => {
    const fetchImpl = vi.fn().mockRejectedValue(new Error("network down"));
    const result = await testTileProviderConfig(tileConfig, "key", fetchImpl);
    expect(result).toEqual({ ok: false, error: "network down" });
  });
});

describe("testGeocodingProviderConfig", () => {
  it("succeeds when the search resolves", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({ results: [] }));
    const result = await testGeocodingProviderConfig(geocodingConfig, "key", fetchImpl);
    expect(result).toEqual({ ok: true });
  });

  it("fails when the search throws", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(jsonResponse({}, { ok: false, status: 401 }));
    const result = await testGeocodingProviderConfig(geocodingConfig, "bad-key", fetchImpl);
    expect(result.ok).toBe(false);
    expect(result.error).toContain("401");
  });
});
