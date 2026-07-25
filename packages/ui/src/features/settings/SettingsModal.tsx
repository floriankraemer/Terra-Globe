import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { CoordinateFormat, ProviderConfig, SecretStore, UnitSystem } from "@terra-globe/core";
import { SUPPORTED_LOCALES, type SupportedLocale } from "../../i18n/types.js";
import { ProvidersTab } from "./ProvidersTab.js";

export interface SettingsModalProps {
  unitSystem: UnitSystem;
  coordinateFormat: CoordinateFormat;
  language: SupportedLocale;
  onChangeUnitSystem: (unitSystem: UnitSystem) => void;
  onChangeCoordinateFormat: (coordinateFormat: CoordinateFormat) => void;
  onChangeLanguage: (language: SupportedLocale) => void;
  providers: ProviderConfig[];
  secretStore: SecretStore;
  onAddProvider: (input: Omit<ProviderConfig, "id" | "enabled">) => string;
  onSetProviderEnabled: (id: string, enabled: boolean) => void;
  onRemoveProvider: (id: string) => void;
  onClose: () => void;
}

type SettingsSection = "units" | "language" | "providers";

export function SettingsModal({
  unitSystem,
  coordinateFormat,
  language,
  onChangeUnitSystem,
  onChangeCoordinateFormat,
  onChangeLanguage,
  providers,
  secretStore,
  onAddProvider,
  onSetProviderEnabled,
  onRemoveProvider,
  onClose,
}: SettingsModalProps) {
  const { t } = useTranslation();
  const [activeSection, setActiveSection] = useState<SettingsSection>("units");
  const SECTIONS: { id: SettingsSection; label: string }[] = [
    { id: "units", label: t("settings.sectionUnits") },
    { id: "language", label: t("settings.sectionLanguage") },
    { id: "providers", label: t("settings.sectionProviders") },
  ];

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="modal-panel settings-modal"
        role="dialog"
        aria-modal="true"
        aria-label={t("settings.title")}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="settings-modal-sidebar">
          {SECTIONS.map((section) => (
            <button
              key={section.id}
              type="button"
              className="settings-modal-nav-item"
              aria-current={activeSection === section.id}
              onClick={() => setActiveSection(section.id)}
            >
              {section.label}
            </button>
          ))}
        </div>
        <div className="settings-modal-content">
          {activeSection === "units" && (
            <>
              <div className="settings-modal-section-header">{t("settings.sectionUnits")}</div>
              <label className="placemark-editor-field">
                {t("settings.unitSystem")}
                <select
                  value={unitSystem}
                  onChange={(e) => onChangeUnitSystem(e.target.value as UnitSystem)}
                >
                  <option value="metric">{t("settings.metric")}</option>
                  <option value="imperial">{t("settings.imperial")}</option>
                </select>
              </label>
              <label className="placemark-editor-field">
                {t("settings.coordinateFormat")}
                <select
                  value={coordinateFormat}
                  onChange={(e) => onChangeCoordinateFormat(e.target.value as CoordinateFormat)}
                >
                  <option value="decimal">{t("settings.decimalDegrees")}</option>
                  <option value="dms">{t("settings.dms")}</option>
                </select>
              </label>
            </>
          )}
          {activeSection === "language" && (
            <>
              <div className="settings-modal-section-header">{t("settings.sectionLanguage")}</div>
              <label className="placemark-editor-field">
                {t("settings.language")}
                <select
                  value={language}
                  onChange={(e) => onChangeLanguage(e.target.value as SupportedLocale)}
                >
                  {SUPPORTED_LOCALES.map((locale) => (
                    <option key={locale.id} value={locale.id}>
                      {locale.label}
                    </option>
                  ))}
                </select>
              </label>
            </>
          )}
          {activeSection === "providers" && (
            <ProvidersTab
              providers={providers}
              secretStore={secretStore}
              onAdd={onAddProvider}
              onSetEnabled={onSetProviderEnabled}
              onRemove={onRemoveProvider}
            />
          )}
        </div>
        <button type="button" className="btn modal-close" onClick={onClose}>
          {t("settings.close")}
        </button>
      </div>
    </div>
  );
}
