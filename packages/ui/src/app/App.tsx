import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type * as Cesium from "cesium";
import {
  createPointGeometry,
  DEFAULT_MARKER_ICON,
  NominatimGeocodingProvider,
  OsrmRoutingProvider,
  type GeocodeResult,
  type GeocodingProvider,
  type GeocodingProviderConfig,
  type LineStringGeometry,
  type RoutingProvider,
  type RoutingProviderConfig,
} from "@terra-globe/core";
import {
  flyToGeometry,
  BUILTIN_TILE_SOURCES,
  type TileSource,
  type PlacemarkStyleEdits,
} from "@terra-globe/map";
import { CesiumViewerHost } from "./CesiumViewerHost.js";
import { DrawingToolbar } from "../features/drawing-toolbar/DrawingToolbar.js";
import { useDrawing } from "../features/drawing-toolbar/useDrawing.js";
import { AddressSearchBox } from "../features/geocoding/AddressSearchBox.js";
import { useGeocoding } from "../features/geocoding/useGeocoding.js";
import { FolderTree } from "../features/folders/FolderTree.js";
import { useLibrary } from "../features/folders/useLibrary.js";
import { UndoRedoToolbar } from "../features/undo-redo/UndoRedoToolbar.js";
import { useUndoRedo } from "../features/undo-redo/useUndoRedo.js";
import { ScreenOverlayLayer } from "../features/screen-overlays/ScreenOverlayLayer.js";
import { HeightProfilePanel } from "../features/height-profile/HeightProfilePanel.js";
import { ImportExportToolbar } from "../features/import-export/ImportExportToolbar.js";
import { RulerToolbar } from "../features/ruler/RulerToolbar.js";
import { RulerPanel } from "../features/ruler/RulerPanel.js";
import { useRuler } from "../features/ruler/useRuler.js";
import { RoutePlannerToolbar } from "../features/route-planner/RoutePlannerToolbar.js";
import { RoutePlannerPanel } from "../features/route-planner/RoutePlannerPanel.js";
import { useRoutePlanner } from "../features/route-planner/useRoutePlanner.js";
import { useImportExport } from "../features/import-export/useImportExport.js";
import { Notice, type NoticeData } from "../features/notice/Notice.js";
import { PlacemarkEditor } from "../features/placemark-editor/PlacemarkEditor.js";
import { SettingsModal } from "../features/settings/SettingsModal.js";
import { useSettings } from "../features/settings/useSettings.js";
import i18n from "../i18n/i18n.js";
import { useFloatingPanel } from "./useFloatingPanel.js";
import { useResizableWidth } from "./useResizableWidth.js";
import { createGeocodingProvider } from "../platform/createGeocodingProvider.js";
import { createRoutingProvider } from "../platform/createRoutingProvider.js";
import { createSecretStore } from "../platform/createSecretStore.js";
import { buildTileSources } from "../platform/tileProviderRegistry.js";

const DEFAULT_STYLE: PlacemarkStyleEdits = {
  outlineEnabled: true,
  outlineColor: "#ff0000",
  outlineWidth: 2,
  filled: false,
  fillColor: "#ff0000",
  markerIcon: DEFAULT_MARKER_ICON,
};

export function App() {
  const { t } = useTranslation();
  const defaultTileSource = BUILTIN_TILE_SOURCES[0]!;
  const [baseLayerId, setBaseLayerId] = useState<string>(defaultTileSource.id);
  const [tileSources, setTileSources] = useState<TileSource[]>(BUILTIN_TILE_SOURCES);
  const baseLayer: TileSource =
    tileSources.find((source) => source.id === baseLayerId) ?? defaultTileSource;
  const [viewer, setViewer] = useState<Cesium.Viewer | null>(null);
  const [selectedPlacemarkId, setSelectedPlacemarkId] = useState<string | null>(null);
  const [editorStyle, setEditorStyle] = useState<PlacemarkStyleEdits>(DEFAULT_STYLE);
  const [notice, setNotice] = useState<NoticeData | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const sidebar = useResizableWidth(260, 200, 600, "terra-globe:sidebarWidth");
  const topbarRef = useRef<HTMLDivElement>(null);
  const [placesPanelTop, setPlacesPanelTop] = useState(76);

  useLayoutEffect(() => {
    const topbar = topbarRef.current;
    if (!topbar) return;
    // The topbar wraps onto extra rows once its controls no longer fit one
    // line (e.g. a narrower window, or another toolbar added later) - the
    // places panel below it must follow that actual height instead of a
    // fixed offset, or the two overlap and swallow clicks meant for the row
    // that wrapped underneath it.
    const observer = new ResizeObserver(([entry]) => {
      if (entry) setPlacesPanelTop(entry.target.getBoundingClientRect().bottom + 12);
    });
    observer.observe(topbar);
    return () => observer.disconnect();
  }, []);
  const placemarkPanel = useFloatingPanel({
    initial: { x: window.innerWidth - 260 - 12, y: 56, width: 260, height: 480 },
    minWidth: 240,
    minHeight: 200,
    maxWidth: 600,
    maxHeight: 800,
    storageKey: "terra-globe:placemarkEditorGeometry",
  });
  const heightProfilePanel = useFloatingPanel({
    initial: { x: window.innerWidth - 260 - 12, y: 550, width: 420, height: 260 },
    minWidth: 300,
    minHeight: 180,
    maxWidth: 900,
    maxHeight: 600,
    storageKey: "terra-globe:heightProfilePanelGeometry",
  });
  const rulerPanel = useFloatingPanel({
    initial: { x: window.innerWidth - 260 - 12, y: 56, width: 240, height: 200 },
    minWidth: 200,
    minHeight: 120,
    maxWidth: 480,
    maxHeight: 600,
    storageKey: "terra-globe:rulerPanelGeometry",
  });
  const routePlannerPanel = useFloatingPanel({
    initial: { x: window.innerWidth - 300 - 12, y: 56, width: 300, height: 260 },
    minWidth: 240,
    minHeight: 160,
    maxWidth: 480,
    maxHeight: 600,
    storageKey: "terra-globe:routePlannerPanelGeometry",
  });
  const secretStore = useRef(createSecretStore()).current;
  const settings = useSettings(secretStore);

  useEffect(() => {
    void i18n.changeLanguage(settings.language);
  }, [settings.language]);

  const [geocodingProvider, setGeocodingProvider] = useState<GeocodingProvider>(
    () => new NominatimGeocodingProvider(),
  );
  const geocoding = useGeocoding(geocodingProvider);
  const routeStopSearch = useGeocoding(geocodingProvider);
  const [routingProvider, setRoutingProvider] = useState<RoutingProvider>(
    () => new OsrmRoutingProvider(),
  );

  useEffect(() => {
    let cancelled = false;
    buildTileSources(settings.providers, secretStore).then((sources) => {
      if (!cancelled) setTileSources(sources);
    });
    return () => {
      cancelled = true;
    };
  }, [settings.providers, secretStore]);

  useEffect(() => {
    let cancelled = false;
    const active = settings.providers.find(
      (p): p is GeocodingProviderConfig => p.kind === "geocoding" && p.enabled,
    );
    createGeocodingProvider(active, secretStore).then((provider) => {
      if (!cancelled) setGeocodingProvider(provider);
    });
    return () => {
      cancelled = true;
    };
  }, [settings.providers, secretStore]);

  useEffect(() => {
    let cancelled = false;
    const active = settings.providers.find(
      (p): p is RoutingProviderConfig => p.kind === "routing" && p.enabled,
    );
    createRoutingProvider(active, secretStore).then((provider) => {
      if (!cancelled) setRoutingProvider(provider);
    });
    return () => {
      cancelled = true;
    };
  }, [settings.providers, secretStore]);
  const onReadyRef = useRef((handle: { viewer: Cesium.Viewer }) => {
    setViewer(handle.viewer);
    // Exposed for E2E tests to inspect camera state (e.g. after a fly-to);
    // harmless in production, mirrors the common devtools-debugging pattern.
    (window as unknown as { __terraGlobeViewer?: Cesium.Viewer }).__terraGlobeViewer =
      handle.viewer;
    // Repurpose Cesium's built-in "home" button: cancel its default
    // fly-to-home behavior and open the Settings modal instead.
    handle.viewer.homeButton.viewModel.command.beforeExecute.addEventListener((commandInfo) => {
      commandInfo.cancel = true;
      setSettingsOpen(true);
    });
  });

  const library = useLibrary(viewer);
  const undoRedo = useUndoRedo(library);
  const ruler = useRuler(viewer);
  const routePlanner = useRoutePlanner(viewer, routingProvider, geocodingProvider);
  const { mode, selectTool, finish, cancel } = useDrawing(
    viewer,
    (geometry) => {
      void undoRedo
        .wrap(() => library.addPlacemark(geometry))
        .then((id) => {
          if (id) setSelectedPlacemarkId(id);
        });
    },
    (entityId) => {
      if (library.placemarks.some((p) => p.id === entityId)) setSelectedPlacemarkId(entityId);
    },
  );
  const importExport = useImportExport(library, undoRedo.wrap);

  // The placemark editor keeps live-previewing the selected entity while
  // open; a restore can remove or replace that entity out from under it
  // (previewPlacemark() would then throw on the now-gone Cesium entity), so
  // close it before time-travelling, same as a plain delete does.
  function runUndo() {
    setSelectedPlacemarkId(null);
    void undoRedo.undo();
  }
  function runRedo() {
    setSelectedPlacemarkId(null);
    void undoRedo.redo();
  }

  useEffect(() => {
    function isTextInput(target: EventTarget | null): boolean {
      if (!(target instanceof HTMLElement)) return false;
      return (
        target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable
      );
    }
    function onKeyDown(e: KeyboardEvent) {
      if (!(e.ctrlKey || e.metaKey) || isTextInput(e.target)) return;
      const key = e.key.toLowerCase();
      if (key === "z" && e.shiftKey) {
        e.preventDefault();
        runRedo();
      } else if (key === "z") {
        e.preventDefault();
        runUndo();
      } else if (key === "y") {
        e.preventDefault();
        runRedo();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [undoRedo]);

  const editingPlacemark = library.placemarks.find((p) => p.id === selectedPlacemarkId) ?? null;
  const [elevationProfilePlacemarkId, setElevationProfilePlacemarkId] = useState<string | null>(
    null,
  );
  const showHeightProfile =
    elevationProfilePlacemarkId !== null && elevationProfilePlacemarkId === selectedPlacemarkId;
  const profileTrack =
    showHeightProfile && editingPlacemark?.geometry.type === "LineString" ? editingPlacemark : null;

  useEffect(() => {
    if (!selectedPlacemarkId) return;
    let cancelled = false;
    library.getPlacemarkStyle(selectedPlacemarkId).then((style) => {
      if (!cancelled) setEditorStyle(style);
    });
    return () => {
      cancelled = true;
    };
  }, [selectedPlacemarkId]);

  return (
    <div className="app-shell" data-app-ready={library.ready}>
      <div className="app-topbar" ref={topbarRef}>
        <label className="base-layer-select">
          {t("app.basemap")}
          <select value={baseLayerId} onChange={(e) => setBaseLayerId(e.target.value)}>
            {tileSources.map((source) => (
              <option key={source.id} value={source.id}>
                {source.name}
              </option>
            ))}
          </select>
        </label>
        <UndoRedoToolbar
          canUndo={undoRedo.canUndo}
          canRedo={undoRedo.canRedo}
          onUndo={runUndo}
          onRedo={runRedo}
        />
        <DrawingToolbar
          mode={mode}
          disabled={!library.ready}
          onSelectTool={(tool) => {
            ruler.cancel();
            routePlanner.finish();
            selectTool(tool);
          }}
          onFinish={finish}
          onCancel={cancel}
        />
        <RulerToolbar
          active={ruler.active}
          disabled={!library.ready}
          vertexCount={ruler.vertexCount}
          onStart={() => {
            cancel();
            routePlanner.finish();
            ruler.start();
          }}
          onUndo={ruler.undo}
          onFinish={ruler.finish}
          onCancel={ruler.cancel}
        />
        <RoutePlannerToolbar
          active={routePlanner.active}
          disabled={!library.ready}
          onToggle={() => {
            if (routePlanner.active) {
              routePlanner.finish();
              return;
            }
            cancel();
            ruler.cancel();
            routePlanner.start();
          }}
        />
        <AddressSearchBox
          disabled={!viewer}
          status={geocoding.status}
          results={geocoding.results}
          error={geocoding.error}
          onSearch={(query) => void geocoding.search(query)}
          onSelectResult={(result: GeocodeResult) => {
            if (viewer) flyToGeometry(viewer, createPointGeometry(result.point));
            geocoding.reset();
          }}
        />
        <ImportExportToolbar
          disabled={!library.ready}
          onImportFile={(file) => {
            importExport
              .importFile(file)
              .then((summary) => {
                const parts = [
                  t("importExport.importedPlacemarks", { count: summary.placemarksImported }),
                  summary.foldersImported > 0
                    ? t("importExport.importedFolders", { count: summary.foldersImported })
                    : null,
                ].filter(Boolean);
                const message =
                  summary.warnings.length > 0
                    ? `${parts.join(", ")}. ${t("importExport.skippedItems", { count: summary.warnings.length, warnings: summary.warnings.join(" ") })}`
                    : `${parts.join(", ")}.`;
                setNotice({ level: "success", message });
              })
              .catch((err: unknown) => {
                setNotice({
                  level: "error",
                  message: err instanceof Error ? err.message : String(err),
                });
              });
          }}
          onExportKml={() => void importExport.exportKml()}
          onExportKmz={() => void importExport.exportKmz()}
        />
      </div>
      {notice && (
        <div className="notice-container">
          <Notice notice={notice} onDismiss={() => setNotice(null)} />
        </div>
      )}
      <div
        className="places-panel"
        style={{
          top: placesPanelTop,
          width: sidebar.width,
          maxHeight: `calc(100vh - ${placesPanelTop}px - 16px)`,
        }}
      >
        <div className="places-panel-header">{t("folders.placesHeader")}</div>
        <div className="places-panel-content">
          <FolderTree
            disabled={!library.ready}
            folders={library.folders}
            placemarks={library.placemarks}
            selectedFolderId={library.selectedFolderId}
            selectedPlacemarkId={selectedPlacemarkId}
            onSelectFolder={library.selectFolder}
            onSelectPlacemark={setSelectedPlacemarkId}
            onFlyToPlacemark={(id) => {
              const placemark = library.placemarks.find((p) => p.id === id);
              if (placemark && viewer) flyToGeometry(viewer, placemark.geometry);
            }}
            onCreateFolder={(parentId, name) =>
              void undoRedo.wrap(() => library.createFolder(parentId, name))
            }
            onRenameFolder={(id, name) => void undoRedo.wrap(() => library.renameFolder(id, name))}
            onDeleteFolder={(id) => void undoRedo.wrap(() => library.deleteFolder(id))}
            onToggleFolderVisibility={(id) =>
              void undoRedo.wrap(() => library.toggleFolderVisibility(id))
            }
            onMoveFolder={(id, parentId, index) =>
              void undoRedo.wrap(() => library.moveFolder(id, parentId, index))
            }
            onDeletePlacemark={(id) => {
              void undoRedo.wrap(() => library.deletePlacemark(id));
              if (id === selectedPlacemarkId) setSelectedPlacemarkId(null);
            }}
            onTogglePlacemarkVisibility={(id) =>
              void undoRedo.wrap(() => library.togglePlacemarkVisibility(id))
            }
            onMovePlacemark={(id, folderId, index) =>
              void undoRedo.wrap(() => library.movePlacemark(id, folderId, index))
            }
          />
        </div>
        <div
          className={
            sidebar.isResizing
              ? "places-panel-resize-handle resizing"
              : "places-panel-resize-handle"
          }
          onMouseDown={sidebar.startResize}
        />
      </div>
      {editingPlacemark && (
        <div
          className="placemark-editor-panel"
          style={{
            left: placemarkPanel.geometry.x,
            top: placemarkPanel.geometry.y,
            width: placemarkPanel.geometry.width,
            height: placemarkPanel.geometry.height,
          }}
        >
          <PlacemarkEditor
            key={editingPlacemark.id}
            placemark={editingPlacemark}
            style={editorStyle}
            unitSystem={settings.unitSystem}
            coordinateFormat={settings.coordinateFormat}
            onDragStart={placemarkPanel.startDrag}
            onPreview={(patch) => library.previewPlacemarkEdits(editingPlacemark.id, patch)}
            onShowElevationProfile={() => setElevationProfilePlacemarkId(editingPlacemark.id)}
            onClose={() => setSelectedPlacemarkId(null)}
            onDelete={() => {
              // Wait for the delete (including its internal refresh()) before
              // closing, same reasoning as onSave below - otherwise the tree
              // can still show the placemark for a moment after the editor
              // has already closed.
              void undoRedo
                .wrap(() => library.deletePlacemark(editingPlacemark.id))
                .then(() => setSelectedPlacemarkId(null));
            }}
            onSave={({ name, description, style }) => {
              // Wait for the save (including its internal refresh()) before
              // closing - closing immediately (fire-and-forget) let a user
              // re-open the same placemark mid-save and see stale pre-save
              // data, since `placemarks` state hadn't caught up yet.
              void undoRedo
                .wrap(() =>
                  library.savePlacemarkEdits(editingPlacemark.id, { name, description, style }),
                )
                .then(() => setSelectedPlacemarkId(null));
            }}
          />
          <div
            className={
              placemarkPanel.isResizing
                ? "placemark-editor-resize-handle resizing"
                : "placemark-editor-resize-handle"
            }
            onMouseDown={placemarkPanel.startResize}
          />
        </div>
      )}
      {profileTrack && (
        <div
          className="height-profile-panel"
          style={{
            left: heightProfilePanel.geometry.x,
            top: heightProfilePanel.geometry.y,
            width: heightProfilePanel.geometry.width,
            height: heightProfilePanel.geometry.height,
          }}
        >
          <HeightProfilePanel
            key={profileTrack.id}
            trackName={profileTrack.name}
            geometry={profileTrack.geometry as LineStringGeometry}
            unitSystem={settings.unitSystem}
            width={heightProfilePanel.geometry.width}
            height={heightProfilePanel.geometry.height}
            onDragStart={heightProfilePanel.startDrag}
            onClose={() => setElevationProfilePlacemarkId(null)}
          />
          <div
            className={
              heightProfilePanel.isResizing
                ? "placemark-editor-resize-handle resizing"
                : "placemark-editor-resize-handle"
            }
            onMouseDown={heightProfilePanel.startResize}
          />
        </div>
      )}
      {ruler.segments.length > 0 && (
        <div
          className="ruler-panel-panel"
          style={{
            left: rulerPanel.geometry.x,
            top: rulerPanel.geometry.y,
            width: rulerPanel.geometry.width,
            height: rulerPanel.geometry.height,
          }}
        >
          <RulerPanel
            segments={ruler.segments}
            totalMeters={ruler.totalMeters}
            unitSystem={settings.unitSystem}
            onDragStart={rulerPanel.startDrag}
            onClose={ruler.cancel}
          />
          <div
            className={
              rulerPanel.isResizing
                ? "placemark-editor-resize-handle resizing"
                : "placemark-editor-resize-handle"
            }
            onMouseDown={rulerPanel.startResize}
          />
        </div>
      )}
      <div
        className="route-planner-panel-panel"
        // aria-hidden (not just display:none) so the hidden panel's inputs -
        // e.g. its own "Address" search box - don't collide with visible
        // same-labeled controls elsewhere when the panel is closed.
        aria-hidden={!routePlanner.active}
        style={{
          display: routePlanner.active ? undefined : "none",
          left: routePlannerPanel.geometry.x,
          top: routePlannerPanel.geometry.y,
          width: routePlannerPanel.geometry.width,
          height: routePlannerPanel.geometry.height,
        }}
      >
        <RoutePlannerPanel
          stops={routePlanner.stops}
          mode={routePlanner.mode}
          alternatives={routePlanner.alternatives}
          selectedIndex={routePlanner.selectedIndex}
          totalDistanceMeters={routePlanner.totalDistanceMeters}
          totalDurationSeconds={routePlanner.totalDurationSeconds}
          loading={routePlanner.loading}
          error={routePlanner.error}
          unitSystem={settings.unitSystem}
          searchStatus={routeStopSearch.status}
          searchResults={routeStopSearch.results}
          searchError={routeStopSearch.error}
          onDragStart={routePlannerPanel.startDrag}
          onClose={routePlanner.finish}
          onClear={routePlanner.clear}
          onChangeMode={routePlanner.setMode}
          onSearch={(query) => void routeStopSearch.search(query)}
          onSelectSearchResult={(result: GeocodeResult) => {
            routePlanner.addStop(result.point, result.label);
            routeStopSearch.reset();
          }}
          onRemoveStop={routePlanner.removeStop}
          onMoveStop={routePlanner.reorderStop}
          onSelectAlternative={routePlanner.selectAlternative}
        />
        <div
          className={
            routePlannerPanel.isResizing
              ? "placemark-editor-resize-handle resizing"
              : "placemark-editor-resize-handle"
          }
          onMouseDown={routePlannerPanel.startResize}
        />
      </div>
      {settingsOpen && (
        <SettingsModal
          unitSystem={settings.unitSystem}
          coordinateFormat={settings.coordinateFormat}
          language={settings.language}
          onChangeUnitSystem={settings.setUnitSystem}
          onChangeCoordinateFormat={settings.setCoordinateFormat}
          onChangeLanguage={settings.setLanguage}
          providers={settings.providers}
          secretStore={secretStore}
          onAddProvider={settings.addProvider}
          onSetProviderEnabled={settings.setProviderEnabled}
          onRemoveProvider={settings.removeProvider}
          onClose={() => setSettingsOpen(false)}
        />
      )}
      <CesiumViewerHost baseLayer={baseLayer} onReady={onReadyRef.current} />
      <ScreenOverlayLayer overlays={library.screenOverlays} />
    </div>
  );
}
