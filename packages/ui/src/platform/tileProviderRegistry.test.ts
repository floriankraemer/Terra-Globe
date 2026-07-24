import { describe, expect, it } from "vitest";
import type { ProviderConfig, SecretStore } from "@webglobe/core";
import { buildTileSources } from "./tileProviderRegistry.js";

function fakeSecretStore(secrets: Record<string, string>): SecretStore {
  return {
    async get(id) {
      return secrets[id];
    },
    async set() {},
    async remove() {},
  };
}

describe("buildTileSources", () => {
  it("returns only the built-ins when there are no enabled tile providers", async () => {
    const sources = await buildTileSources([], fakeSecretStore({}));
    expect(sources.map((s) => s.id)).toEqual(["osm", "opentopomap"]);
  });

  it("appends enabled tile providers with the resolved API key substituted", async () => {
    const providers: ProviderConfig[] = [
      { id: "p1", kind: "tile", preset: "mapbox-streets", name: "My Mapbox", enabled: true },
    ];
    const sources = await buildTileSources(providers, fakeSecretStore({ p1: "sk-123" }));

    const custom = sources.find((s) => s.id === "p1");
    expect(custom?.name).toBe("My Mapbox");
    expect(custom?.url).toContain("access_token=sk-123");
  });

  it("excludes disabled tile providers and geocoding providers", async () => {
    const providers: ProviderConfig[] = [
      { id: "p1", kind: "tile", preset: "mapbox-streets", name: "Disabled", enabled: false },
      { id: "g1", kind: "geocoding", preset: "opencage", name: "Geocoder", enabled: true },
    ];
    const sources = await buildTileSources(providers, fakeSecretStore({}));
    expect(sources.map((s) => s.id)).toEqual(["osm", "opentopomap"]);
  });
});
