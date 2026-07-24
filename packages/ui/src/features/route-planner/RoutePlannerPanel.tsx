import { useState } from "react";
import { formatDistance, formatDuration, type UnitSystem } from "@terra-globe/core";
import type { GeocodeResult, RouteLeg } from "@terra-globe/core";
import { AddressSearchBox } from "../geocoding/AddressSearchBox.js";
import type { GeocodingStatus } from "../geocoding/useGeocoding.js";
import type { RouteStop, TravelMode } from "./useRoutePlanner.js";

export interface RoutePlannerPanelProps {
  stops: RouteStop[];
  mode: TravelMode;
  alternatives: RouteLeg[];
  selectedIndex: number;
  totalDistanceMeters: number;
  totalDurationSeconds: number;
  loading: boolean;
  error: string | null;
  unitSystem: UnitSystem;
  searchStatus: GeocodingStatus;
  searchResults: GeocodeResult[];
  searchError: string | null;
  onDragStart?: (e: React.MouseEvent) => void;
  onClose: () => void;
  onChangeMode: (mode: TravelMode) => void;
  onSearch: (query: string) => void;
  onSelectSearchResult: (result: GeocodeResult) => void;
  onRemoveStop: (index: number) => void;
  onMoveStop: (from: number, to: number) => void;
  onSelectAlternative: (index: number) => void;
}

const MODE_OPTIONS: { value: TravelMode; label: string }[] = [
  { value: "car", label: "Car" },
  { value: "foot", label: "Foot" },
  { value: "bike", label: "Bike" },
  { value: "train", label: "Train" },
  { value: "plane", label: "Plane" },
];

function stopLabel(stop: RouteStop): string {
  return stop.label ?? `${stop.point.lat.toFixed(2)}, ${stop.point.lon.toFixed(2)}`;
}

export function RoutePlannerPanel({
  stops,
  mode,
  alternatives,
  selectedIndex,
  totalDistanceMeters,
  totalDurationSeconds,
  loading,
  error,
  unitSystem,
  searchStatus,
  searchResults,
  searchError,
  onDragStart,
  onClose,
  onChangeMode,
  onSearch,
  onSelectSearchResult,
  onRemoveStop,
  onMoveStop,
  onSelectAlternative,
}: RoutePlannerPanelProps) {
  const [draggedFrom, setDraggedFrom] = useState<number | null>(null);
  const [dropTarget, setDropTarget] = useState<{
    index: number;
    position: "before" | "after";
  } | null>(null);

  return (
    <div className="route-planner-panel" aria-label="Route planner">
      <div className="route-planner-panel-header" onMouseDown={onDragStart}>
        <span>Route</span>
        <button
          type="button"
          className="route-planner-panel-close"
          onClick={onClose}
          aria-label="Close"
        >
          ×
        </button>
      </div>
      <div className="route-planner-panel-content">
        <label className="route-planner-mode-select">
          Mode
          <select value={mode} onChange={(e) => onChangeMode(e.target.value as TravelMode)}>
            {MODE_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <AddressSearchBox
          disabled={false}
          status={searchStatus}
          results={searchResults}
          error={searchError}
          onSearch={onSearch}
          onSelectResult={onSelectSearchResult}
          hideLabel
        />
        <ol className="tree route-planner-panel-stops">
          {stops.map((stop, i) => (
            <li
              key={stop.id}
              data-stop-id={stop.id}
              draggable
              className={
                dropTarget?.index === i ? `tree-row drop-${dropTarget.position}` : "tree-row"
              }
              onDragStart={() => setDraggedFrom(i)}
              onDragOver={(e) => {
                e.preventDefault();
                const rect = e.currentTarget.getBoundingClientRect();
                const position = e.clientY - rect.top < rect.height / 2 ? "before" : "after";
                setDropTarget({ index: i, position });
              }}
              onDragLeave={() =>
                setDropTarget((current) => (current?.index === i ? null : current))
              }
              onDrop={(e) => {
                e.preventDefault();
                if (draggedFrom !== null) {
                  let to = dropTarget?.position === "before" ? i : i + 1;
                  if (to > draggedFrom) to -= 1;
                  if (to !== draggedFrom) onMoveStop(draggedFrom, to);
                }
                setDraggedFrom(null);
                setDropTarget(null);
              }}
              onDragEnd={() => {
                setDraggedFrom(null);
                setDropTarget(null);
              }}
            >
              <span>
                Stop {i + 1}: {stopLabel(stop)}
              </span>
              <button
                type="button"
                className="btn"
                disabled={i === 0}
                onClick={() => onMoveStop(i, i - 1)}
                aria-label={`Move stop ${i + 1} up`}
              >
                ↑
              </button>
              <button
                type="button"
                className="btn"
                disabled={i === stops.length - 1}
                onClick={() => onMoveStop(i, i + 1)}
                aria-label={`Move stop ${i + 1} down`}
              >
                ↓
              </button>
              <button
                type="button"
                className="btn btn-danger"
                onClick={() => onRemoveStop(i)}
                aria-label={`Remove stop ${i + 1}`}
              >
                ×
              </button>
            </li>
          ))}
        </ol>
        {loading && <div className="route-planner-panel-status">Calculating route...</div>}
        {error && <div className="route-planner-panel-error">{error}</div>}
        {!loading && !error && alternatives.length > 0 && (
          <>
            <div className="route-planner-panel-total">
              {formatDistance(totalDistanceMeters, unitSystem)} ·{" "}
              {formatDuration(totalDurationSeconds)}
            </div>
            {alternatives.length > 1 && (
              <ul className="route-planner-panel-alternatives">
                {alternatives.map((alt, i) => (
                  <li key={i}>
                    <button
                      type="button"
                      className="btn"
                      aria-pressed={i === selectedIndex}
                      onClick={() => onSelectAlternative(i)}
                    >
                      Route {i + 1}: {formatDistance(alt.distanceMeters, unitSystem)} ·{" "}
                      {formatDuration(alt.durationSeconds)}
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </>
        )}
      </div>
    </div>
  );
}
