import { beforeEach, describe, expect, it } from "vitest";
import { LocalStorageSecretStore } from "./LocalStorageSecretStore.js";

describe("LocalStorageSecretStore", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("returns undefined for an unknown id", async () => {
    const store = new LocalStorageSecretStore();
    expect(await store.get("missing")).toBeUndefined();
  });

  it("round-trips a secret through set/get", async () => {
    const store = new LocalStorageSecretStore();
    await store.set("p1", "sk-123");
    expect(await store.get("p1")).toBe("sk-123");
  });

  it("removes a secret", async () => {
    const store = new LocalStorageSecretStore();
    await store.set("p1", "sk-123");
    await store.remove("p1");
    expect(await store.get("p1")).toBeUndefined();
  });

  it("persists across separate instances (backed by localStorage)", async () => {
    await new LocalStorageSecretStore().set("p1", "sk-123");
    expect(await new LocalStorageSecretStore().get("p1")).toBe("sk-123");
  });

  it("falls back to empty when stored JSON is corrupt", async () => {
    localStorage.setItem("terra-globe:secrets", "not json");
    const store = new LocalStorageSecretStore();
    expect(await store.get("p1")).toBeUndefined();
  });
});
