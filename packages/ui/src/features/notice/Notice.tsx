import { useTranslation } from "react-i18next";

export type NoticeLevel = "error" | "success";

export interface NoticeData {
  level: NoticeLevel;
  message: string;
}

export interface NoticeProps {
  notice: NoticeData;
  onDismiss: () => void;
}

export function Notice({ notice, onDismiss }: NoticeProps) {
  const { t } = useTranslation();
  return (
    <div className={`notice notice-${notice.level}`} role="alert">
      <span className="notice-message">{notice.message}</span>
      <button
        type="button"
        className="btn notice-dismiss"
        onClick={onDismiss}
        aria-label={t("notice.dismiss")}
      >
        ×
      </button>
    </div>
  );
}
