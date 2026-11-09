import { useTranslation } from "react-i18next";

export interface AreaExportToolbarProps {
  active: boolean;
  disabled: boolean;
  onStart: () => void;
  onCancel: () => void;
}

export function AreaExportToolbar({ active, disabled, onStart, onCancel }: AreaExportToolbarProps) {
  const { t } = useTranslation();
  return (
    <div role="toolbar" aria-label={t("areaExport.ariaLabel")} className="toolbar-group">
      <button
        type="button"
        className="btn"
        aria-pressed={active}
        disabled={disabled}
        onClick={onStart}
      >
        {t("areaExport.start")}
      </button>
      {active && (
        <button type="button" className="btn btn-danger" onClick={onCancel}>
          {t("areaExport.cancel")}
        </button>
      )}
    </div>
  );
}
