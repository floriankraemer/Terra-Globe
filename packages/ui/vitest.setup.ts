import "@testing-library/jest-dom/vitest";
import "fake-indexeddb/auto";
import "./src/i18n/i18n.js";
import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";

afterEach(() => {
  cleanup();
});
