import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { FakeGeocodingProvider, type GeocodingProvider } from "@terra-globe/core";
import { useGeocoding } from "./useGeocoding.js";

const BERLIN = { label: "Berlin, Germany", point: { lon: 13.4, lat: 52.5 } };

describe("useGeocoding", () => {
  it("starts idle with no results", () => {
    const { result } = renderHook(() => useGeocoding(new FakeGeocodingProvider()));
    expect(result.current.status).toBe("idle");
    expect(result.current.results).toEqual([]);
  });

  it("goes loading then ready with results on a successful search", async () => {
    const provider = new FakeGeocodingProvider({ Berlin: [BERLIN] });
    const { result } = renderHook(() => useGeocoding(provider));

    act(() => {
      void result.current.search("Berlin");
    });
    expect(result.current.status).toBe("loading");

    await waitFor(() => expect(result.current.status).toBe("ready"));
    expect(result.current.results).toEqual([BERLIN]);
  });

  it("goes ready with an empty list when nothing matches", async () => {
    const { result } = renderHook(() => useGeocoding(new FakeGeocodingProvider()));

    act(() => {
      void result.current.search("nowhere");
    });

    await waitFor(() => expect(result.current.status).toBe("ready"));
    expect(result.current.results).toEqual([]);
  });

  it("goes to error status when the provider rejects", async () => {
    const provider: GeocodingProvider = {
      search: () => Promise.reject(new Error("network down")),
    };
    const { result } = renderHook(() => useGeocoding(provider));

    act(() => {
      void result.current.search("Berlin");
    });

    await waitFor(() => expect(result.current.status).toBe("error"));
    expect(result.current.error).toBe("network down");
  });

  it("reset clears results and returns to idle", async () => {
    const provider = new FakeGeocodingProvider({ Berlin: [BERLIN] });
    const { result } = renderHook(() => useGeocoding(provider));

    act(() => {
      void result.current.search("Berlin");
    });
    await waitFor(() => expect(result.current.status).toBe("ready"));

    act(() => {
      result.current.reset();
    });

    expect(result.current.status).toBe("idle");
    expect(result.current.results).toEqual([]);
  });

  it("ignores an empty query without calling the provider", () => {
    const provider = new FakeGeocodingProvider();
    const { result } = renderHook(() => useGeocoding(provider));

    act(() => {
      void result.current.search("   ");
    });

    expect(result.current.status).toBe("idle");
  });
});
