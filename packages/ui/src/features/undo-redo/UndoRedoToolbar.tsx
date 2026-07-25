import { Redo2, Undo2 } from "lucide-react";
import { useTranslation } from "react-i18next";

export interface UndoRedoToolbarProps {
  canUndo: boolean;
  canRedo: boolean;
  onUndo: () => void;
  onRedo: () => void;
}

export function UndoRedoToolbar({ canUndo, canRedo, onUndo, onRedo }: UndoRedoToolbarProps) {
  const { t } = useTranslation();
  return (
    <div role="toolbar" aria-label={t("undoRedo.ariaLabel")} className="toolbar-group">
      <button
        type="button"
        className="btn"
        disabled={!canUndo}
        aria-label={t("undoRedo.undo")}
        title={t("undoRedo.undo")}
        onClick={onUndo}
      >
        <Undo2 size={18} aria-hidden="true" />
      </button>
      <button
        type="button"
        className="btn"
        disabled={!canRedo}
        aria-label={t("undoRedo.redo")}
        title={t("undoRedo.redo")}
        onClick={onRedo}
      >
        <Redo2 size={18} aria-hidden="true" />
      </button>
    </div>
  );
}
