import { expect, type Page } from "@playwright/test";

/**
 * The canvas mounts before the IndexedDB-backed places repository finishes
 * loading; clicks/creates before that point silently no-op (see
 * useLibrary.ts). Wait for the app to flag itself ready before interacting.
 */
export async function waitForAppReady(page: Page): Promise<void> {
  await expect(page.locator("[data-app-ready='true']")).toBeVisible({ timeout: 15_000 });
}
