import { useState } from "react";
import type { GeocodeResult } from "@webglobe/core";
import type { GeocodingStatus } from "./useGeocoding.js";

export interface AddressSearchBoxProps {
  disabled: boolean;
  status: GeocodingStatus;
  results: GeocodeResult[];
  error: string | null;
  onSearch: (query: string) => void;
  onSelectResult: (result: GeocodeResult) => void;
}

export function AddressSearchBox({
  disabled,
  status,
  results,
  error,
  onSearch,
  onSelectResult,
}: AddressSearchBoxProps) {
  const [query, setQuery] = useState("");

  return (
    <div className="address-search">
      <form
        role="search"
        onSubmit={(e) => {
          e.preventDefault();
          onSearch(query);
        }}
      >
        <label className="address-search-label">
          Address
          <input
            type="text"
            value={query}
            disabled={disabled}
            placeholder="Search for an address..."
            onChange={(e) => setQuery(e.target.value)}
          />
        </label>
        <button type="submit" className="btn" disabled={disabled || status === "loading"}>
          {status === "loading" ? "Searching..." : "Search"}
        </button>
      </form>
      {status === "error" && <div className="address-search-error">{error}</div>}
      {status === "ready" && (
        <ul className="address-search-results">
          {results.length === 0 && <li className="address-search-empty">No results</li>}
          {results.map((result, index) => (
            <li key={`${result.label}-${index}`}>
              <button
                type="button"
                className="address-search-result"
                onClick={() => {
                  setQuery("");
                  onSelectResult(result);
                }}
              >
                {result.label}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
