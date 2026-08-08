import { useTranslation } from "react-i18next";
import { MenuButton } from "../../components/MenuButton.js";

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
  const activeGeometry = geometryTools.find((entry) => entry.tool === mode);

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
      <MenuButton
        label={`${t("drawingToolbar.geometryLabel")}: ${activeGeometry?.label ?? t("drawingToolbar.selectShape")}`}
        disabled={disabled}
      >
        {(close) =>
          geometryTools.map(({ tool, label }) => (
            <button
              key={tool}
              type="button"
              role="menuitemradio"
              className="menu-item"
              aria-checked={mode === tool}
              onClick={() => {
                onSelectTool(tool);
                close();
              }}
            >
              {label}
            </button>
          ))
        }
      </MenuButton>
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
