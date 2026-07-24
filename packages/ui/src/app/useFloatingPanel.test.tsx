import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { useFloatingPanel } from "./useFloatingPanel.js";

function TestHarness({ storageKey }: { storageKey?: string }) {
  const { geometry, isDragging, isResizing, startDrag, startResize } = useFloatingPanel({
    initial: { x: 100, y: 80, width: 260, height: 200 },
    minWidth: 200,
    minHeight: 150,
    maxWidth: 600,
    maxHeight: 600,
    storageKey,
  });
  return (
    <div>
      <div data-testid="geometry">{JSON.stringify(geometry)}</div>
      <div data-testid="dragging">{String(isDragging)}</div>
      <div data-testid="resizing">{String(isResizing)}</div>
      <div data-testid="drag-handle" onMouseDown={startDrag} />
      <div data-testid="resize-handle" onMouseDown={startResize} />
    </div>
  );
}

function geometryOf(): { x: number; y: number; width: number; height: number } {
  return JSON.parse(screen.getByTestId("geometry").textContent ?? "{}");
}

describe("useFloatingPanel - drag", () => {
  it("starts at the initial geometry", () => {
    render(<TestHarness />);
    expect(geometryOf()).toEqual({ x: 100, y: 80, width: 260, height: 200 });
    expect(screen.getByTestId("dragging")).toHaveTextContent("false");
  });

  it("moves x/y with the mouse after a mousedown on the drag handle", () => {
    render(<TestHarness />);
    fireEvent.mouseDown(screen.getByTestId("drag-handle"), { clientX: 100, clientY: 100 });
    expect(screen.getByTestId("dragging")).toHaveTextContent("true");

    fireEvent.mouseMove(window, { clientX: 150, clientY: 120 });
    expect(geometryOf()).toMatchObject({ x: 150, y: 100 });

    fireEvent.mouseUp(window);
    expect(screen.getByTestId("dragging")).toHaveTextContent("false");
  });

  it("clamps position to stay within the viewport", () => {
    render(<TestHarness />);
    fireEvent.mouseDown(screen.getByTestId("drag-handle"), { clientX: 0, clientY: 0 });
    fireEvent.mouseMove(window, { clientX: -5000, clientY: -5000 });
    expect(geometryOf()).toMatchObject({ x: 0, y: 0 });
  });

  it("stops updating position after mouseup", () => {
    render(<TestHarness />);
    fireEvent.mouseDown(screen.getByTestId("drag-handle"), { clientX: 100, clientY: 100 });
    fireEvent.mouseMove(window, { clientX: 150, clientY: 100 });
    fireEvent.mouseUp(window);

    fireEvent.mouseMove(window, { clientX: 400, clientY: 100 });
    expect(geometryOf()).toMatchObject({ x: 150, y: 80 });
  });
});

describe("useFloatingPanel - resize", () => {
  it("grows width/height with the mouse after a mousedown on the resize handle", () => {
    render(<TestHarness />);
    fireEvent.mouseDown(screen.getByTestId("resize-handle"), { clientX: 100, clientY: 100 });
    expect(screen.getByTestId("resizing")).toHaveTextContent("true");

    fireEvent.mouseMove(window, { clientX: 150, clientY: 130 });
    expect(geometryOf()).toMatchObject({ width: 310, height: 230 });

    fireEvent.mouseUp(window);
    expect(screen.getByTestId("resizing")).toHaveTextContent("false");
  });

  it("clamps to the minimum size", () => {
    render(<TestHarness />);
    fireEvent.mouseDown(screen.getByTestId("resize-handle"), { clientX: 100, clientY: 100 });
    fireEvent.mouseMove(window, { clientX: -5000, clientY: -5000 });
    expect(geometryOf()).toMatchObject({ width: 200, height: 150 });
  });

  it("clamps to the maximum size", () => {
    render(<TestHarness />);
    fireEvent.mouseDown(screen.getByTestId("resize-handle"), { clientX: 100, clientY: 100 });
    fireEvent.mouseMove(window, { clientX: 5000, clientY: 5000 });
    expect(geometryOf()).toMatchObject({ width: 600, height: 600 });
  });
});

describe("useFloatingPanel - persistence", () => {
  afterEach(() => {
    window.localStorage.clear();
  });

  it("reads the initial geometry from localStorage when present", () => {
    window.localStorage.setItem(
      "panel-geometry",
      JSON.stringify({ x: 10, y: 20, width: 300, height: 250 }),
    );
    render(<TestHarness storageKey="panel-geometry" />);
    expect(geometryOf()).toEqual({ x: 10, y: 20, width: 300, height: 250 });
  });

  it("falls back to the initial geometry when localStorage has malformed JSON", () => {
    window.localStorage.setItem("panel-geometry", "not json");
    render(<TestHarness storageKey="panel-geometry" />);
    expect(geometryOf()).toEqual({ x: 100, y: 80, width: 260, height: 200 });
  });

  it("persists geometry to localStorage after a drag", () => {
    render(<TestHarness storageKey="panel-geometry" />);
    fireEvent.mouseDown(screen.getByTestId("drag-handle"), { clientX: 100, clientY: 100 });
    fireEvent.mouseMove(window, { clientX: 150, clientY: 120 });
    fireEvent.mouseUp(window);

    expect(JSON.parse(window.localStorage.getItem("panel-geometry") ?? "{}")).toMatchObject({
      x: 150,
      y: 100,
    });
  });
});
