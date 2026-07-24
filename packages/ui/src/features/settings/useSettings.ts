import { useCallback, useState } from "react";
import type { CoordinateFormat, ProviderConfig, SecretStore, UnitSystem } from "@webglobe/core";

export interface Settings {
  unitSystem: UnitSystem;
  coordinateFormat: CoordinateFormat;
  providers: ProviderConfig[];
}

export interface UseSettingsResult extends Settings {
  setUnitSystem: (unitSystem: UnitSystem) => void;
  setCoordinateFormat: (coordinateFormat: CoordinateFormat) => void;
  addProvider: (input: Omit<ProviderConfig, "id" | "enabled">) => string;
  setProviderEnabled: (id: string, enabled: boolean) => void;
  removeProvider: (id: string) => void;
}

const STORAGE_KEY = "webglobe:settings";

const DEFAULT_SETTINGS: Settings = {
  unitSystem: "metric",
  coordinateFormat: "decimal",
  providers: [],
};

function isUnitSystem(value: unknown): value is UnitSystem {
  return value === "metric" || value === "imperial";
}

function isCoordinateFormat(value: unknown): value is CoordinateFormat {
  return value === "decimal" || value === "dms";
}

function isProviderConfig(value: unknown): value is ProviderConfig {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Partial<ProviderConfig>;
  if (
    typeof candidate.id !== "string" ||
    typeof candidate.name !== "string" ||
    typeof candidate.enabled !== "boolean"
  ) {
    return false;
  }
  if (candidate.kind === "tile") return typeof candidate.preset === "string";
  if (candidate.kind === "geocoding") return typeof candidate.preset === "string";
  return false;
}

function readStoredSettings(): Settings {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT_SETTINGS;
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) return DEFAULT_SETTINGS;
    const candidate = parsed as Partial<Settings>;
    return {
      unitSystem: isUnitSystem(candidate.unitSystem)
        ? candidate.unitSystem
        : DEFAULT_SETTINGS.unitSystem,
      coordinateFormat: isCoordinateFormat(candidate.coordinateFormat)
        ? candidate.coordinateFormat
        : DEFAULT_SETTINGS.coordinateFormat,
      providers: Array.isArray(candidate.providers)
        ? candidate.providers.filter(isProviderConfig)
        : DEFAULT_SETTINGS.providers,
    };
  } catch {
    return DEFAULT_SETTINGS;
  }
}

function persist(settings: Settings): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // localStorage unavailable (e.g. private browsing) - settings just won't persist.
  }
}

export function useSettings(secretStore: SecretStore): UseSettingsResult {
  const [settings, setSettings] = useState<Settings>(readStoredSettings);

  const setUnitSystem = useCallback((unitSystem: UnitSystem) => {
    setSettings((prev) => {
      const next = { ...prev, unitSystem };
      persist(next);
      return next;
    });
  }, []);

  const setCoordinateFormat = useCallback((coordinateFormat: CoordinateFormat) => {
    setSettings((prev) => {
      const next = { ...prev, coordinateFormat };
      persist(next);
      return next;
    });
  }, []);

  const addProvider = useCallback((input: Omit<ProviderConfig, "id" | "enabled">) => {
    const id = crypto.randomUUID();
    setSettings((prev) => {
      const config = { ...input, id, enabled: false } as ProviderConfig;
      const next = { ...prev, providers: [...prev.providers, config] };
      persist(next);
      return next;
    });
    return id;
  }, []);

  const setProviderEnabled = useCallback((id: string, enabled: boolean) => {
    setSettings((prev) => {
      const target = prev.providers.find((p) => p.id === id);
      const providers = prev.providers.map((p) => {
        if (p.id === id) return { ...p, enabled };
        if (enabled && target?.kind === "geocoding" && p.kind === "geocoding") {
          return { ...p, enabled: false };
        }
        return p;
      });
      const next = { ...prev, providers };
      persist(next);
      return next;
    });
  }, []);

  const removeProvider = useCallback(
    (id: string) => {
      setSettings((prev) => {
        const next = { ...prev, providers: prev.providers.filter((p) => p.id !== id) };
        persist(next);
        return next;
      });
      void secretStore.remove(id);
    },
    [secretStore],
  );

  return {
    ...settings,
    setUnitSystem,
    setCoordinateFormat,
    addProvider,
    setProviderEnabled,
    removeProvider,
  };
}
