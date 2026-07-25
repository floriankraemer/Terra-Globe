import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  GEOCODING_PRESETS,
  ROUTING_PRESETS,
  TILE_PRESETS,
  testGeocodingProviderConfig,
  testRoutingProviderConfig,
  testTileProviderConfig,
  type GeocodingPresetId,
  type ProviderConfig,
  type RoutingPresetId,
  type SecretStore,
  type TilePresetId,
} from "@terra-globe/core";
import { isTauri } from "../../platform/isTauri.js";

export interface ProvidersTabProps {
  providers: ProviderConfig[];
  secretStore: SecretStore;
  onAdd: (input: Omit<ProviderConfig, "id" | "enabled">) => string;
  onSetEnabled: (id: string, enabled: boolean) => void;
  onRemove: (id: string) => void;
}

type TestStatus = "idle" | "testing" | "success" | "failure";
type Kind = ProviderConfig["kind"];

const TILE_PRESET_OPTIONS = Object.values(TILE_PRESETS);
const GEOCODING_PRESET_OPTIONS = Object.values(GEOCODING_PRESETS);
const ROUTING_PRESET_OPTIONS = Object.values(ROUTING_PRESETS);

function presetOptionsFor(kind: Kind) {
  if (kind === "tile") return TILE_PRESET_OPTIONS;
  if (kind === "geocoding") return GEOCODING_PRESET_OPTIONS;
  return ROUTING_PRESET_OPTIONS;
}

export function ProvidersTab({
  providers,
  secretStore,
  onAdd,
  onSetEnabled,
  onRemove,
}: ProvidersTabProps) {
  const { t } = useTranslation();
  const KIND_LABELS: Record<Kind, string> = {
    tile: t("providers.kindTile"),
    geocoding: t("providers.kindGeocoding"),
    routing: t("providers.kindRouting"),
  };
  const STATUS_LABELS: Record<TestStatus, string> = {
    idle: t("providers.statusIdle"),
    testing: t("providers.statusTesting"),
    success: t("providers.statusSuccess"),
    failure: t("providers.statusFailure"),
  };
  const [kind, setKind] = useState<Kind>("tile");
  const [presetId, setPresetId] = useState<string>(TILE_PRESET_OPTIONS[0]!.id);
  const [name, setName] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [testState, setTestState] = useState<
    Record<string, { status: TestStatus; error?: string }>
  >({});

  const presetOptions = presetOptionsFor(kind);

  function handleKindChange(nextKind: Kind) {
    setKind(nextKind);
    setPresetId(presetOptionsFor(nextKind)[0]!.id);
  }

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    const preset = presetOptions.find((p) => p.id === presetId);
    if (!preset) return;
    const trimmedName = name.trim() || preset.name;
    const id =
      kind === "tile"
        ? onAdd({ kind: "tile", preset: presetId as TilePresetId, name: trimmedName })
        : kind === "geocoding"
          ? onAdd({ kind: "geocoding", preset: presetId as GeocodingPresetId, name: trimmedName })
          : onAdd({ kind: "routing", preset: presetId as RoutingPresetId, name: trimmedName });
    if (apiKey.trim().length > 0) {
      void secretStore.set(id, apiKey.trim());
    }
    setName("");
    setApiKey("");
  }

  async function handleTest(config: ProviderConfig) {
    setTestState((s) => ({ ...s, [config.id]: { status: "testing" } }));
    const key = (await secretStore.get(config.id)) ?? "";
    const result =
      config.kind === "tile"
        ? await testTileProviderConfig(config, key)
        : config.kind === "geocoding"
          ? await testGeocodingProviderConfig(config, key)
          : await testRoutingProviderConfig(config, key);
    setTestState((s) => ({
      ...s,
      [config.id]: { status: result.ok ? "success" : "failure", error: result.error },
    }));
    if (result.ok) onSetEnabled(config.id, true);
  }

  function handleRemove(id: string) {
    onRemove(id);
    setTestState((s) => {
      const rest = { ...s };
      delete rest[id];
      return rest;
    });
  }

  return (
    <>
      <div className="settings-modal-section-header">{t("providers.header")}</div>
      {!isTauri() && (
        <div className="provider-secret-warning">{t("providers.desktopWarning")}</div>
      )}

      <ul className="providers-list">
        {providers.map((config) => {
          const state = testState[config.id] ?? { status: "idle" as TestStatus };
          return (
            <li key={config.id} className="provider-item">
              <span className="provider-item-name">{config.name}</span>
              <span className="provider-item-kind">{KIND_LABELS[config.kind]}</span>
              <span className={`provider-status-badge ${state.status}`} title={state.error}>
                {STATUS_LABELS[state.status]}
              </span>
              <button type="button" className="btn" onClick={() => void handleTest(config)}>
                {t("providers.test")}
              </button>
              <button type="button" className="btn" onClick={() => handleRemove(config.id)}>
                {t("providers.remove")}
              </button>
            </li>
          );
        })}
        {providers.length === 0 && (
          <li className="provider-item-empty">{t("providers.empty")}</li>
        )}
      </ul>

      <form className="provider-add-form" onSubmit={handleSubmit}>
        <label className="placemark-editor-field">
          {t("providers.type")}
          <select value={kind} onChange={(e) => handleKindChange(e.target.value as Kind)}>
            <option value="tile">{t("providers.tileKind")}</option>
            <option value="geocoding">{t("providers.geocodingKind")}</option>
            <option value="routing">{t("providers.routingKind")}</option>
          </select>
        </label>
        <label className="placemark-editor-field">
          {t("providers.provider")}
          <select value={presetId} onChange={(e) => setPresetId(e.target.value)}>
            {presetOptions.map((preset) => (
              <option key={preset.id} value={preset.id}>
                {preset.name}
              </option>
            ))}
          </select>
        </label>
        <label className="placemark-editor-field">
          {t("providers.name")}
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={presetOptions.find((p) => p.id === presetId)?.name}
          />
        </label>
        <label className="placemark-editor-field">
          {t("providers.apiKey")}
          <input type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} />
        </label>
        <button type="submit" className="btn">
          {t("providers.addProvider")}
        </button>
      </form>
    </>
  );
}
