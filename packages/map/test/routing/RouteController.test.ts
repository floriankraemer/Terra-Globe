import { describe, expect, it } from "vitest";
import { RouteController } from "../../src/routing/RouteController.js";

const A = { lon: 0, lat: 0 };
const B = { lon: 1, lat: 0 };
const C = { lon: 2, lat: 0 };

describe("RouteController", () => {
  it("starts with no waypoints", () => {
    expect(new RouteController().getWaypoints()).toEqual([]);
  });

  it("accumulates waypoints in the order added", () => {
    const controller = new RouteController();
    controller.addWaypoint(A);
    controller.addWaypoint(B);
    controller.addWaypoint(C);
    expect(controller.getWaypoints()).toEqual([A, B, C]);
  });

  it("removes a waypoint by index", () => {
    const controller = new RouteController();
    controller.addWaypoint(A);
    controller.addWaypoint(B);
    controller.addWaypoint(C);
    controller.removeWaypoint(1);
    expect(controller.getWaypoints()).toEqual([A, C]);
  });

  it("reorders waypoints via moveWaypoint", () => {
    const controller = new RouteController();
    controller.addWaypoint(A);
    controller.addWaypoint(B);
    controller.addWaypoint(C);
    controller.moveWaypoint(0, 2);
    expect(controller.getWaypoints()).toEqual([B, C, A]);
  });

  it("ignores out-of-range moveWaypoint calls", () => {
    const controller = new RouteController();
    controller.addWaypoint(A);
    controller.addWaypoint(B);
    controller.moveWaypoint(0, 5);
    expect(controller.getWaypoints()).toEqual([A, B]);
  });

  it("clears all waypoints", () => {
    const controller = new RouteController();
    controller.addWaypoint(A);
    controller.clear();
    expect(controller.getWaypoints()).toEqual([]);
  });
});
