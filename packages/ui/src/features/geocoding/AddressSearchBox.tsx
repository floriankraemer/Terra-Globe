import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { GeocodeResult } from "@terra-globe/core";
import type { GeocodingStatus } from "./useGeocoding.js";

export interface AddressSearchBoxProps {
  disabled: boolean;
  status: GeocodingStatus;
  results: GeocodeResult[];
  error: string | null;
  onSearch: (query: string) => void;
  onSelectResult: (result: GeocodeResult) => void;
  /** Keeps the "Address" text in the accessibility tree but hides it visually. */
  hideLabel?: boolean;
}

export function AddressSearchBox({
  disabled,
  status,
  results,
  error,
  onSearch,
  onSelectResult,
  hideLabel,
}: AddressSearchBoxProps) {
  const { t } = useTranslation();
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
          <span className={hideLabel ? "visually-hidden" : undefined}>
            {t("geocoding.addressLabel")}
          </span>
          <input
            type="text"
            value={query}
            disabled={disabled}
            placeholder={t("geocoding.placeholder")}
            onChange={(e) => setQuery(e.target.value)}
          />
        </label>
        <button type="submit" className="btn" disabled={disabled || status === "loading"}>
          {status === "loading" ? t("geocoding.searching") : t("geocoding.search")}
        </button>
      </form>
      {status === "error" && <div className="address-search-error">{error}</div>}
      {status === "ready" && (
        <ul className="address-search-results">
          {results.length === 0 && (
            <li className="address-search-empty">{t("geocoding.noResults")}</li>
          )}
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
