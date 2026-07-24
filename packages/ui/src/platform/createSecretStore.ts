import type { SecretStore } from "@webglobe/core";
import { isTauri } from "./isTauri.js";
import { LocalStorageSecretStore } from "./LocalStorageSecretStore.js";
import { TauriKeyringSecretStore } from "./TauriKeyringSecretStore.js";

/**
 * Composition root: the only place that picks the secret storage backend.
 * Everything downstream depends solely on the SecretStore port.
 */
export function createSecretStore(): SecretStore {
  return isTauri() ? new TauriKeyringSecretStore() : new LocalStorageSecretStore();
}
