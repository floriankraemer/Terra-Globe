import { useTranslation } from "react-i18next";
import { computeTrackProfile, type LineStringGeometry, type UnitSystem } from "@terra-globe/core";
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
  const { t } = useTranslation();
  const profile = computeTrackProfile(geometry);
  const chartWidth = Math.max(1, width - CONTENT_PADDING * 2);
  const chartHeight = Math.max(1, height - HEADER_HEIGHT - CONTENT_PADDING * 2);

  return (
    <div className="height-profile" aria-label={t("heightProfile.ariaLabel")}>
      <div className="height-profile-header" onMouseDown={onDragStart}>
        <span>{t("heightProfile.title", { name: trackName })}</span>
        <button
          type="button"
          className="height-profile-close"
          onClick={onClose}
          aria-label={t("heightProfile.close")}
        >
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
