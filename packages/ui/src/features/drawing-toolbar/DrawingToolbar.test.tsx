import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { DrawingToolbar } from "./DrawingToolbar.js";

describe("DrawingToolbar", () => {
  it("renders Marker button unpressed and Geometry dropdown unset when idle", () => {
    render(
      <DrawingToolbar mode="idle" onSelectTool={vi.fn()} onFinish={vi.fn()} onCancel={vi.fn()} />,
    );

    expect(screen.getByRole("button", { name: "Marker" })).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByRole("button", { name: "Geometry: Select shape" })).toBeInTheDocument();
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
        onFinish={vi.fn()}
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
        onFinish={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Geometry: Select shape" }));
    await user.click(screen.getByRole("menuitemradio", { name: "Circle" }));

    expect(onSelectTool).toHaveBeenCalledWith("circle");
  });

  it("reflects the active geometry tool as the dropdown's label and shows Cancel", () => {
    render(
      <DrawingToolbar
        mode="rectangle"
        onSelectTool={vi.fn()}
        onFinish={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: "Geometry: Rectangle" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Finish" })).not.toBeInTheDocument();
  });

  it("shows Finish only while drawing a polygon, and calls onFinish", async () => {
    const onFinish = vi.fn();
    const user = userEvent.setup();
    render(
      <DrawingToolbar
        mode="polygon"
        onSelectTool={vi.fn()}
        onFinish={onFinish}
        onCancel={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Finish" }));

    expect(onFinish).toHaveBeenCalled();
  });

  it("shows Finish while drawing a line, and calls onFinish", async () => {
    const onFinish = vi.fn();
    const user = userEvent.setup();
    render(
      <DrawingToolbar mode="line" onSelectTool={vi.fn()} onFinish={onFinish} onCancel={vi.fn()} />,
    );

    await user.click(screen.getByRole("button", { name: "Finish" }));

    expect(onFinish).toHaveBeenCalled();
  });

  it("calls onSelectTool with 'line' when Line is chosen in the Geometry dropdown", async () => {
    const onSelectTool = vi.fn();
    const user = userEvent.setup();
    render(
      <DrawingToolbar
        mode="idle"
        onSelectTool={onSelectTool}
        onFinish={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Geometry: Select shape" }));
    await user.click(screen.getByRole("menuitemradio", { name: "Line" }));

    expect(onSelectTool).toHaveBeenCalledWith("line");
  });

  it("calls onCancel when Cancel is clicked", async () => {
    const onCancel = vi.fn();
    const user = userEvent.setup();
    render(
      <DrawingToolbar mode="point" onSelectTool={vi.fn()} onFinish={vi.fn()} onCancel={onCancel} />,
    );

    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(onCancel).toHaveBeenCalled();
  });
});
