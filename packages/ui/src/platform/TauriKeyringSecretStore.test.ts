import { describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { TauriKeyringSecretStore } from "./TauriKeyringSecretStore.js";

describe("TauriKeyringSecretStore", () => {
  it("get() invokes secret_get and maps null to undefined", async () => {
    invoke.mockResolvedValueOnce(null);
    const store = new TauriKeyringSecretStore();
    expect(await store.get("p1")).toBeUndefined();
    expect(invoke).toHaveBeenCalledWith("secret_get", { id: "p1" });
  });

  it("get() returns the resolved value", async () => {
    invoke.mockResolvedValueOnce("sk-123");
    const store = new TauriKeyringSecretStore();
    expect(await store.get("p1")).toBe("sk-123");
  });

  it("set() invokes secret_set with id and value", async () => {
    invoke.mockResolvedValueOnce(undefined);
    const store = new TauriKeyringSecretStore();
    await store.set("p1", "sk-123");
    expect(invoke).toHaveBeenCalledWith("secret_set", { id: "p1", value: "sk-123" });
  });

  it("remove() invokes secret_remove", async () => {
    invoke.mockResolvedValueOnce(undefined);
    const store = new TauriKeyringSecretStore();
    await store.remove("p1");
    expect(invoke).toHaveBeenCalledWith("secret_remove", { id: "p1" });
  });
});
