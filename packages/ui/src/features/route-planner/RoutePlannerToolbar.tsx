export interface RoutePlannerToolbarProps {
  active: boolean;
  disabled: boolean;
  waypointCount: number;
  onStart: () => void;
  onFinish: () => void;
  onCancel: () => void;
}

export function RoutePlannerToolbar({
  active,
  disabled,
  waypointCount,
  onStart,
  onFinish,
  onCancel,
}: RoutePlannerToolbarProps) {
  return (
    <div role="toolbar" aria-label="Route planner" className="toolbar-group">
      <button
        type="button"
        className="btn"
        aria-pressed={active}
        disabled={disabled}
        onClick={onStart}
      >
        Route
      </button>
      {active && (
        <button type="button" className="btn" onClick={onFinish}>
          Finish
        </button>
      )}
      {(active || waypointCount > 0) && (
        <button type="button" className="btn btn-danger" onClick={onCancel}>
          Cancel
        </button>
      )}
    </div>
  );
}
