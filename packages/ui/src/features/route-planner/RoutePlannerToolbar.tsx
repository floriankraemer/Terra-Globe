import { useTranslation } from "react-i18next";

export interface RoutePlannerToolbarProps {
  active: boolean;
  disabled: boolean;
  onToggle: () => void;
}

export function RoutePlannerToolbar({ active, disabled, onToggle }: RoutePlannerToolbarProps) {
  const { t } = useTranslation();
  return (
    <div role="toolbar" aria-label={t("routePlanner.ariaLabel")} className="toolbar-group">
      <button
        type="button"
        className="btn"
        aria-pressed={active}
        disabled={disabled}
        onClick={onToggle}
      >
        {t("routePlanner.toggle")}
      </button>
    </div>
  );
}
