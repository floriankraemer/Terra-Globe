/**
 * Secret storage port (dependency-inversion boundary). Holds API keys addressed by
 * provider config id. Never persisted alongside plain settings data.
 */
export interface SecretStore {
  get(id: string): Promise<string | undefined>;
  set(id: string, value: string): Promise<void>;
  remove(id: string): Promise<void>;
}
