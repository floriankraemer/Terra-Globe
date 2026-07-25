import { useTranslation } from "react-i18next";

export type DrawingTool = "point" | "rectangle" | "circle" | "polygon" | "line";
export type DrawingMode = DrawingTool | "idle";

export interface DrawingToolbarProps {
  mode: DrawingMode;
  disabled: boolean;
  onSelectTool: (tool: DrawingTool) => void;
  onFinish: () => void;
  onCancel: () => void;
}

export function DrawingToolbar({
  mode,
  disabled,
  onSelectTool,
  onFinish,
  onCancel,
}: DrawingToolbarProps) {
  const { t } = useTranslation();
  const geometryTools: { tool: DrawingTool; label: string }[] = [
    { tool: "rectangle", label: t("drawingToolbar.rectangle") },
    { tool: "circle", label: t("drawingToolbar.circle") },
    { tool: "polygon", label: t("drawingToolbar.polygon") },
    { tool: "line", label: t("drawingToolbar.line") },
  ];
  const geometryValue = geometryTools.some((entry) => entry.tool === mode) ? mode : "";

  return (
    <div role="toolbar" aria-label={t("drawingToolbar.ariaLabel")} className="toolbar-group">
      <button
        type="button"
        className="btn"
        aria-pressed={mode === "point"}
        disabled={disabled}
        onClick={() => onSelectTool("point")}
      >
        {t("drawingToolbar.marker")}
      </button>
      <label className="base-layer-select">
        {t("drawingToolbar.geometryLabel")}
        <select
          value={geometryValue}
          disabled={disabled}
          onChange={(e) => onSelectTool(e.target.value as DrawingTool)}
        >
          <option value="" disabled>
            {t("drawingToolbar.selectShape")}
          </option>
          {geometryTools.map(({ tool, label }) => (
            <option key={tool} value={tool}>
              {label}
            </option>
          ))}
        </select>
      </label>
      {(mode === "polygon" || mode === "line") && (
        <button type="button" className="btn" onClick={onFinish}>
          {t("drawingToolbar.finish")}
        </button>
      )}
      {mode !== "idle" && (
        <button type="button" className="btn btn-danger" onClick={onCancel}>
          {t("drawingToolbar.cancel")}
        </button>
      )}
    </div>
  );
}
