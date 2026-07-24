import { formatDistance, type UnitSystem } from "@webglobe/core";
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
  return (
    <div className="ruler-panel" aria-label="Ruler measurement">
      <div className="ruler-panel-header" onMouseDown={onDragStart}>
        <span>Ruler</span>
        <button type="button" className="ruler-panel-close" onClick={onClose} aria-label="Close">
          ×
        </button>
      </div>
      <div className="ruler-panel-content">
        <ul className="ruler-panel-segments">
          {segments.map((segment, i) => (
            <li key={i}>
              Segment {i + 1}: {formatDistance(segment.distanceMeters, unitSystem)}
            </li>
          ))}
        </ul>
        <div className="ruler-panel-total">Total: {formatDistance(totalMeters, unitSystem)}</div>
      </div>
    </div>
  );
}
