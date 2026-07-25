import { useTranslation } from "react-i18next";

export interface RulerToolbarProps {
  active: boolean;
  disabled: boolean;
  vertexCount: number;
  onStart: () => void;
  onUndo: () => void;
  onFinish: () => void;
  onCancel: () => void;
}

export function RulerToolbar({
  active,
  disabled,
  vertexCount,
  onStart,
  onUndo,
  onFinish,
  onCancel,
}: RulerToolbarProps) {
  const { t } = useTranslation();
  return (
    <div role="toolbar" aria-label={t("ruler.ariaLabel")} className="toolbar-group">
      <button
        type="button"
        className="btn"
        aria-pressed={active}
        disabled={disabled}
        onClick={onStart}
      >
        {t("ruler.start")}
      </button>
      {active && vertexCount > 0 && (
        <button type="button" className="btn" onClick={onUndo}>
          {t("ruler.undo")}
        </button>
      )}
      {active && vertexCount >= 2 && (
        <button type="button" className="btn" onClick={onFinish}>
          {t("ruler.finish")}
        </button>
      )}
      {active && (
        <button type="button" className="btn btn-danger" onClick={onCancel}>
          {t("ruler.cancel")}
        </button>
      )}
    </div>
  );
}
