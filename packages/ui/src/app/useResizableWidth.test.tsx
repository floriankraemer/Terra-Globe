import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { useResizableWidth } from "./useResizableWidth.js";

function TestHarness({ storageKey }: { storageKey?: string }) {
  const { width, isResizing, startResize } = useResizableWidth(260, 200, 500, storageKey);
  return (
    <div>
      <div data-testid="width">{width}</div>
      <div data-testid="resizing">{String(isResizing)}</div>
      <div data-testid="handle" onMouseDown={startResize} />
    </div>
  );
}

describe("useResizableWidth", () => {
  it("starts at the initial width", () => {
    render(<TestHarness />);
    expect(screen.getByTestId("width")).toHaveTextContent("260");
    expect(screen.getByTestId("resizing")).toHaveTextContent("false");
  });

  it("grows the width as the mouse moves right after a mousedown on the handle", () => {
    render(<TestHarness />);
    fireEvent.mouseDown(screen.getByTestId("handle"), { clientX: 100 });
    expect(screen.getByTestId("resizing")).toHaveTextContent("true");

    fireEvent.mouseMove(window, { clientX: 150 });
    expect(screen.getByTestId("width")).toHaveTextContent("310");

    fireEvent.mouseUp(window);
    expect(screen.getByTestId("resizing")).toHaveTextContent("false");
  });

  it("clamps to the minimum width", () => {
    render(<TestHarness />);
    fireEvent.mouseDown(screen.getByTestId("handle"), { clientX: 100 });
    fireEvent.mouseMove(window, { clientX: -1000 });
    expect(screen.getByTestId("width")).toHaveTextContent("200");
  });

  it("clamps to the maximum width", () => {
    render(<TestHarness />);
    fireEvent.mouseDown(screen.getByTestId("handle"), { clientX: 100 });
    fireEvent.mouseMove(window, { clientX: 5000 });
    expect(screen.getByTestId("width")).toHaveTextContent("500");
  });

  it("stops updating width after mouseup", () => {
    render(<TestHarness />);
    fireEvent.mouseDown(screen.getByTestId("handle"), { clientX: 100 });
    fireEvent.mouseMove(window, { clientX: 150 });
    fireEvent.mouseUp(window);

    fireEvent.mouseMove(window, { clientX: 300 });
    expect(screen.getByTestId("width")).toHaveTextContent("310");
  });
});

describe("useResizableWidth - persistence", () => {
  afterEach(() => {
    window.localStorage.clear();
  });

  it("reads the initial width from localStorage when present", () => {
    window.localStorage.setItem("sidebar-width", "350");
    render(<TestHarness storageKey="sidebar-width" />);
    expect(screen.getByTestId("width")).toHaveTextContent("350");
  });

  it("clamps a stored width outside [min, max]", () => {
    window.localStorage.setItem("sidebar-width", "9999");
    render(<TestHarness storageKey="sidebar-width" />);
    expect(screen.getByTestId("width")).toHaveTextContent("500");
  });

  it("persists the width to localStorage after a resize", () => {
    render(<TestHarness storageKey="sidebar-width" />);
    fireEvent.mouseDown(screen.getByTestId("handle"), { clientX: 100 });
    fireEvent.mouseMove(window, { clientX: 150 });
    fireEvent.mouseUp(window);

    expect(window.localStorage.getItem("sidebar-width")).toBe("310");
  });
});
