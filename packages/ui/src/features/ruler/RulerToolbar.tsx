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
  return (
    <div role="toolbar" aria-label="Ruler" className="toolbar-group">
      <button
        type="button"
        className="btn"
        aria-pressed={active}
        disabled={disabled}
        onClick={onStart}
      >
        Ruler
      </button>
      {active && vertexCount > 0 && (
        <button type="button" className="btn" onClick={onUndo}>
          Undo
        </button>
      )}
      {active && vertexCount >= 2 && (
        <button type="button" className="btn" onClick={onFinish}>
          Finish
        </button>
      )}
      {active && (
        <button type="button" className="btn btn-danger" onClick={onCancel}>
          Cancel
        </button>
      )}
    </div>
  );
}
