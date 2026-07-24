import { useEffect, useRef, useState } from "react";
import type * as Cesium from "cesium";
import {
  createPointGeometry,
  NominatimGeocodingProvider,
  type GeocodeResult,
  type GeocodingProvider,
  type GeocodingProviderConfig,
  type LineStringGeometry,
} from "@webglobe/core";
import {
  flyToGeometry,
  BUILTIN_TILE_SOURCES,
  type TileSource,
  type PlacemarkStyleEdits,
} from "@webglobe/map";
import { CesiumViewerHost } from "./CesiumViewerHost.js";
import { DrawingToolbar } from "../features/drawing-toolbar/DrawingToolbar.js";
import { useDrawing } from "../features/drawing-toolbar/useDrawing.js";
import { AddressSearchBox } from "../features/geocoding/AddressSearchBox.js";
import { useGeocoding } from "../features/geocoding/useGeocoding.js";
import { FolderTree } from "../features/folders/FolderTree.js";
import { useLibrary } from "../features/folders/useLibrary.js";
import { HeightProfilePanel } from "../features/height-profile/HeightProfilePanel.js";
import { ImportExportToolbar } from "../features/import-export/ImportExportToolbar.js";
import { useImportExport } from "../features/import-export/useImportExport.js";
import { Notice, type NoticeData } from "../features/notice/Notice.js";
import { PlacemarkEditor } from "../features/placemark-editor/PlacemarkEditor.js";
import { SettingsModal } from "../features/settings/SettingsModal.js";
import { useSettings } from "../features/settings/useSettings.js";
import { useFloatingPanel } from "./useFloatingPanel.js";
import { useResizableWidth } from "./useResizableWidth.js";
import { createGeocodingProvider } from "../platform/createGeocodingProvider.js";
import { createSecretStore } from "../platform/createSecretStore.js";
import { buildTileSources } from "../platform/tileProviderRegistry.js";

const DEFAULT_STYLE: PlacemarkStyleEdits = {
  outlineEnabled: true,
  outlineColor: "#ff0000",
  outlineWidth: 2,
  filled: false,
  fillColor: "#ff0000",
};

export function App() {
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
  const sidebar = useResizableWidth(260, 200, 600, "webglobe:sidebarWidth");
  const placemarkPanel = useFloatingPanel({
    initial: { x: window.innerWidth - 260 - 12, y: 56, width: 260, height: 480 },
    minWidth: 240,
    minHeight: 200,
    maxWidth: 600,
    maxHeight: 800,
    storageKey: "webglobe:placemarkEditorGeometry",
  });
  const heightProfilePanel = useFloatingPanel({
    initial: { x: window.innerWidth - 260 - 12, y: 550, width: 420, height: 260 },
    minWidth: 300,
    minHeight: 180,
    maxWidth: 900,
    maxHeight: 600,
    storageKey: "webglobe:heightProfilePanelGeometry",
  });
  const secretStore = useRef(createSecretStore()).current;
  const settings = useSettings(secretStore);
  const [geocodingProvider, setGeocodingProvider] = useState<GeocodingProvider>(
    () => new NominatimGeocodingProvider(),
  );
  const geocoding = useGeocoding(geocodingProvider);

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
  const onReadyRef = useRef((handle: { viewer: Cesium.Viewer }) => {
    setViewer(handle.viewer);
    // Exposed for E2E tests to inspect camera state (e.g. after a fly-to);
    // harmless in production, mirrors the common devtools-debugging pattern.
    (window as unknown as { __webglobeViewer?: Cesium.Viewer }).__webglobeViewer = handle.viewer;
    // Repurpose Cesium's built-in "home" button: cancel its default
    // fly-to-home behavior and open the Settings modal instead.
    handle.viewer.homeButton.viewModel.command.beforeExecute.addEventListener((commandInfo) => {
      commandInfo.cancel = true;
      setSettingsOpen(true);
    });
  });

  const library = useLibrary(viewer);
  const { mode, selectTool, finishPolygon, cancel } = useDrawing(
    viewer,
    (geometry) => {
      void library.addPlacemark(geometry).then((id) => {
        if (id) setSelectedPlacemarkId(id);
      });
    },
    (entityId) => {
      if (library.placemarks.some((p) => p.id === entityId)) setSelectedPlacemarkId(entityId);
    },
  );
  const importExport = useImportExport(library);

  const editingPlacemark = library.placemarks.find((p) => p.id === selectedPlacemarkId) ?? null;
  const selectedTrack =
    editingPlacemark && editingPlacemark.geometry.type === "LineString" ? editingPlacemark : null;
  const [dismissedTrackId, setDismissedTrackId] = useState<string | null>(null);
  const showHeightProfile = selectedTrack !== null && selectedTrack.id !== dismissedTrackId;

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
      <div className="app-topbar">
        <label className="base-layer-select">
          Basemap
          <select value={baseLayerId} onChange={(e) => setBaseLayerId(e.target.value)}>
            {tileSources.map((source) => (
              <option key={source.id} value={source.id}>
                {source.name}
              </option>
            ))}
          </select>
        </label>
        <DrawingToolbar
          mode={mode}
          disabled={!library.ready}
          onSelectTool={selectTool}
          onFinishPolygon={finishPolygon}
          onCancel={cancel}
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
                  `Imported ${summary.placemarksImported} placemark(s)`,
                  summary.foldersImported > 0 ? `${summary.foldersImported} folder(s)` : null,
                ].filter(Boolean);
                const message =
                  summary.warnings.length > 0
                    ? `${parts.join(", ")}. ${summary.warnings.length} item(s) skipped: ${summary.warnings.join(" ")}`
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
      <div className="places-panel" style={{ width: sidebar.width }}>
        <div className="places-panel-header">Places</div>
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
            onCreateFolder={(parentId, name) => void library.createFolder(parentId, name)}
            onRenameFolder={(id, name) => void library.renameFolder(id, name)}
            onDeleteFolder={(id) => void library.deleteFolder(id)}
            onToggleFolderVisibility={(id) => void library.toggleFolderVisibility(id)}
            onMoveFolder={(id, parentId, index) => void library.moveFolder(id, parentId, index)}
            onDeletePlacemark={(id) => {
              void library.deletePlacemark(id);
              if (id === selectedPlacemarkId) setSelectedPlacemarkId(null);
            }}
            onTogglePlacemarkVisibility={(id) => void library.togglePlacemarkVisibility(id)}
            onMovePlacemark={(id, folderId, index) =>
              void library.movePlacemark(id, folderId, index)
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
            onClose={() => setSelectedPlacemarkId(null)}
            onDelete={() => {
              // Wait for the delete (including its internal refresh()) before
              // closing, same reasoning as onSave below - otherwise the tree
              // can still show the placemark for a moment after the editor
              // has already closed.
              void library
                .deletePlacemark(editingPlacemark.id)
                .then(() => setSelectedPlacemarkId(null));
            }}
            onSave={({ name, description, style }) => {
              // Wait for the save (including its internal refresh()) before
              // closing - closing immediately (fire-and-forget) let a user
              // re-open the same placemark mid-save and see stale pre-save
              // data, since `placemarks` state hadn't caught up yet.
              void library
                .savePlacemarkEdits(editingPlacemark.id, { name, description, style })
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
      {showHeightProfile && selectedTrack && (
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
            key={selectedTrack.id}
            trackName={selectedTrack.name}
            geometry={selectedTrack.geometry as LineStringGeometry}
            unitSystem={settings.unitSystem}
            width={heightProfilePanel.geometry.width}
            height={heightProfilePanel.geometry.height}
            onDragStart={heightProfilePanel.startDrag}
            onClose={() => setDismissedTrackId(selectedTrack.id)}
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
      {settingsOpen && (
        <SettingsModal
          unitSystem={settings.unitSystem}
          coordinateFormat={settings.coordinateFormat}
          onChangeUnitSystem={settings.setUnitSystem}
          onChangeCoordinateFormat={settings.setCoordinateFormat}
          providers={settings.providers}
          secretStore={secretStore}
          onAddProvider={settings.addProvider}
          onSetProviderEnabled={settings.setProviderEnabled}
          onRemoveProvider={settings.removeProvider}
          onClose={() => setSettingsOpen(false)}
        />
      )}
      <CesiumViewerHost baseLayer={baseLayer} onReady={onReadyRef.current} />
    </div>
  );
}
