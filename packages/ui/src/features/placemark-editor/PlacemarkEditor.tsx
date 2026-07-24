import { useEffect, useState } from "react";
import { ConfirmModal } from "../confirm/ConfirmModal.js";
import {
  circleAreaSquareMeters,
  circleCircumferenceMeters,
  formatArea,
  formatCoordinate,
  formatDistance,
  geometryCenter,
  polygonAreaSquareMeters,
  rectangleAreaSquareMeters,
  type CoordinateFormat,
  type Placemark,
  type UnitSystem,
} from "@webglobe/core";

export interface PlacemarkStyleEdits {
  outlineEnabled: boolean;
  outlineColor: string;
  outlineWidth: number;
  filled: boolean;
  fillColor: string;
}

export interface PlacemarkEditorProps {
  placemark: Placemark;
  style: PlacemarkStyleEdits;
  unitSystem?: UnitSystem;
  coordinateFormat?: CoordinateFormat;
  onSave: (patch: { name: string; description: string; style: PlacemarkStyleEdits }) => void;
  onClose: () => void;
  onDelete: () => void;
  onDragStart?: (e: React.MouseEvent) => void;
}

function geometryMeasurements(
  geometry: Placemark["geometry"],
  unitSystem: UnitSystem,
): { label: string; value: string }[] {
  if (geometry.type === "Circle") {
    return [
      { label: "Radius", value: formatDistance(geometry.radiusMeters, unitSystem) },
      {
        label: "Circumference",
        value: formatDistance(circleCircumferenceMeters(geometry), unitSystem),
      },
      { label: "Area", value: formatArea(circleAreaSquareMeters(geometry), unitSystem) },
    ];
  }
  if (geometry.type === "Rectangle") {
    return [{ label: "Area", value: formatArea(rectangleAreaSquareMeters(geometry), unitSystem) }];
  }
  if (geometry.type === "Polygon") {
    return [{ label: "Area", value: formatArea(polygonAreaSquareMeters(geometry), unitSystem) }];
  }
  return [];
}

export function PlacemarkEditor({
  placemark,
  style,
  unitSystem = "metric",
  coordinateFormat = "decimal",
  onSave,
  onClose,
  onDelete,
  onDragStart,
}: PlacemarkEditorProps) {
  const geometryType = placemark.geometry.type;
  const isPoint = geometryType === "Point";
  const isLine = geometryType === "LineString";
  const isAreaShape = !isPoint && !isLine;
  const center = geometryCenter(placemark.geometry);
  const measurements = geometryMeasurements(placemark.geometry, unitSystem);

  const [name, setName] = useState(placemark.name);
  const [description, setDescription] = useState(placemark.description ?? "");
  const [draftStyle, setDraftStyle] = useState(style);
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  // `style` is resolved via a separate async Style lookup, so it can arrive
  // after this component has already mounted with the caller's placeholder
  // value - re-sync when it does.
  useEffect(() => {
    setDraftStyle(style);
  }, [style]);

  function patchStyle(patch: Partial<PlacemarkStyleEdits>): void {
    setDraftStyle((prev) => ({ ...prev, ...patch }));
  }

  return (
    <form
      className="placemark-editor"
      aria-label="Edit placemark"
      onSubmit={(e) => {
        e.preventDefault();
        onSave({ name, description, style: draftStyle });
      }}
    >
      <div className="placemark-editor-header" onMouseDown={onDragStart}>
        Edit Placemark
      </div>
      <div className="placemark-editor-id">ID: {placemark.id}</div>
      <label className="placemark-editor-field">
        Name
        <input value={name} onChange={(e) => setName(e.target.value)} autoFocus />
      </label>
      <label className="placemark-editor-field">
        Description
        <textarea value={description} onChange={(e) => setDescription(e.target.value)} rows={3} />
      </label>
      <div className="placemark-editor-field">
        {isPoint ? "Location" : "Center"}
        <div className="placemark-editor-coordinates">
          {formatCoordinate(center, coordinateFormat)}
        </div>
      </div>

      {measurements.length > 0 && (
        <div className="placemark-editor-field">
          Measurements
          <div className="placemark-editor-measurements">
            {measurements.map(({ label, value }) => (
              <div key={label} className="placemark-editor-measurement-row">
                <span>{label}</span>
                <span>{value}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {isPoint && (
        <label className="placemark-editor-field">
          Color
          <input
            type="color"
            value={draftStyle.fillColor}
            onChange={(e) =>
              setDraftStyle({
                outlineEnabled: true,
                outlineColor: e.target.value,
                outlineWidth: 2,
                filled: true,
                fillColor: e.target.value,
              })
            }
          />
        </label>
      )}

      {(isLine || isAreaShape) && (
        <>
          <label className="placemark-editor-checkbox-field">
            <input
              type="checkbox"
              checked={draftStyle.outlineEnabled}
              onChange={(e) => patchStyle({ outlineEnabled: e.target.checked })}
            />
            Outline
          </label>
          <label className="placemark-editor-field">
            Outline Color
            <input
              type="color"
              value={draftStyle.outlineColor}
              disabled={!draftStyle.outlineEnabled}
              onChange={(e) => patchStyle({ outlineColor: e.target.value })}
            />
          </label>
          <label className="placemark-editor-field">
            Outline Width
            <input
              type="number"
              min={0}
              step={1}
              value={draftStyle.outlineWidth}
              disabled={!draftStyle.outlineEnabled}
              onChange={(e) => patchStyle({ outlineWidth: Number(e.target.value) })}
            />
          </label>
        </>
      )}

      {isAreaShape && (
        <>
          <label className="placemark-editor-checkbox-field">
            <input
              type="checkbox"
              checked={draftStyle.filled}
              onChange={(e) => patchStyle({ filled: e.target.checked })}
            />
            Filled
          </label>
          <label className="placemark-editor-field">
            Fill Color
            <input
              type="color"
              value={draftStyle.fillColor}
              disabled={!draftStyle.filled}
              onChange={(e) => patchStyle({ fillColor: e.target.value })}
            />
          </label>
        </>
      )}

      <div className="placemark-editor-actions">
        <button type="submit" className="btn">
          Save
        </button>
        <button type="button" className="btn" onClick={onClose}>
          Close
        </button>
        <button type="button" className="btn btn-danger" onClick={() => setConfirmingDelete(true)}>
          Delete
        </button>
      </div>

      {confirmingDelete && (
        <ConfirmModal
          title="Delete placemark?"
          message={`Are you sure you want to delete "${name}"? This cannot be undone.`}
          onConfirm={() => {
            setConfirmingDelete(false);
            onDelete();
          }}
          onCancel={() => setConfirmingDelete(false)}
        />
      )}
    </form>
  );
}
