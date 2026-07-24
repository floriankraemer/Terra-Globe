import { useEffect, useRef, useState } from "react";
import { ConfirmModal } from "../confirm/ConfirmModal.js";
import {
  circleAreaSquareMeters,
  circleCircumferenceMeters,
  formatArea,
  formatCoordinate,
  formatDistance,
  geometryCenter,
  hasElevationData,
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
  /** Called on every draft change (and once more, with the original values, on unsaved close/unmount) so the live map entity can preview edits before Save. */
  onPreview?: (patch: { name: string; style: PlacemarkStyleEdits }) => void;
  onShowElevationProfile?: () => void;
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
  onPreview,
  onShowElevationProfile,
}: PlacemarkEditorProps) {
  const geometryType = placemark.geometry.type;
  const isPoint = geometryType === "Point";
  const isLine = geometryType === "LineString";
  const isAreaShape = !isPoint && !isLine;
  const center = geometryCenter(placemark.geometry);
  const measurements = geometryMeasurements(placemark.geometry, unitSystem);
  const showElevationButton = hasElevationData(placemark.geometry);

  const [name, setName] = useState(placemark.name);
  const [description, setDescription] = useState(placemark.description ?? "");
  const [draftStyle, setDraftStyle] = useState(style);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [confirmingClose, setConfirmingClose] = useState(false);

  // `style` is resolved via a separate async Style lookup, so it can arrive
  // after this component has already mounted with the caller's placeholder
  // value - re-sync when it does.
  useEffect(() => {
    setDraftStyle(style);
  }, [style]);

  function patchStyle(patch: Partial<PlacemarkStyleEdits>): void {
    setDraftStyle((prev) => ({ ...prev, ...patch }));
  }

  const onPreviewRef = useRef(onPreview);
  onPreviewRef.current = onPreview;

  // Last-persisted name/style, refreshed every render so it stays correct
  // even if the async `style` prop above resolves after mount.
  const originalRef = useRef({ name: placemark.name, style });
  originalRef.current = { name: placemark.name, style };

  // A successful Save/Delete already leaves the live entity in the correct
  // state (new persisted style, or removed) - skip the unmount revert then.
  const suppressRevertRef = useRef(false);

  // Preview every draft change on the live map entity. `description` has no
  // visual representation on the entity, so it's intentionally left out -
  // its "revert on close" is already free since it's never pushed anywhere
  // until Save.
  useEffect(() => {
    onPreviewRef.current?.({ name, style: draftStyle });
  }, [name, draftStyle]);

  useEffect(() => {
    return () => {
      if (!suppressRevertRef.current) onPreviewRef.current?.(originalRef.current);
    };
  }, []);

  const isDirty =
    name !== placemark.name ||
    description !== (placemark.description ?? "") ||
    draftStyle.outlineEnabled !== style.outlineEnabled ||
    draftStyle.outlineColor !== style.outlineColor ||
    draftStyle.outlineWidth !== style.outlineWidth ||
    draftStyle.filled !== style.filled ||
    draftStyle.fillColor !== style.fillColor;

  function requestClose(): void {
    if (isDirty) {
      setConfirmingClose(true);
    } else {
      onClose();
    }
  }

  return (
    <form
      className="placemark-editor"
      aria-label="Edit placemark"
      onSubmit={(e) => {
        e.preventDefault();
        suppressRevertRef.current = true;
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

      {showElevationButton && onShowElevationProfile && (
        <button
          type="button"
          className="btn placemark-editor-elevation-button"
          onClick={onShowElevationProfile}
        >
          Elevation Profile
        </button>
      )}

      <div className="placemark-editor-actions">
        <button type="submit" className="btn">
          Save
        </button>
        <button type="button" className="btn" onClick={requestClose}>
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
            suppressRevertRef.current = true;
            setConfirmingDelete(false);
            onDelete();
          }}
          onCancel={() => setConfirmingDelete(false)}
        />
      )}

      {confirmingClose && (
        <ConfirmModal
          title="Unsaved changes"
          message={`Save changes to "${name}" before closing?`}
          confirmLabel="Discard"
          cancelLabel="Cancel"
          extraLabel="Save"
          onExtra={() => {
            setConfirmingClose(false);
            suppressRevertRef.current = true;
            onSave({ name, description, style: draftStyle });
          }}
          onConfirm={() => {
            setConfirmingClose(false);
            onClose();
          }}
          onCancel={() => setConfirmingClose(false)}
        />
      )}
    </form>
  );
}
