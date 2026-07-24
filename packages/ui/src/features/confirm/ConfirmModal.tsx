export interface ConfirmModalProps {
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  onConfirm: () => void;
  onCancel: () => void;
  /** Optional third action (e.g. "Save") shown before Confirm/Cancel. */
  extraLabel?: string;
  onExtra?: () => void;
}

export function ConfirmModal({
  title,
  message,
  confirmLabel = "Delete",
  cancelLabel = "Cancel",
  onConfirm,
  onCancel,
  extraLabel,
  onExtra,
}: ConfirmModalProps) {
  return (
    <div className="modal-overlay" onClick={onCancel}>
      <div
        className="modal-panel confirm-modal"
        role="alertdialog"
        aria-modal="true"
        aria-label={title}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="confirm-modal-title">{title}</div>
        <p className="confirm-modal-message">{message}</p>
        <div className="confirm-modal-actions">
          {extraLabel && onExtra && (
            <button type="button" className="btn" onClick={onExtra}>
              {extraLabel}
            </button>
          )}
          <button type="button" className="btn btn-danger" onClick={onConfirm}>
            {confirmLabel}
          </button>
          <button type="button" className="btn" onClick={onCancel}>
            {cancelLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
