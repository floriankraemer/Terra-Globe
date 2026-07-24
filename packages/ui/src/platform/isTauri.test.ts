import { afterEach, describe, expect, it } from "vitest";
import { isTauri } from "./isTauri.js";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

describe("isTauri", () => {
  afterEach(() => {
    delete window.__TAURI_INTERNALS__;
  });

  it("returns false when not running inside Tauri", () => {
    expect(isTauri()).toBe(false);
  });

  it("returns true when the Tauri internals global is present", () => {
    window.__TAURI_INTERNALS__ = {};
    expect(isTauri()).toBe(true);
  });
});
