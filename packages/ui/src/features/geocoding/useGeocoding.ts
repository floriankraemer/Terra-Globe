import { useCallback, useRef, useState } from "react";
import type { GeocodeResult, GeocodingProvider } from "@webglobe/core";

export type GeocodingStatus = "idle" | "loading" | "ready" | "error";

export interface UseGeocodingResult {
  status: GeocodingStatus;
  results: GeocodeResult[];
  error: string | null;
  search: (query: string) => Promise<void>;
  reset: () => void;
}

export function useGeocoding(provider: GeocodingProvider): UseGeocodingResult {
  const [status, setStatus] = useState<GeocodingStatus>("idle");
  const [results, setResults] = useState<GeocodeResult[]>([]);
  const [error, setError] = useState<string | null>(null);
  // Guards against an older search's response landing after a newer one.
  const requestIdRef = useRef(0);

  const search = useCallback(
    async (query: string) => {
      if (query.trim().length === 0) return;

      const requestId = ++requestIdRef.current;
      setStatus("loading");
      setError(null);

      try {
        const found = await provider.search(query);
        if (requestIdRef.current !== requestId) return;
        setResults(found);
        setStatus("ready");
      } catch (err) {
        if (requestIdRef.current !== requestId) return;
        setResults([]);
        setError(err instanceof Error ? err.message : String(err));
        setStatus("error");
      }
    },
    [provider],
  );

  const reset = useCallback(() => {
    requestIdRef.current++;
    setStatus("idle");
    setResults([]);
    setError(null);
  }, []);

  return { status, results, error, search, reset };
}
