import type { SecretStore } from "@webglobe/core";

/**
 * Desktop adapter: stores secrets in the OS keychain (macOS Keychain, Windows
 * Credential Manager, Linux Secret Service) via the `secret_*` Tauri commands
 * in src-tauri/src/commands/secrets.rs, which wrap the Rust `keyring` crate.
 */
export class TauriKeyringSecretStore implements SecretStore {
  async get(id: string): Promise<string | undefined> {
    const { invoke } = await import("@tauri-apps/api/core");
    const value = await invoke<string | null>("secret_get", { id });
    return value ?? undefined;
  }

  async set(id: string, value: string): Promise<void> {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("secret_set", { id, value });
  }

  async remove(id: string): Promise<void> {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("secret_remove", { id });
  }
}
