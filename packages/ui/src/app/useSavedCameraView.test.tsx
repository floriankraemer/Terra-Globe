import { render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type * as Cesium from "cesium";
import { parseCameraView, useSavedCameraView } from "./useSavedCameraView.js";

const STORAGE_KEY = "terra-globe:cameraView";

const SAMPLE_VIEW = {
  longitude: 13.4,
  latitude: 52.5,
  height: 1000,
  heading: 0.1,
  pitch: -0.2,
  roll: 0,
};

function createFakeViewer(): {
  viewer: Cesium.Viewer;
  setView: ReturnType<typeof vi.fn>;
  triggerMoveEnd: () => void;
} {
  let moveEndListener: (() => void) | undefined;
  const setView = vi.fn();
  const viewer = {
    camera: {
      positionCartographic: { longitude: 0.1, latitude: 0.2, height: 500 },
      heading: 0.3,
      pitch: -0.4,
      roll: 0.05,
      setView,
      moveEnd: {
        addEventListener: (fn: () => void) => {
          moveEndListener = fn;
        },
        removeEventListener: () => {
          moveEndListener = undefined;
        },
      },
    },
  } as unknown as Cesium.Viewer;
  return {
    viewer,
    setView,
    triggerMoveEnd: () => moveEndListener?.(),
  };
}

function TestHarness({ viewer }: { viewer: Cesium.Viewer | null }) {
  useSavedCameraView(viewer, STORAGE_KEY);
  return null;
}

describe("parseCameraView", () => {
  it("returns null when nothing is stored", () => {
    expect(parseCameraView(null)).toBeNull();
  });

  it("returns null for malformed JSON", () => {
    expect(parseCameraView("not json")).toBeNull();
  });

  it("returns null when a required field is missing", () => {
    expect(parseCameraView(JSON.stringify({ longitude: 1, latitude: 2 }))).toBeNull();
  });

  it("returns null when a field is non-numeric", () => {
    expect(parseCameraView(JSON.stringify({ ...SAMPLE_VIEW, height: "high" }))).toBeNull();
  });

  it("parses a valid stored view", () => {
    expect(parseCameraView(JSON.stringify(SAMPLE_VIEW))).toEqual(SAMPLE_VIEW);
  });
});

describe("useSavedCameraView", () => {
  afterEach(() => {
    window.localStorage.clear();
    vi.restoreAllMocks();
  });

  it("does nothing while the viewer is not yet ready", () => {
    render(<TestHarness viewer={null} />);
    expect(window.localStorage.getItem(STORAGE_KEY)).toBeNull();
  });

  it("restores a previously saved view once the viewer becomes available", () => {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(SAMPLE_VIEW));
    const { viewer, setView } = createFakeViewer();
    render(<TestHarness viewer={viewer} />);
    expect(setView).toHaveBeenCalledWith({
      destination: expect.anything(),
      orientation: {
        heading: SAMPLE_VIEW.heading,
        pitch: SAMPLE_VIEW.pitch,
        roll: SAMPLE_VIEW.roll,
      },
    });
  });

  it("does not call setView when nothing was saved", () => {
    const { viewer, setView } = createFakeViewer();
    render(<TestHarness viewer={viewer} />);
    expect(setView).not.toHaveBeenCalled();
  });

  it("saves the current camera pose to localStorage when the camera comes to rest", () => {
    const { viewer, triggerMoveEnd } = createFakeViewer();
    render(<TestHarness viewer={viewer} />);
    triggerMoveEnd();

    const saved = parseCameraView(window.localStorage.getItem(STORAGE_KEY));
    expect(saved).toEqual({
      longitude: expect.closeTo(0.1 * (180 / Math.PI), 5),
      latitude: expect.closeTo(0.2 * (180 / Math.PI), 5),
      height: 500,
      heading: 0.3,
      pitch: -0.4,
      roll: 0.05,
    });
  });

  it("stops saving after the viewer is torn down", () => {
    const { viewer, triggerMoveEnd } = createFakeViewer();
    const { unmount } = render(<TestHarness viewer={viewer} />);
    unmount();
    triggerMoveEnd();
    expect(window.localStorage.getItem(STORAGE_KEY)).toBeNull();
  });
});
