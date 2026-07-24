import type { SecretStore } from "@webglobe/core";

const STORAGE_KEY = "webglobe:secrets";

function readAll(): Record<string, string> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return {};
    const parsed: unknown = JSON.parse(raw);
    return typeof parsed === "object" && parsed !== null ? (parsed as Record<string, string>) : {};
  } catch {
    return {};
  }
}

function writeAll(secrets: Record<string, string>): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(secrets));
  } catch {
    // Storage unavailable (e.g. private browsing quota) — secret silently not persisted.
  }
}

/**
 * Fallback adapter for plain-browser (non-Tauri) sessions. Stores secrets in
 * plaintext localStorage — less secure than the OS keychain; callers should
 * surface a warning to the user when this adapter is active.
 */
export class LocalStorageSecretStore implements SecretStore {
  async get(id: string): Promise<string | undefined> {
    return readAll()[id];
  }

  async set(id: string, value: string): Promise<void> {
    const secrets = readAll();
    secrets[id] = value;
    writeAll(secrets);
  }

  async remove(id: string): Promise<void> {
    const secrets = readAll();
    delete secrets[id];
    writeAll(secrets);
  }
}
