import { useTranslation } from "react-i18next";

const APP_NAME = "Terra Globe";
const AUTHOR = "Florian Krämer";
const LICENSE_NAME = "GNU General Public License v3.0";
const GITHUB_URL = "https://github.com/floriankraemer/Terra-Globe";
const ISSUES_URL = "https://github.com/floriankraemer/Terra-Globe/issues";

export interface AboutModalProps {
  onClose: () => void;
}

export function AboutModal({ onClose }: AboutModalProps) {
  const { t } = useTranslation();

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="modal-panel about-modal"
        role="dialog"
        aria-modal="true"
        aria-label={t("about.title", { name: APP_NAME })}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="about-modal-title">{t("about.title", { name: APP_NAME })}</div>
        <p className="about-modal-line">{t("about.version", { version: __APP_VERSION__ })}</p>
        <p className="about-modal-line">{t("about.author", { author: AUTHOR })}</p>
        <p className="about-modal-line">{t("about.license", { license: LICENSE_NAME })}</p>
        <div className="about-modal-links">
          <a href={GITHUB_URL} target="_blank" rel="noreferrer">
            {t("about.githubLink")}
          </a>
          <a href={ISSUES_URL} target="_blank" rel="noreferrer">
            {t("about.issuesLink")}
          </a>
        </div>
        <button type="button" className="btn modal-close" onClick={onClose}>
          {t("common.close")}
        </button>
      </div>
    </div>
  );
}
