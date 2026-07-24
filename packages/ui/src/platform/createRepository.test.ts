import { afterEach, describe, expect, it, vi } from "vitest";

describe("createRepository", () => {
  afterEach(() => {
    delete window.__TAURI_INTERNALS__;
    vi.resetModules();
  });

  it("returns an IndexedDbPlacesRepository when not running inside Tauri", async () => {
    const { createRepository } = await import("./createRepository.js");
    const { IndexedDbPlacesRepository } = await import("@terra-globe/storage-indexeddb");

    const repo = await createRepository();

    expect(repo).toBeInstanceOf(IndexedDbPlacesRepository);
  });
});
