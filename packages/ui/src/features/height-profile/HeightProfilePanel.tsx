import { computeTrackProfile, type LineStringGeometry, type UnitSystem } from "@webglobe/core";
import { HeightProfileChart } from "./HeightProfileChart.js";

export interface HeightProfilePanelProps {
  trackName: string;
  geometry: LineStringGeometry;
  unitSystem: UnitSystem;
  width: number;
  height: number;
  onDragStart?: (e: React.MouseEvent) => void;
  onClose: () => void;
}

const HEADER_HEIGHT = 32;
const CONTENT_PADDING = 12;

export function HeightProfilePanel({
  trackName,
  geometry,
  unitSystem,
  width,
  height,
  onDragStart,
  onClose,
}: HeightProfilePanelProps) {
  const profile = computeTrackProfile(geometry);
  const chartWidth = Math.max(1, width - CONTENT_PADDING * 2);
  const chartHeight = Math.max(1, height - HEADER_HEIGHT - CONTENT_PADDING * 2);

  return (
    <div className="height-profile" aria-label="Elevation profile">
      <div className="height-profile-header" onMouseDown={onDragStart}>
        <span>Elevation Profile — {trackName}</span>
        <button type="button" className="height-profile-close" onClick={onClose} aria-label="Close">
          ×
        </button>
      </div>
      <div className="height-profile-content">
        <HeightProfileChart
          profile={profile}
          unitSystem={unitSystem}
          width={chartWidth}
          height={chartHeight}
        />
      </div>
    </div>
  );
}
