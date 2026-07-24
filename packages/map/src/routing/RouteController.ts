import type { GeoPoint } from "@terra-globe/core";

/**
 * Ordered waypoint list for the route planner - similar to RulerController,
 * has no entity/commit step of its own. Route calculation is async
 * (network-backed routing provider or a local estimate), so it belongs in
 * the UI hook, not here.
 */
export class RouteController {
  private waypoints: GeoPoint[] = [];

  getWaypoints(): GeoPoint[] {
    return [...this.waypoints];
  }

  addWaypoint(point: GeoPoint): void {
    this.waypoints.push(point);
  }

  removeWaypoint(index: number): void {
    this.waypoints.splice(index, 1);
  }

  moveWaypoint(from: number, to: number): void {
    if (from < 0 || from >= this.waypoints.length || to < 0 || to >= this.waypoints.length) return;
    const [moved] = this.waypoints.splice(from, 1);
    this.waypoints.splice(to, 0, moved!);
  }

  clear(): void {
    this.waypoints = [];
  }
}
