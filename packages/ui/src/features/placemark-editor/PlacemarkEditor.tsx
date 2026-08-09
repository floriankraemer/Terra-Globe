import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ConfirmModal } from "../confirm/ConfirmModal.js";
import { MARKER_ICON_OPTIONS } from "./markerIconOptions.js";
import {
  circleAreaSquareMeters,
  circleCircumferenceMeters,
  distanceUnitLabel,
  formatArea,
  formatCoordinate,
  formatDistance,
  geometryCenter,
  hasElevationData,
  metersToUnit,
  polygonAreaSquareMeters,
  rectangleAreaSquareMeters,
  unitToMeters,
  type CircleGeometry,
  type CoordinateFormat,
  type MarkerIconId,
  type Placemark,
  type PlacemarkGeometry,
  type UnitSystem,
} from "@terra-globe/core";

export interface PlacemarkStyleEdits {
  outlineEnabled: boolean;
  outlineColor: string;
  outlineWidth: number;
  filled: boolean;
  fillColor: string;
  markerIcon?: MarkerIconId;
}

export interface PlacemarkEditorProps {
  placemark: Placemark;
  style: PlacemarkStyleEdits;
  unitSystem?: UnitSystem;
  coordinateFormat?: CoordinateFormat;
  onSave: (patch: {
    name: string;
    description: string;
    style: PlacemarkStyleEdits;
    visibility: boolean;
    geometry: PlacemarkGeometry;
  }) => void;
  onClose: () => void;
  onDelete: () => void;
  onDragStart?: (e: React.MouseEvent) => void;
  /** Called on every draft change (and once more, with the original values, on unsaved close/unmount) so the live map entity can preview edits before Save. */
  onPreview?: (patch: {
    name: string;
    style: PlacemarkStyleEdits;
    visibility: boolean;
    geometry: PlacemarkGeometry;
  }) => void;
  onShowElevationProfile?: () => void;
  /** Session-only distance rings around a Point marker - never persisted, see useDistanceRings. */
  onDistanceRingsChange?: (
    rings: { spacingMeters: number; discRadiusMeters: number } | null,
  ) => void;
}

function geometryMeasurements(
  geometry: Placemark["geometry"],
  unitSystem: UnitSystem,
  t: (key: string) => string,
): { label: string; value: string }[] {
  if (geometry.type === "Circle") {
    return [
      {
        label: t("placemarkEditor.circumference"),
        value: formatDistance(circleCircumferenceMeters(geometry), unitSystem),
      },
      {
        label: t("placemarkEditor.area"),
        value: formatArea(circleAreaSquareMeters(geometry), unitSystem),
      },
    ];
  }
  if (geometry.type === "Rectangle") {
    return [
      {
        label: t("placemarkEditor.area"),
        value: formatArea(rectangleAreaSquareMeters(geometry), unitSystem),
      },
    ];
  }
  if (geometry.type === "Polygon") {
    return [
      {
        label: t("placemarkEditor.area"),
        value: formatArea(polygonAreaSquareMeters(geometry), unitSystem),
      },
    ];
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
  onDistanceRingsChange,
}: PlacemarkEditorProps) {
  const { t } = useTranslation();
  const geometryType = placemark.geometry.type;
  const isPoint = geometryType === "Point";
  const isLine = geometryType === "LineString";
  const isAreaShape = !isPoint && !isLine;
  const center = geometryCenter(placemark.geometry);
  const measurements = geometryMeasurements(placemark.geometry, unitSystem, t);
  const showElevationButton = hasElevationData(placemark.geometry);

  const [name, setName] = useState(placemark.name);
  const [description, setDescription] = useState(placemark.description ?? "");
  const [draftStyle, setDraftStyle] = useState(style);
  const [visible, setVisible] = useState(placemark.visibility);
  const [draftRadiusMeters, setDraftRadiusMeters] = useState(
    geometryType === "Circle" ? (placemark.geometry as CircleGeometry).radiusMeters : 0,
  );
  // Decoupled from draftRadiusMeters so an in-progress edit (e.g. clearing
  // the field to type a new value) isn't snapped back to the last valid
  // number on every keystroke - only committed to draftRadiusMeters once it
  // parses to a positive number.
  const [radiusText, setRadiusText] = useState(() =>
    String(metersToUnit(draftRadiusMeters, unitSystem)),
  );
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [confirmingClose, setConfirmingClose] = useState(false);

  // Session-only overlay, never persisted (see PlacemarkEditorProps.onDistanceRingsChange)
  // - always starts off, resetting whenever the editor remounts for a new placemark.
  const [ringsEnabled, setRingsEnabled] = useState(false);
  const [ringSpacingMeters, setRingSpacingMeters] = useState(100);
  const [ringDiscRadiusMeters, setRingDiscRadiusMeters] = useState(500);
  const [ringSpacingText, setRingSpacingText] = useState(() =>
    String(metersToUnit(ringSpacingMeters, unitSystem)),
  );
  const [ringDiscRadiusText, setRingDiscRadiusText] = useState(() =>
    String(metersToUnit(ringDiscRadiusMeters, unitSystem)),
  );

  const draftGeometry: PlacemarkGeometry =
    geometryType === "Circle"
      ? { ...(placemark.geometry as CircleGeometry), radiusMeters: draftRadiusMeters }
      : placemark.geometry;

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
  const originalRef = useRef({
    name: placemark.name,
    style,
    visibility: placemark.visibility,
    geometry: placemark.geometry,
  });
  originalRef.current = {
    name: placemark.name,
    style,
    visibility: placemark.visibility,
    geometry: placemark.geometry,
  };

  // A successful Save/Delete already leaves the live entity in the correct
  // state (new persisted style, or removed) - skip the unmount revert then.
  const suppressRevertRef = useRef(false);

  // Preview every draft change on the live map entity. `description` has no
  // visual representation on the entity, so it's intentionally left out -
  // its "revert on close" is already free since it's never pushed anywhere
  // until Save.
  useEffect(() => {
    onPreviewRef.current?.({
      name,
      style: draftStyle,
      visibility: visible,
      geometry: draftGeometry,
    });
  }, [name, draftStyle, visible, draftGeometry]);

  useEffect(() => {
    return () => {
      if (!suppressRevertRef.current) onPreviewRef.current?.(originalRef.current);
    };
  }, []);

  // Distance rings are a separate, session-only overlay - kept entirely out
  // of onPreview/draftGeometry/draftStyle/isDirty/onSave so they never leak
  // into a Save or KML export.
  const onDistanceRingsChangeRef = useRef(onDistanceRingsChange);
  onDistanceRingsChangeRef.current = onDistanceRingsChange;

  useEffect(() => {
    onDistanceRingsChangeRef.current?.(
      ringsEnabled
        ? { spacingMeters: ringSpacingMeters, discRadiusMeters: ringDiscRadiusMeters }
        : null,
    );
  }, [ringsEnabled, ringSpacingMeters, ringDiscRadiusMeters]);

  useEffect(() => {
    return () => {
      onDistanceRingsChangeRef.current?.(null);
    };
  }, []);

  const isDirty =
    name !== placemark.name ||
    description !== (placemark.description ?? "") ||
    draftStyle.outlineEnabled !== style.outlineEnabled ||
    draftStyle.outlineColor !== style.outlineColor ||
    draftStyle.outlineWidth !== style.outlineWidth ||
    draftStyle.filled !== style.filled ||
    draftStyle.fillColor !== style.fillColor ||
    draftStyle.markerIcon !== style.markerIcon ||
    visible !== placemark.visibility ||
    (geometryType === "Circle" &&
      draftRadiusMeters !== (placemark.geometry as CircleGeometry).radiusMeters);

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
      aria-label={t("placemarkEditor.ariaLabel")}
      onSubmit={(e) => {
        e.preventDefault();
        suppressRevertRef.current = true;
        onSave({
          name,
          description,
          style: draftStyle,
          visibility: visible,
          geometry: draftGeometry,
        });
      }}
    >
      <div className="placemark-editor-header" onMouseDown={onDragStart}>
        {t("placemarkEditor.header")}
      </div>
      <div className="placemark-editor-id">{t("placemarkEditor.id", { id: placemark.id })}</div>
      <label className="placemark-editor-field">
        {t("placemarkEditor.name")}
        <input value={name} onChange={(e) => setName(e.target.value)} autoFocus />
      </label>
      <label className="placemark-editor-field">
        {t("placemarkEditor.description")}
        <textarea value={description} onChange={(e) => setDescription(e.target.value)} rows={3} />
      </label>
      <label className="placemark-editor-checkbox-field">
        <input type="checkbox" checked={visible} onChange={(e) => setVisible(e.target.checked)} />
        {t("placemarkEditor.visible")}
      </label>
      <div className="placemark-editor-field">
        {isPoint ? t("placemarkEditor.location") : t("placemarkEditor.center")}
        <div className="placemark-editor-coordinates">
          {formatCoordinate(center, coordinateFormat)}
        </div>
      </div>

      {geometryType === "Circle" && (
        <div className="placemark-editor-field">
          <label htmlFor="placemark-editor-radius">{t("placemarkEditor.radius")}</label>
          <div className="placemark-editor-radius-input">
            <input
              id="placemark-editor-radius"
              type="number"
              min={0.01}
              step="any"
              value={radiusText}
              onChange={(e) => {
                setRadiusText(e.target.value);
                const next = Number(e.target.value);
                if (Number.isFinite(next) && next > 0) {
                  setDraftRadiusMeters(unitToMeters(next, unitSystem));
                }
              }}
            />
            <span>{distanceUnitLabel(unitSystem)}</span>
          </div>
        </div>
      )}

      {measurements.length > 0 && (
        <div className="placemark-editor-field">
          {t("placemarkEditor.measurements")}
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
        <>
          <label className="placemark-editor-field">
            {t("placemarkEditor.color")}
            <input
              type="color"
              value={draftStyle.fillColor}
              onChange={(e) =>
                patchStyle({
                  outlineEnabled: true,
                  outlineColor: e.target.value,
                  outlineWidth: 2,
                  filled: true,
                  fillColor: e.target.value,
                })
              }
            />
          </label>
          <div className="placemark-editor-field">
            {t("placemarkEditor.marker")}
            <div
              className="placemark-editor-marker-icons"
              role="radiogroup"
              aria-label={t("placemarkEditor.markerIconAriaLabel")}
            >
              {MARKER_ICON_OPTIONS.map(({ id, Icon }) => (
                <button
                  key={id}
                  type="button"
                  role="radio"
                  aria-checked={draftStyle.markerIcon === id}
                  aria-label={id}
                  className={
                    "placemark-editor-marker-icon-button" +
                    (draftStyle.markerIcon === id ? " selected" : "")
                  }
                  onClick={() => patchStyle({ markerIcon: id })}
                >
                  <Icon size={18} aria-hidden="true" />
                </button>
              ))}
            </div>
          </div>
          <label className="placemark-editor-checkbox-field">
            <input
              type="checkbox"
              checked={ringsEnabled}
              onChange={(e) => setRingsEnabled(e.target.checked)}
            />
            {t("placemarkEditor.showDistanceRings")}
          </label>
          {ringsEnabled && (
            <div className="placemark-editor-field-row">
              <div className="placemark-editor-field">
                <label htmlFor="placemark-editor-ring-spacing">
                  {t("placemarkEditor.ringSpacing")}
                </label>
                <div className="placemark-editor-radius-input">
                  <input
                    id="placemark-editor-ring-spacing"
                    type="number"
                    min={0.01}
                    step="any"
                    value={ringSpacingText}
                    onChange={(e) => {
                      setRingSpacingText(e.target.value);
                      const next = Number(e.target.value);
                      if (Number.isFinite(next) && next > 0) {
                        setRingSpacingMeters(unitToMeters(next, unitSystem));
                      }
                    }}
                  />
                  <span>{distanceUnitLabel(unitSystem)}</span>
                </div>
              </div>
              <div className="placemark-editor-field">
                <label htmlFor="placemark-editor-ring-disc-radius">
                  {t("placemarkEditor.discRadius")}
                </label>
                <div className="placemark-editor-radius-input">
                  <input
                    id="placemark-editor-ring-disc-radius"
                    type="number"
                    min={0.01}
                    step="any"
                    value={ringDiscRadiusText}
                    onChange={(e) => {
                      setRingDiscRadiusText(e.target.value);
                      const next = Number(e.target.value);
                      if (Number.isFinite(next) && next > 0) {
                        setRingDiscRadiusMeters(unitToMeters(next, unitSystem));
                      }
                    }}
                  />
                  <span>{distanceUnitLabel(unitSystem)}</span>
                </div>
              </div>
            </div>
          )}
        </>
      )}

      {(isLine || isAreaShape) && (
        <>
          <label className="placemark-editor-checkbox-field">
            <input
              type="checkbox"
              checked={draftStyle.outlineEnabled}
              onChange={(e) => patchStyle({ outlineEnabled: e.target.checked })}
            />
            {t("placemarkEditor.outline")}
          </label>
          <label className="placemark-editor-field">
            {t("placemarkEditor.outlineColor")}
            <input
              type="color"
              value={draftStyle.outlineColor}
              disabled={!draftStyle.outlineEnabled}
              onChange={(e) => patchStyle({ outlineColor: e.target.value })}
            />
          </label>
          <label className="placemark-editor-field">
            {t("placemarkEditor.outlineWidth")}
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
            {t("placemarkEditor.filled")}
          </label>
          <label className="placemark-editor-field">
            {t("placemarkEditor.fillColor")}
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
          {t("placemarkEditor.elevationProfile")}
        </button>
      )}

      <div className="placemark-editor-actions">
        <button type="submit" className="btn">
          {t("placemarkEditor.save")}
        </button>
        <button type="button" className="btn" onClick={requestClose}>
          {t("placemarkEditor.close")}
        </button>
        <button type="button" className="btn btn-danger" onClick={() => setConfirmingDelete(true)}>
          {t("placemarkEditor.delete")}
        </button>
      </div>

      {confirmingDelete && (
        <ConfirmModal
          title={t("confirm.deletePlacemarkTitle")}
          message={t("confirm.deletePlacemarkMessage", { name })}
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
          title={t("confirm.unsavedChangesTitle")}
          message={t("confirm.unsavedChangesMessage", { name })}
          confirmLabel={t("confirm.discard")}
          cancelLabel={t("common.cancel")}
          extraLabel={t("confirm.save")}
          onExtra={() => {
            setConfirmingClose(false);
            suppressRevertRef.current = true;
            onSave({
              name,
              description,
              style: draftStyle,
              visibility: visible,
              geometry: draftGeometry,
            });
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
