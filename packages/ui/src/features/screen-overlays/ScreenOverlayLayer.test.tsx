import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { ScreenOverlay } from "@terra-globe/core";
import { ScreenOverlayLayer } from "./ScreenOverlayLayer.js";

function overlay(overrides: Partial<ScreenOverlay> = {}): ScreenOverlay {
  return {
    id: "so1",
    folderId: null,
    name: "Legend",
    imageUrl: "https://example.com/legend.png",
    overlayXY: { x: 0, y: 1, xUnits: "fraction", yUnits: "fraction" },
    screenXY: { x: 0, y: 1, xUnits: "fraction", yUnits: "fraction" },
    visibility: true,
    order: 0,
    createdAt: "now",
    updatedAt: "now",
    ...overrides,
  };
}

describe("ScreenOverlayLayer", () => {
  it("renders an image per visible overlay", () => {
    render(<ScreenOverlayLayer overlays={[overlay()]} />);

    expect(screen.getByRole("img", { name: "Legend" })).toHaveAttribute(
      "src",
      "https://example.com/legend.png",
    );
  });

  it("skips overlays with visibility false", () => {
    render(<ScreenOverlayLayer overlays={[overlay({ visibility: false })]} />);

    expect(screen.queryByRole("img")).not.toBeInTheDocument();
  });
});
