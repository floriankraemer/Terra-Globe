import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { SecretStore } from "@webglobe/core";
import { useSettings } from "./useSettings.js";

function fakeSecretStore(): SecretStore & { removed: string[] } {
  const removed: string[] = [];
  return {
    removed,
    async get() {
      return undefined;
    },
    async set() {},
    async remove(id: string) {
      removed.push(id);
    },
  };
}

describe("useSettings", () => {
  afterEach(() => {
    window.localStorage.clear();
  });

  it("defaults to metric units, decimal coordinates, and no providers", () => {
    const { result } = renderHook(() => useSettings(fakeSecretStore()));
    expect(result.current.unitSystem).toBe("metric");
    expect(result.current.coordinateFormat).toBe("decimal");
    expect(result.current.providers).toEqual([]);
  });

  it("updates and persists the unit system", () => {
    const { result } = renderHook(() => useSettings(fakeSecretStore()));

    act(() => result.current.setUnitSystem("imperial"));

    expect(result.current.unitSystem).toBe("imperial");
    expect(JSON.parse(window.localStorage.getItem("webglobe:settings")!)).toMatchObject({
      unitSystem: "imperial",
    });
  });

  it("updates and persists the coordinate format", () => {
    const { result } = renderHook(() => useSettings(fakeSecretStore()));

    act(() => result.current.setCoordinateFormat("dms"));

    expect(result.current.coordinateFormat).toBe("dms");
    expect(JSON.parse(window.localStorage.getItem("webglobe:settings")!)).toMatchObject({
      coordinateFormat: "dms",
    });
  });

  it("reads previously persisted settings on mount", () => {
    window.localStorage.setItem(
      "webglobe:settings",
      JSON.stringify({ unitSystem: "imperial", coordinateFormat: "dms" }),
    );

    const { result } = renderHook(() => useSettings(fakeSecretStore()));

    expect(result.current.unitSystem).toBe("imperial");
    expect(result.current.coordinateFormat).toBe("dms");
  });

  it("falls back to defaults for corrupt stored settings", () => {
    window.localStorage.setItem("webglobe:settings", "not json");

    const { result } = renderHook(() => useSettings(fakeSecretStore()));

    expect(result.current.unitSystem).toBe("metric");
    expect(result.current.coordinateFormat).toBe("decimal");
    expect(result.current.providers).toEqual([]);
  });

  it("falls back to [] when stored providers is not an array", () => {
    window.localStorage.setItem("webglobe:settings", JSON.stringify({ providers: "nope" }));

    const { result } = renderHook(() => useSettings(fakeSecretStore()));

    expect(result.current.providers).toEqual([]);
  });

  it("addProvider appends a disabled config and persists it", () => {
    const { result } = renderHook(() => useSettings(fakeSecretStore()));

    let id = "";
    act(() => {
      id = result.current.addProvider({
        kind: "tile",
        preset: "mapbox-streets",
        name: "My Mapbox",
      });
    });

    expect(result.current.providers).toEqual([
      { id, kind: "tile", preset: "mapbox-streets", name: "My Mapbox", enabled: false },
    ]);
    expect(JSON.parse(window.localStorage.getItem("webglobe:settings")!).providers).toHaveLength(1);
  });

  it("setProviderEnabled enables the target provider", () => {
    const { result } = renderHook(() => useSettings(fakeSecretStore()));
    let id = "";
    act(() => {
      id = result.current.addProvider({ kind: "tile", preset: "mapbox-streets", name: "Mapbox" });
    });

    act(() => result.current.setProviderEnabled(id, true));

    expect(result.current.providers.find((p) => p.id === id)?.enabled).toBe(true);
  });

  it("enabling a geocoding provider disables any other enabled geocoding provider", () => {
    const { result } = renderHook(() => useSettings(fakeSecretStore()));
    let firstId = "";
    let secondId = "";
    act(() => {
      firstId = result.current.addProvider({
        kind: "geocoding",
        preset: "opencage",
        name: "First",
      });
    });
    act(() => result.current.setProviderEnabled(firstId, true));
    act(() => {
      secondId = result.current.addProvider({
        kind: "geocoding",
        preset: "locationiq",
        name: "Second",
      });
    });

    act(() => result.current.setProviderEnabled(secondId, true));

    expect(result.current.providers.find((p) => p.id === firstId)?.enabled).toBe(false);
    expect(result.current.providers.find((p) => p.id === secondId)?.enabled).toBe(true);
  });

  it("does not disable tile providers when enabling another tile provider", () => {
    const { result } = renderHook(() => useSettings(fakeSecretStore()));
    let firstId = "";
    let secondId = "";
    act(() => {
      firstId = result.current.addProvider({
        kind: "tile",
        preset: "mapbox-streets",
        name: "First",
      });
    });
    act(() => result.current.setProviderEnabled(firstId, true));
    act(() => {
      secondId = result.current.addProvider({
        kind: "tile",
        preset: "maptiler-streets",
        name: "Second",
      });
    });

    act(() => result.current.setProviderEnabled(secondId, true));

    expect(result.current.providers.find((p) => p.id === firstId)?.enabled).toBe(true);
    expect(result.current.providers.find((p) => p.id === secondId)?.enabled).toBe(true);
  });

  it("removeProvider removes the config and clears its secret", () => {
    const secretStore = fakeSecretStore();
    const { result } = renderHook(() => useSettings(secretStore));
    let id = "";
    act(() => {
      id = result.current.addProvider({ kind: "tile", preset: "mapbox-streets", name: "Mapbox" });
    });

    act(() => result.current.removeProvider(id));

    expect(result.current.providers).toEqual([]);
    expect(secretStore.removed).toEqual([id]);
  });
});
