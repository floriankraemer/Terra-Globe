import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { DrawingToolbar } from "./DrawingToolbar.js";

describe("DrawingToolbar", () => {
  it("renders Marker button unpressed and Geometry dropdown unset when idle", () => {
    render(
      <DrawingToolbar
        mode="idle"
        onSelectTool={vi.fn()}
        onFinishPolygon={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: "Marker" })).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByRole("combobox", { name: "Geometry" })).toHaveValue("");
    expect(screen.queryByRole("button", { name: "Cancel" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Finish" })).not.toBeInTheDocument();
  });

  it("calls onSelectTool when the Marker button is clicked", async () => {
    const onSelectTool = vi.fn();
    const user = userEvent.setup();
    render(
      <DrawingToolbar
        mode="idle"
        onSelectTool={onSelectTool}
        onFinishPolygon={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Marker" }));

    expect(onSelectTool).toHaveBeenCalledWith("point");
  });

  it("calls onSelectTool when a shape is chosen in the Geometry dropdown", async () => {
    const onSelectTool = vi.fn();
    const user = userEvent.setup();
    render(
      <DrawingToolbar
        mode="idle"
        onSelectTool={onSelectTool}
        onFinishPolygon={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    await user.selectOptions(screen.getByRole("combobox", { name: "Geometry" }), "circle");

    expect(onSelectTool).toHaveBeenCalledWith("circle");
  });

  it("reflects the active geometry tool as the dropdown's value and shows Cancel", () => {
    render(
      <DrawingToolbar
        mode="rectangle"
        onSelectTool={vi.fn()}
        onFinishPolygon={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    expect(screen.getByRole("combobox", { name: "Geometry" })).toHaveValue("rectangle");
    expect(screen.getByRole("button", { name: "Cancel" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Finish" })).not.toBeInTheDocument();
  });

  it("shows Finish only while drawing a polygon, and calls onFinishPolygon", async () => {
    const onFinishPolygon = vi.fn();
    const user = userEvent.setup();
    render(
      <DrawingToolbar
        mode="polygon"
        onSelectTool={vi.fn()}
        onFinishPolygon={onFinishPolygon}
        onCancel={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Finish" }));

    expect(onFinishPolygon).toHaveBeenCalled();
  });

  it("calls onCancel when Cancel is clicked", async () => {
    const onCancel = vi.fn();
    const user = userEvent.setup();
    render(
      <DrawingToolbar
        mode="point"
        onSelectTool={vi.fn()}
        onFinishPolygon={vi.fn()}
        onCancel={onCancel}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(onCancel).toHaveBeenCalled();
  });
});
