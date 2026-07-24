export type DrawingTool = "point" | "rectangle" | "circle" | "polygon" | "line";
export type DrawingMode = DrawingTool | "idle";

export interface DrawingToolbarProps {
  mode: DrawingMode;
  disabled: boolean;
  onSelectTool: (tool: DrawingTool) => void;
  onFinish: () => void;
  onCancel: () => void;
}

const GEOMETRY_TOOLS: { tool: DrawingTool; label: string }[] = [
  { tool: "rectangle", label: "Rectangle" },
  { tool: "circle", label: "Circle" },
  { tool: "polygon", label: "Polygon" },
  { tool: "line", label: "Line" },
];

export function DrawingToolbar({
  mode,
  disabled,
  onSelectTool,
  onFinish,
  onCancel,
}: DrawingToolbarProps) {
  const geometryValue = GEOMETRY_TOOLS.some((t) => t.tool === mode) ? mode : "";

  return (
    <div role="toolbar" aria-label="Drawing tools" className="toolbar-group">
      <button
        type="button"
        className="btn"
        aria-pressed={mode === "point"}
        disabled={disabled}
        onClick={() => onSelectTool("point")}
      >
        Marker
      </button>
      <label className="base-layer-select">
        Geometry
        <select
          value={geometryValue}
          disabled={disabled}
          onChange={(e) => onSelectTool(e.target.value as DrawingTool)}
        >
          <option value="" disabled>
            Select shape
          </option>
          {GEOMETRY_TOOLS.map(({ tool, label }) => (
            <option key={tool} value={tool}>
              {label}
            </option>
          ))}
        </select>
      </label>
      {(mode === "polygon" || mode === "line") && (
        <button type="button" className="btn" onClick={onFinish}>
          Finish
        </button>
      )}
      {mode !== "idle" && (
        <button type="button" className="btn btn-danger" onClick={onCancel}>
          Cancel
        </button>
      )}
    </div>
  );
}
