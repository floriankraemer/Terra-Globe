import { useState } from "react";
import type { CoordinateFormat, ProviderConfig, SecretStore, UnitSystem } from "@terra-globe/core";
import { ProvidersTab } from "./ProvidersTab.js";

export interface SettingsModalProps {
  unitSystem: UnitSystem;
  coordinateFormat: CoordinateFormat;
  onChangeUnitSystem: (unitSystem: UnitSystem) => void;
  onChangeCoordinateFormat: (coordinateFormat: CoordinateFormat) => void;
  providers: ProviderConfig[];
  secretStore: SecretStore;
  onAddProvider: (input: Omit<ProviderConfig, "id" | "enabled">) => string;
  onSetProviderEnabled: (id: string, enabled: boolean) => void;
  onRemoveProvider: (id: string) => void;
  onClose: () => void;
}

type SettingsSection = "units" | "providers";

const SECTIONS: { id: SettingsSection; label: string }[] = [
  { id: "units", label: "Units" },
  { id: "providers", label: "Providers" },
];

export function SettingsModal({
  unitSystem,
  coordinateFormat,
  onChangeUnitSystem,
  onChangeCoordinateFormat,
  providers,
  secretStore,
  onAddProvider,
  onSetProviderEnabled,
  onRemoveProvider,
  onClose,
}: SettingsModalProps) {
  const [activeSection, setActiveSection] = useState<SettingsSection>("units");

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="modal-panel settings-modal"
        role="dialog"
        aria-modal="true"
        aria-label="Settings"
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
              <div className="settings-modal-section-header">Units</div>
              <label className="placemark-editor-field">
                Unit System
                <select
                  value={unitSystem}
                  onChange={(e) => onChangeUnitSystem(e.target.value as UnitSystem)}
                >
                  <option value="metric">Metric</option>
                  <option value="imperial">Imperial</option>
                </select>
              </label>
              <label className="placemark-editor-field">
                Coordinate Format
                <select
                  value={coordinateFormat}
                  onChange={(e) => onChangeCoordinateFormat(e.target.value as CoordinateFormat)}
                >
                  <option value="decimal">Decimal degrees</option>
                  <option value="dms">Degrees, minutes, seconds</option>
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
          Close
        </button>
      </div>
    </div>
  );
}
