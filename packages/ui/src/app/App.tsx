import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Info, Settings as SettingsIcon } from "lucide-react";
import type * as Cesium from "cesium";
import { SceneMode } from "cesium";
import {
  createPointGeometry,
  geometryCenter,
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
import { useDragToMove } from "../features/drag-to-move/useDragToMove.js";
import { AddressSearchBox } from "../features/geocoding/AddressSearchBox.js";
import { useGeocoding } from "../features/geocoding/useGeocoding.js";
import { FolderTree } from "../features/folders/FolderTree.js";
import { useLibrary } from "../features/folders/useLibrary.js";
import { UndoRedoToolbar } from "../features/undo-redo/UndoRedoToolbar.js";
import { ViewMenu } from "../features/scene-mode/ViewMenu.js";
import { BaseLayerMenu } from "../features/basemap/BaseLayerMenu.js";
import { useUndoRedo } from "../features/undo-redo/useUndoRedo.js";
import { ScreenOverlayLayer } from "../features/screen-overlays/ScreenOverlayLayer.js";
import { HeightProfilePanel } from "../features/height-profile/HeightProfilePanel.js";
import { FileMenu } from "../features/import-export/FileMenu.js";
import { RulerToolbar } from "../features/ruler/RulerToolbar.js";
import { RulerPanel } from "../features/ruler/RulerPanel.js";
import { useRuler } from "../features/ruler/useRuler.js";
import { AreaExportToolbar } from "../features/area-export/AreaExportToolbar.js";
import { AreaExportPanel } from "../features/area-export/AreaExportPanel.js";
import { useAreaExport } from "../features/area-export/useAreaExport.js";
import { useDistanceRings } from "../features/placemark-editor/useDistanceRings.js";
import { RoutePlannerToolbar } from "../features/route-planner/RoutePlannerToolbar.js";
import { RoutePlannerPanel } from "../features/route-planner/RoutePlannerPanel.js";
import { useRoutePlanner } from "../features/route-planner/useRoutePlanner.js";
import { useImportExport } from "../features/import-export/useImportExport.js";
import { Notice, type NoticeData } from "../features/notice/Notice.js";
import { PlacemarkEditor } from "../features/placemark-editor/PlacemarkEditor.js";
import { SettingsModal } from "../features/settings/SettingsModal.js";
import { AboutModal } from "../features/about/AboutModal.js";
import { useSettings } from "../features/settings/useSettings.js";
import { useFileSync } from "../features/file-sync/useFileSync.js";
import { useExitConfirm } from "../features/file-sync/useExitConfirm.js";
import { ConfirmModal } from "../features/confirm/ConfirmModal.js";
import i18n from "../i18n/i18n.js";
import { useFloatingPanel } from "./useFloatingPanel.js";
import { useResizableWidth } from "./useResizableWidth.js";
import { useSavedCameraView } from "./useSavedCameraView.js";
import { createGeocodingProvider } from "../platform/createGeocodingProvider.js";
import { createRoutingProvider } from "../platform/createRoutingProvider.js";
import { createSecretStore } from "../platform/createSecretStore.js";
import { buildTileSources } from "../platform/tileProviderRegistry.js";

export function App() {
  const { t } = useTranslation();
  const defaultTileSource = BUILTIN_TILE_SOURCES[0]!;
  const [baseLayerId, setBaseLayerId] = useState<string>(defaultTileSource.id);
  const [tileSources, setTileSources] = useState<TileSource[]>(BUILTIN_TILE_SOURCES);
  const [sceneMode, setSceneMode] = useState<Cesium.SceneMode>(SceneMode.SCENE3D);
  const baseLayer: TileSource =
    tileSources.find((source) => source.id === baseLayerId) ?? defaultTileSource;
  const [viewer, setViewer] = useState<Cesium.Viewer | null>(null);
  const [selectedPlacemarkId, setSelectedPlacemarkId] = useState<string | null>(null);
  const [editorStyle, setEditorStyle] = useState<PlacemarkStyleEdits | null>(null);
  const [notice, setNotice] = useState<NoticeData | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [loadConfirmOpen, setLoadConfirmOpen] = useState(false);
  const sidebar = useResizableWidth(260, 200, 600, "terra-globe:sidebarWidth");
  const topbarRef = useRef<HTMLDivElement>(null);
  const [placesPanelTop, setPlacesPanelTop] = useState(76);
  const [placesSearchQuery, setPlacesSearchQuery] = useState("");

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
  const areaExportPanel = useFloatingPanel({
    initial: { x: window.innerWidth - 260 - 12, y: 56, width: 260, height: 320 },
    minWidth: 220,
    minHeight: 260,
    maxWidth: 480,
    maxHeight: 600,
    storageKey: "terra-globe:areaExportPanelGeometry",
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
  });

  useSavedCameraView(viewer);
  const library = useLibrary(viewer);
  const undoRedo = useUndoRedo(library);
  const fileSync = useFileSync(library, settings.autoSave);
  // Every persisted mutation goes through both: undoRedo.wrap snapshots
  // before/after for undo/redo, fileSync.wrap marks the file dirty and
  // schedules an autosave. Order matters - fileSync must see the
  // already-undo-tracked action's result, not wrap undoRedo itself.
  const wrap = useCallback(
    <T,>(action: () => Promise<T>): Promise<T> => fileSync.wrap(() => undoRedo.wrap(action)),
    [fileSync, undoRedo],
  );
  const exitConfirm = useExitConfirm(fileSync.dirty, settings.autoSave, fileSync.saveNow);
  const ruler = useRuler(viewer);
  const areaExport = useAreaExport(viewer);
  const distanceRings = useDistanceRings(viewer);
  const routePlanner = useRoutePlanner(viewer, routingProvider, geocodingProvider);
  const { mode, selectTool, finish, cancel } = useDrawing(
    viewer,
    (geometry) => {
      void wrap(() => library.addPlacemark(geometry)).then((id) => {
        if (id) setSelectedPlacemarkId(id);
      });
    },
    (entityId) => {
      if (library.placemarks.some((p) => p.id === entityId)) setSelectedPlacemarkId(entityId);
    },
  );
  const importExport = useImportExport(library, wrap);

  // The placemark editor keeps live-previewing the selected entity while
  // open; a restore can remove or replace that entity out from under it
  // (previewPlacemark() would then throw on the now-gone Cesium entity), so
  // close it before time-travelling, same as a plain delete does.
  function runUndo() {
    setSelectedPlacemarkId(null);
    // Uses fileSync.wrap directly (not the composed `wrap`) - undo/redo
    // already manage their own history stacks via undoRedo.undo(), so
    // routing through undoRedo.wrap too would record a spurious new entry.
    void fileSync.wrap(() => undoRedo.undo());
  }
  function runRedo() {
    setSelectedPlacemarkId(null);
    void fileSync.wrap(() => undoRedo.redo());
  }

  async function performLoad() {
    const result = await fileSync.loadFromDisk();
    if (!result) return;
    try {
      await undoRedo.wrap(() => library.restoreSnapshot(result.snapshot));
      fileSync.onLoaded(result.filePath);
      if (result.warnings.length > 0) {
        setNotice({
          level: "error",
          message: t("importExport.skippedItems", {
            count: result.warnings.length,
            warnings: result.warnings.join(" "),
          }),
        });
      }
    } catch (err) {
      setNotice({
        level: "error",
        message: err instanceof Error ? err.message : String(err),
      });
    }
  }

  function onLoad() {
    if (fileSync.dirty && !settings.autoSave) {
      setLoadConfirmOpen(true);
      return;
    }
    void performLoad();
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
  }, [undoRedo, fileSync]);

  const editingPlacemark = library.placemarks.find((p) => p.id === selectedPlacemarkId) ?? null;
  const [elevationProfilePlacemarkId, setElevationProfilePlacemarkId] = useState<string | null>(
    null,
  );
  const showHeightProfile =
    elevationProfilePlacemarkId !== null && elevationProfilePlacemarkId === selectedPlacemarkId;
  const profileTrack =
    showHeightProfile && editingPlacemark?.geometry.type === "LineString" ? editingPlacemark : null;

  useEffect(() => {
    // Clear synchronously on every selection change (including deselection) -
    // the editor only mounts once editorStyle is non-null (see the JSX
    // below), so this guarantees it never mounts for a new placemark using
    // the PREVIOUS one's style. Resolving that gap by falling back to a
    // default style instead would just trade one wrong color (stale) for
    // another (default) - not mounting the editor at all until the correct
    // style has loaded is the only way to never preview a wrong color.
    setEditorStyle(null);
    if (!selectedPlacemarkId) return;
    let cancelled = false;
    library.getPlacemarkStyle(selectedPlacemarkId).then((style) => {
      if (!cancelled) setEditorStyle(style);
    });
    return () => {
      cancelled = true;
    };
  }, [selectedPlacemarkId]);

  useDragToMove(
    viewer,
    editingPlacemark,
    mode !== "idle" || ruler.active || routePlanner.active || areaExport.active,
    (id) => (editorStyle ? library.beginPlacemarkDrag(id, editorStyle) : undefined),
    (id, geometry) => {
      void wrap(() => library.updatePlacemarkGeometry(id, geometry));
    },
  );

  return (
    <div className="app-shell" data-app-ready={library.ready}>
      <div className="app-topbar" ref={topbarRef}>
        <FileMenu
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
          onLoad={onLoad}
          onSave={() => void fileSync.saveNow()}
          saveDisabled={!fileSync.dirty}
        />
        <BaseLayerMenu
          tileSources={tileSources}
          baseLayerId={baseLayerId}
          onChange={setBaseLayerId}
        />
        <ViewMenu mode={sceneMode} onChange={setSceneMode} />
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
            areaExport.cancel();
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
            areaExport.cancel();
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
            areaExport.cancel();
            routePlanner.start();
          }}
        />
        <AreaExportToolbar
          active={areaExport.active}
          disabled={!library.ready}
          onStart={() => {
            cancel();
            ruler.cancel();
            routePlanner.finish();
            areaExport.start();
          }}
          onCancel={areaExport.cancel}
        />
        <AddressSearchBox
          disabled={!viewer}
          hideLabel
          status={geocoding.status}
          results={geocoding.results}
          error={geocoding.error}
          onSearch={(query) => void geocoding.search(query)}
          onSelectResult={(result: GeocodeResult) => {
            if (viewer) flyToGeometry(viewer, createPointGeometry(result.point));
            geocoding.reset();
          }}
        />
        <div className="toolbar-group topbar-actions">
          <button type="button" className="btn" onClick={() => setSettingsOpen(true)}>
            <SettingsIcon size={16} aria-hidden="true" />
            {t("app.settings")}
          </button>
          <button
            type="button"
            className="btn"
            aria-label={t("app.about")}
            title={t("app.about")}
            onClick={() => setAboutOpen(true)}
          >
            <Info size={18} aria-hidden="true" />
          </button>
        </div>
      </div>
      {notice && (
        <div className="notice-container">
          <Notice notice={notice} onDismiss={() => setNotice(null)} />
        </div>
      )}
      {areaExport.exporting && (
        <div className="area-export-overlay">
          {areaExport.progress && areaExport.progress.total > 1
            ? t("areaExport.progress", {
                done: areaExport.progress.done,
                total: areaExport.progress.total,
              })
            : t("areaExport.generating")}
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
          <div className="places-panel-search">
            <input
              type="search"
              value={placesSearchQuery}
              disabled={!library.ready}
              placeholder={t("folders.searchPlaceholder")}
              onChange={(e) => setPlacesSearchQuery(e.target.value)}
            />
          </div>
          <FolderTree
            disabled={!library.ready}
            folders={library.folders}
            placemarks={library.placemarks}
            selectedFolderId={library.selectedFolderId}
            selectedPlacemarkId={selectedPlacemarkId}
            searchQuery={placesSearchQuery}
            onSelectFolder={library.selectFolder}
            onSelectPlacemark={setSelectedPlacemarkId}
            onFlyToPlacemark={(id) => {
              const placemark = library.placemarks.find((p) => p.id === id);
              if (placemark && viewer) flyToGeometry(viewer, placemark.geometry);
            }}
            onCreateFolder={(parentId, name) =>
              void wrap(() => library.createFolder(parentId, name))
            }
            onRenameFolder={(id, name) => void wrap(() => library.renameFolder(id, name))}
            onDeleteFolder={(id) => void wrap(() => library.deleteFolder(id))}
            onToggleFolderVisibility={(id) => void wrap(() => library.toggleFolderVisibility(id))}
            onMoveFolder={(id, parentId, index) =>
              void wrap(() => library.moveFolder(id, parentId, index))
            }
            onDeletePlacemark={(id) => {
              void wrap(() => library.deletePlacemark(id));
              if (id === selectedPlacemarkId) setSelectedPlacemarkId(null);
            }}
            onTogglePlacemarkVisibility={(id) =>
              void wrap(() => library.togglePlacemarkVisibility(id))
            }
            onMovePlacemark={(id, folderId, index) =>
              void wrap(() => library.movePlacemark(id, folderId, index))
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
      {editingPlacemark && editorStyle && (
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
            onDistanceRingsChange={(rings) =>
              distanceRings.set(
                geometryCenter(editingPlacemark.geometry),
                rings,
                settings.unitSystem,
              )
            }
            onClose={() => setSelectedPlacemarkId(null)}
            onDelete={() => {
              // Wait for the delete (including its internal refresh()) before
              // closing, same reasoning as onSave below - otherwise the tree
              // can still show the placemark for a moment after the editor
              // has already closed.
              void wrap(() => library.deletePlacemark(editingPlacemark.id)).then(() =>
                setSelectedPlacemarkId(null),
              );
            }}
            onSave={({ name, description, style, visibility, geometry }) => {
              // Wait for the save (including its internal refresh()) before
              // closing - closing immediately (fire-and-forget) let a user
              // re-open the same placemark mid-save and see stale pre-save
              // data, since `placemarks` state hadn't caught up yet.
              void wrap(() =>
                library.savePlacemarkEdits(editingPlacemark.id, {
                  name,
                  description,
                  style,
                  visibility,
                  geometry,
                }),
              ).then(() => setSelectedPlacemarkId(null));
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
      {areaExport.active && (
        <div
          className="area-export-panel-panel"
          style={{
            left: areaExportPanel.geometry.x,
            top: areaExportPanel.geometry.y,
            width: areaExportPanel.geometry.width,
            height: areaExportPanel.geometry.height,
          }}
        >
          <AreaExportPanel
            bounds={areaExport.bounds}
            scaleDenominator={areaExport.scaleDenominator}
            dpi={areaExport.dpi}
            exporting={areaExport.exporting}
            progress={areaExport.progress}
            error={areaExport.error}
            plan={areaExport.plan}
            planError={areaExport.planError}
            onDragStart={areaExportPanel.startDrag}
            onClose={areaExport.cancel}
            onRedraw={areaExport.redraw}
            onSetScale={areaExport.setScale}
            onSetDpi={areaExport.setDpi}
            onExport={() => void areaExport.runExport()}
          />
          <div
            className={
              areaExportPanel.isResizing
                ? "placemark-editor-resize-handle resizing"
                : "placemark-editor-resize-handle"
            }
            onMouseDown={areaExportPanel.startResize}
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
          autoSave={settings.autoSave}
          onChangeAutoSave={settings.setAutoSave}
          onClose={() => setSettingsOpen(false)}
        />
      )}
      {aboutOpen && <AboutModal onClose={() => setAboutOpen(false)} />}
      {loadConfirmOpen && (
        <ConfirmModal
          title={t("confirm.unsavedFileChangesTitle")}
          message={t("confirm.unsavedFileChangesMessage")}
          confirmLabel={t("confirm.discard")}
          cancelLabel={t("common.cancel")}
          extraLabel={t("confirm.save")}
          onExtra={() => {
            setLoadConfirmOpen(false);
            void fileSync.saveNow().then(() => performLoad());
          }}
          onConfirm={() => {
            setLoadConfirmOpen(false);
            void performLoad();
          }}
          onCancel={() => setLoadConfirmOpen(false)}
        />
      )}
      {exitConfirm.confirmOpen && (
        <ConfirmModal
          title={t("confirm.unsavedFileChangesTitle")}
          message={t("confirm.unsavedFileChangesMessage")}
          confirmLabel={t("confirm.discard")}
          cancelLabel={t("common.cancel")}
          extraLabel={t("confirm.save")}
          onExtra={() => void exitConfirm.onSave()}
          onConfirm={exitConfirm.onDiscard}
          onCancel={exitConfirm.onCancel}
        />
      )}
      <CesiumViewerHost baseLayer={baseLayer} sceneMode={sceneMode} onReady={onReadyRef.current} />
      <ScreenOverlayLayer overlays={library.screenOverlays} />
    </div>
  );
}
