import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { RectangleBounds } from "@terra-globe/core";
import type { ExportPlan } from "@terra-globe/map";
import type { ExportProgress, PlanError } from "./useAreaExport.js";

const SCALE_PRESETS = [1000, 2500, 5000, 10000, 25000, 50000, 100000];
const DPI_PRESETS = [96, 150, 300];

export interface AreaExportPanelProps {
  bounds: RectangleBounds | null;
  scaleDenominator: number;
  dpi: number;
  exporting: boolean;
  progress: ExportProgress | null;
  error: string | null;
  plan: ExportPlan | null;
  planError: PlanError | null;
  onDragStart?: (e: React.MouseEvent) => void;
  onClose: () => void;
  onRedraw: () => void;
  onSetScale: (n: number) => void;
  onSetDpi: (n: number) => void;
  onExport: () => void;
}

export function AreaExportPanel({
  bounds,
  scaleDenominator,
  dpi,
  exporting,
  progress,
  error,
  plan,
  planError,
  onDragStart,
  onClose,
  onRedraw,
  onSetScale,
  onSetDpi,
  onExport,
}: AreaExportPanelProps) {
  const { t } = useTranslation();
  const [customMode, setCustomMode] = useState(!SCALE_PRESETS.includes(scaleDenominator));

  return (
    <div className="area-export-panel" aria-label={t("areaExport.panelAriaLabel")}>
      <div className="area-export-panel-header" onMouseDown={onDragStart}>
        <span>{t("areaExport.panelTitle")}</span>
        <button
          type="button"
          className="area-export-panel-close"
          onClick={onClose}
          aria-label={t("areaExport.close")}
        >
          ×
        </button>
      </div>
      <div className="area-export-panel-content">
        <label className="area-export-field">
          {t("areaExport.scaleLabel")}
          <select
            value={customMode ? "custom" : scaleDenominator}
            onChange={(e) => {
              if (e.target.value === "custom") {
                setCustomMode(true);
                return;
              }
              setCustomMode(false);
              onSetScale(Number(e.target.value));
            }}
          >
            {SCALE_PRESETS.map((preset) => (
              <option key={preset} value={preset}>
                1:{preset.toLocaleString()}
              </option>
            ))}
            <option value="custom">{t("areaExport.customScale")}</option>
          </select>
        </label>
        {customMode && (
          <label className="area-export-field">
            {t("areaExport.customScaleLabel")}
            <input
              type="number"
              min={1}
              value={scaleDenominator}
              onChange={(e) => {
                const n = Number(e.target.value);
                if (Number.isFinite(n) && n > 0) onSetScale(n);
              }}
            />
          </label>
        )}
        <label className="area-export-field">
          {t("areaExport.dpiLabel")}
          <select value={dpi} onChange={(e) => onSetDpi(Number(e.target.value))}>
            {DPI_PRESETS.map((preset) => (
              <option key={preset} value={preset}>
                {preset}
              </option>
            ))}
          </select>
        </label>
        {bounds && plan && (
          <div className="area-export-dimensions">
            {t("areaExport.dimensions", { width: plan.pixelWidth, height: plan.pixelHeight })}
            {plan.tileCount > 1 && <> · {t("areaExport.tileCount", { count: plan.tileCount })}</>}
          </div>
        )}
        {bounds && planError && (
          <div className="area-export-warning">
            {t("areaExport.tooLarge", {
              width: planError.pixelWidth,
              height: planError.pixelHeight,
              max: planError.maxDimensionPx,
              maxMp: planError.maxMegapixels,
            })}
          </div>
        )}
        {error && <div className="area-export-error">{error}</div>}
        {exporting && progress && (
          <div className="area-export-progress">
            <progress value={progress.done} max={progress.total} />
            <span>{t("areaExport.progress", { done: progress.done, total: progress.total })}</span>
          </div>
        )}
        <div className="area-export-actions">
          {bounds && (
            <button type="button" className="btn" onClick={onRedraw}>
              {t("areaExport.redraw")}
            </button>
          )}
          <button
            type="button"
            className="btn"
            disabled={!bounds || exporting || !!planError}
            onClick={onExport}
          >
            {exporting ? t("areaExport.exporting") : t("areaExport.export")}
          </button>
        </div>
      </div>
    </div>
  );
}
