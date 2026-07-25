import { useTranslation } from "react-i18next";
import { formatDistance, type UnitSystem } from "@terra-globe/core";
import type { RulerSegment } from "./useRuler.js";

export interface RulerPanelProps {
  segments: RulerSegment[];
  totalMeters: number;
  unitSystem: UnitSystem;
  onDragStart?: (e: React.MouseEvent) => void;
  onClose: () => void;
}

export function RulerPanel({
  segments,
  totalMeters,
  unitSystem,
  onDragStart,
  onClose,
}: RulerPanelProps) {
  const { t } = useTranslation();
  return (
    <div className="ruler-panel" aria-label={t("ruler.panelAriaLabel")}>
      <div className="ruler-panel-header" onMouseDown={onDragStart}>
        <span>{t("ruler.panelTitle")}</span>
        <button
          type="button"
          className="ruler-panel-close"
          onClick={onClose}
          aria-label={t("ruler.close")}
        >
          ×
        </button>
      </div>
      <div className="ruler-panel-content">
        <ul className="ruler-panel-segments">
          {segments.map((segment, i) => (
            <li key={i}>
              {t("ruler.segment", {
                n: i + 1,
                distance: formatDistance(segment.distanceMeters, unitSystem),
              })}
            </li>
          ))}
        </ul>
        <div className="ruler-panel-total">
          {t("ruler.total", { distance: formatDistance(totalMeters, unitSystem) })}
        </div>
      </div>
    </div>
  );
}
