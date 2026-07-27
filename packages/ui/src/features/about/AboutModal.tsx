import { useTranslation } from "react-i18next";

const APP_NAME = "Terra Globe";
const AUTHOR = "Florian Krämer";
const AUTHOR_URL = "https://florian-kraemer.net/";
const LICENSE_NAME = "GNU General Public License v3.0";
const LICENSE_URL = "https://www.gnu.org/licenses/gpl-3.0.html";
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
        <p className="about-modal-line">
          {t("about.authorLabel")}{" "}
          <a href={AUTHOR_URL} target="_blank" rel="noopener noreferrer">
            {AUTHOR}
          </a>
        </p>
        <p className="about-modal-line">
          {t("about.licensePrefix")}{" "}
          <a href={LICENSE_URL} target="_blank" rel="noopener noreferrer">
            {LICENSE_NAME}
          </a>
        </p>
        <div className="about-modal-libraries">
          <div className="about-modal-section-header">{t("about.builtWith")}</div>
          <ul>
            {__APP_LIBRARIES__.map((library) => (
              <li key={library.name}>
                <a href={library.url} target="_blank" rel="noopener noreferrer">
                  {library.name}
                </a>{" "}
                {library.version}
              </li>
            ))}
          </ul>
        </div>
        <div className="about-modal-links">
          <a href={GITHUB_URL} target="_blank" rel="noopener noreferrer">
            {t("about.githubLink")}
          </a>
          <a href={ISSUES_URL} target="_blank" rel="noopener noreferrer">
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
