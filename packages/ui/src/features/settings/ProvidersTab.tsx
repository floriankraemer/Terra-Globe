import { useState } from "react";
import {
  GEOCODING_PRESETS,
  TILE_PRESETS,
  testGeocodingProviderConfig,
  testTileProviderConfig,
  type GeocodingPresetId,
  type ProviderConfig,
  type SecretStore,
  type TilePresetId,
} from "@webglobe/core";
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

export function ProvidersTab({
  providers,
  secretStore,
  onAdd,
  onSetEnabled,
  onRemove,
}: ProvidersTabProps) {
  const [kind, setKind] = useState<Kind>("tile");
  const [presetId, setPresetId] = useState<string>(TILE_PRESET_OPTIONS[0]!.id);
  const [name, setName] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [testState, setTestState] = useState<
    Record<string, { status: TestStatus; error?: string }>
  >({});

  const presetOptions = kind === "tile" ? TILE_PRESET_OPTIONS : GEOCODING_PRESET_OPTIONS;

  function handleKindChange(nextKind: Kind) {
    setKind(nextKind);
    const firstOption =
      nextKind === "tile" ? TILE_PRESET_OPTIONS[0]! : GEOCODING_PRESET_OPTIONS[0]!;
    setPresetId(firstOption.id);
  }

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    const preset = presetOptions.find((p) => p.id === presetId);
    if (!preset) return;
    const trimmedName = name.trim() || preset.name;
    const id =
      kind === "tile"
        ? onAdd({ kind: "tile", preset: presetId as TilePresetId, name: trimmedName })
        : onAdd({ kind: "geocoding", preset: presetId as GeocodingPresetId, name: trimmedName });
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
        : await testGeocodingProviderConfig(config, key);
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
      <div className="settings-modal-section-header">Providers</div>
      {!isTauri() && (
        <div className="provider-secret-warning">
          Not running as desktop app — API keys are stored in browser local storage, which is less
          secure than the OS keychain.
        </div>
      )}

      <ul className="providers-list">
        {providers.map((config) => {
          const state = testState[config.id] ?? { status: "idle" as TestStatus };
          return (
            <li key={config.id} className="provider-item">
              <span className="provider-item-name">{config.name}</span>
              <span className="provider-item-kind">
                {config.kind === "tile" ? "Tile" : "Geocoding"}
              </span>
              <span className={`provider-status-badge ${state.status}`} title={state.error}>
                {state.status}
              </span>
              <button type="button" className="btn" onClick={() => void handleTest(config)}>
                Test
              </button>
              <button type="button" className="btn" onClick={() => handleRemove(config.id)}>
                Remove
              </button>
            </li>
          );
        })}
        {providers.length === 0 && (
          <li className="provider-item-empty">No providers configured yet.</li>
        )}
      </ul>

      <form className="provider-add-form" onSubmit={handleSubmit}>
        <label className="placemark-editor-field">
          Type
          <select value={kind} onChange={(e) => handleKindChange(e.target.value as Kind)}>
            <option value="tile">Tile (basemap)</option>
            <option value="geocoding">Geocoding (address search)</option>
          </select>
        </label>
        <label className="placemark-editor-field">
          Provider
          <select value={presetId} onChange={(e) => setPresetId(e.target.value)}>
            {presetOptions.map((preset) => (
              <option key={preset.id} value={preset.id}>
                {preset.name}
              </option>
            ))}
          </select>
        </label>
        <label className="placemark-editor-field">
          Name
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={presetOptions.find((p) => p.id === presetId)?.name}
          />
        </label>
        <label className="placemark-editor-field">
          API Key
          <input type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} />
        </label>
        <button type="submit" className="btn">
          Add Provider
        </button>
      </form>
    </>
  );
}
