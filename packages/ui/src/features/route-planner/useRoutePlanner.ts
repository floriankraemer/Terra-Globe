import { useEffect, useRef, useState } from "react";
import * as Cesium from "cesium";
import {
  estimateStraightLineRoute,
  type GeocodingProvider,
  type GeoPoint,
  type RouteLeg,
  type RoutingProfile,
  type RoutingProvider,
  type Style,
} from "@webglobe/core";
import {
  CesiumEntityFactory,
  CesiumScreenPicker,
  RouteController,
  flyToGeometry,
  type EntityHandle,
  type IEntityFactory,
} from "@webglobe/map";

export type TravelMode = RoutingProfile | "train" | "plane";

export interface RouteStop {
  id: string;
  point: GeoPoint;
  label: string | null;
}

interface StopMeta {
  id: string;
  label: string | null;
}

export interface UseRoutePlannerResult {
  active: boolean;
  stops: RouteStop[];
  mode: TravelMode;
  alternatives: RouteLeg[];
  selectedIndex: number;
  totalDistanceMeters: number;
  totalDurationSeconds: number;
  loading: boolean;
  error: string | null;
  setMode: (mode: TravelMode) => void;
  selectAlternative: (index: number) => void;
  start: () => void;
  finish: () => void;
  cancel: () => void;
  addStop: (point: GeoPoint, label?: string) => void;
  removeStop: (index: number) => void;
  reorderStop: (from: number, to: number) => void;
}

const ROUTE_STYLE: Style = {
  id: "route-line",
  outlineEnabled: true,
  outlineColor: "#1e90ff",
  outlineWidth: 5,
  filled: false,
  fillColor: "#1e90ff",
  fillOpacity: 0,
};
const ROUTE_ALTERNATIVE_STYLE: Style = {
  ...ROUTE_STYLE,
  id: "route-alternative",
  outlineColor: "#9fb8d9",
  outlineWidth: 3,
};
const ROUTE_STOP_STYLE: Style = {
  ...ROUTE_STYLE,
  id: "route-stop",
  filled: true,
  fillOpacity: 1,
};

const PRIMARY_HANDLE: EntityHandle = { entityId: "__route_primary__" };
const stopHandle = (index: number): EntityHandle => ({ entityId: `__route_stop_${index}__` });
const alternativeHandle = (index: number): EntityHandle => ({
  entityId: `__route_alt_${index}__`,
});

/** Wires RouteController to a live Cesium viewer. Cesium glue - covered by E2E, not unit tests. */
export function useRoutePlanner(
  viewer: Cesium.Viewer | null,
  routingProvider: RoutingProvider | undefined,
  geocodingProvider: GeocodingProvider | undefined,
): UseRoutePlannerResult {
  const [active, setActive] = useState(false);
  const [waypoints, setWaypoints] = useState<GeoPoint[]>([]);
  const [stopMeta, setStopMeta] = useState<StopMeta[]>([]);
  const [mode, setMode] = useState<TravelMode>("car");
  const [alternatives, setAlternatives] = useState<RouteLeg[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const controllerRef = useRef(new RouteController());
  const activeRef = useRef(false);
  const geocodingProviderRef = useRef(geocodingProvider);
  geocodingProviderRef.current = geocodingProvider;
  const entityFactoryRef = useRef<IEntityFactory | null>(null);
  const pickerRef = useRef<CesiumScreenPicker | null>(null);
  const stopHandlesRef = useRef<EntityHandle[]>([]);
  const altHandlesRef = useRef<EntityHandle[]>([]);
  const primaryVisibleRef = useRef(false);
  const requestIdRef = useRef(0);

  function syncStopEntities(points: GeoPoint[]): void {
    const factory = entityFactoryRef.current;
    if (!factory) return;
    stopHandlesRef.current.forEach((handle) => factory.removeEntity(handle));
    stopHandlesRef.current = points.map((point, i) => {
      const handle = stopHandle(i);
      factory.createEntity({ type: "Point", coordinates: point }, handle.entityId, ROUTE_STOP_STYLE);
      return handle;
    });
  }

  function clearRouteEntities(): void {
    const factory = entityFactoryRef.current;
    if (!factory) return;
    if (primaryVisibleRef.current) {
      factory.removeEntity(PRIMARY_HANDLE);
      primaryVisibleRef.current = false;
    }
    altHandlesRef.current.forEach((handle) => factory.removeEntity(handle));
    altHandlesRef.current = [];
  }

  function renderRoute(legs: RouteLeg[], selected: number): void {
    const factory = entityFactoryRef.current;
    if (!factory || legs.length === 0) {
      clearRouteEntities();
      return;
    }
    altHandlesRef.current.forEach((handle) => factory.removeEntity(handle));
    altHandlesRef.current = legs
      .map((leg, i) => {
        if (i === selected) return null;
        const handle = alternativeHandle(i);
        factory.createEntity(leg.geometry, handle.entityId, ROUTE_ALTERNATIVE_STYLE);
        return handle;
      })
      .filter((h): h is EntityHandle => h !== null);

    const primary = legs[selected]!;
    if (primaryVisibleRef.current) {
      factory.updateEntity(PRIMARY_HANDLE, primary.geometry, ROUTE_STYLE);
    } else {
      factory.createEntity(primary.geometry, PRIMARY_HANDLE.entityId, ROUTE_STYLE);
      primaryVisibleRef.current = true;
    }
  }

  useEffect(() => {
    if (!viewer) return;

    const entityFactory = new CesiumEntityFactory(viewer.entities);
    entityFactoryRef.current = entityFactory;
    pickerRef.current = new CesiumScreenPicker(viewer);

    const handler = new Cesium.ScreenSpaceEventHandler(viewer.scene.canvas);
    handler.setInputAction((event: Cesium.ScreenSpaceEventHandler.PositionedEvent) => {
      if (!activeRef.current) return;
      const point = pickerRef.current?.pickGround({ x: event.position.x, y: event.position.y });
      if (!point) return;
      addStop(point);
    }, Cesium.ScreenSpaceEventType.LEFT_CLICK);

    return () => {
      handler.destroy();
      clearRouteEntities();
      stopHandlesRef.current.forEach((handle) => entityFactory.removeEntity(handle));
      stopHandlesRef.current = [];
      entityFactoryRef.current = null;
    };
  }, [viewer]);

  useEffect(() => {
    if (waypoints.length < 2) {
      requestIdRef.current++;
      setAlternatives([]);
      setSelectedIndex(0);
      setError(null);
      setLoading(false);
      renderRoute([], 0);
      return;
    }

    const requestId = ++requestIdRef.current;
    setLoading(true);
    setError(null);

    const result: Promise<RouteLeg[]> =
      mode === "train" || mode === "plane"
        ? Promise.resolve([estimateStraightLineRoute(waypoints, mode)])
        : routingProvider
          ? routingProvider.route(waypoints, mode)
          : Promise.reject(new Error("No routing provider configured"));

    result
      .then((legs) => {
        if (requestIdRef.current !== requestId) return;
        setAlternatives(legs);
        setSelectedIndex(0);
        setLoading(false);
        renderRoute(legs, 0);
        if (viewer && legs[0]) flyToGeometry(viewer, legs[0].geometry);
      })
      .catch((err: unknown) => {
        if (requestIdRef.current !== requestId) return;
        setAlternatives([]);
        setError(err instanceof Error ? err.message : String(err));
        setLoading(false);
        renderRoute([], 0);
      });
  }, [waypoints, mode, routingProvider]);

  function start(): void {
    activeRef.current = true;
    setActive(true);
  }

  function finish(): void {
    activeRef.current = false;
    setActive(false);
  }

  function cancel(): void {
    controllerRef.current.clear();
    activeRef.current = false;
    setActive(false);
    setWaypoints([]);
    setStopMeta([]);
    syncStopEntities([]);
  }

  function addStop(point: GeoPoint, label?: string): void {
    const id = crypto.randomUUID();
    controllerRef.current.addWaypoint(point);
    const points = controllerRef.current.getWaypoints();
    setWaypoints(points);
    setStopMeta((prev) => [...prev, { id, label: label ?? null }]);
    syncStopEntities(points);

    if (label === undefined && geocodingProviderRef.current) {
      void geocodingProviderRef.current.reverse(point).then((resolved) => {
        if (resolved === null) return;
        setStopMeta((prev) => prev.map((meta) => (meta.id === id ? { ...meta, label: resolved } : meta)));
      });
    }
  }

  function removeStop(index: number): void {
    controllerRef.current.removeWaypoint(index);
    const points = controllerRef.current.getWaypoints();
    setWaypoints(points);
    setStopMeta((prev) => prev.filter((_, i) => i !== index));
    syncStopEntities(points);
  }

  function reorderStop(from: number, to: number): void {
    controllerRef.current.moveWaypoint(from, to);
    const points = controllerRef.current.getWaypoints();
    setWaypoints(points);
    setStopMeta((prev) => {
      const next = [...prev];
      const [moved] = next.splice(from, 1);
      if (moved) next.splice(to, 0, moved);
      return next;
    });
    syncStopEntities(points);
  }

  function selectAlternative(index: number): void {
    if (index < 0 || index >= alternatives.length) return;
    setSelectedIndex(index);
    renderRoute(alternatives, index);
  }

  const primaryLeg = alternatives[selectedIndex];
  const stops: RouteStop[] = waypoints.map((point, i) => ({
    id: stopMeta[i]?.id ?? `${i}`,
    point,
    label: stopMeta[i]?.label ?? null,
  }));

  return {
    active,
    stops,
    mode,
    alternatives,
    selectedIndex,
    totalDistanceMeters: primaryLeg?.distanceMeters ?? 0,
    totalDurationSeconds: primaryLeg?.durationSeconds ?? 0,
    loading,
    error,
    setMode,
    selectAlternative,
    start,
    finish,
    cancel,
    addStop,
    removeStop,
    reorderStop,
  };
}
