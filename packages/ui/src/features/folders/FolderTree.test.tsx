import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Folder, Placemark } from "@terra-globe/core";
import { FolderTree } from "./FolderTree.js";

function folder(overrides: Partial<Folder>): Folder {
  return {
    id: "f1",
    parentId: null,
    name: "Folder",
    visibility: true,
    order: 0,
    createdAt: "now",
    updatedAt: "now",
    ...overrides,
  };
}

function placemark(overrides: Partial<Placemark>): Placemark {
  return {
    id: "p1",
    folderId: null,
    name: "Placemark",
    geometry: { type: "Point", coordinates: { lon: 0, lat: 0 } },
    styleId: null,
    visibility: true,
    createdAt: "now",
    updatedAt: "now",
    ...overrides,
  };
}

const noop = () => {};

describe("FolderTree", () => {
  it("renders nested folders and their placemarks", () => {
    const folders = [
      folder({ id: "root-child", name: "Trips", parentId: null }),
      folder({ id: "nested", name: "2026", parentId: "root-child" }),
    ];
    const placemarks = [placemark({ id: "p1", name: "Berlin", folderId: "nested" })];

    render(
      <FolderTree
        folders={folders}
        placemarks={placemarks}
        selectedFolderId={null}
        selectedPlacemarkId={null}
        onSelectFolder={noop}
        onSelectPlacemark={noop}
        onFlyToPlacemark={noop}
        onCreateFolder={noop}
        onRenameFolder={noop}
        onDeleteFolder={noop}
        onToggleFolderVisibility={noop}
        onMoveFolder={noop}
        onDeletePlacemark={noop}
        onTogglePlacemarkVisibility={noop}
        onMovePlacemark={noop}
      />,
    );

    expect(screen.getByText("Trips")).toBeInTheDocument();
    expect(screen.getByText("2026")).toBeInTheDocument();
    expect(screen.getByText("Berlin")).toBeInTheDocument();
  });

  it("calls onSelectFolder when a folder name is clicked", async () => {
    const onSelectFolder = vi.fn();
    const user = userEvent.setup();
    render(
      <FolderTree
        folders={[folder({ id: "f1", name: "Trips" })]}
        placemarks={[]}
        selectedFolderId={null}
        selectedPlacemarkId={null}
        onSelectFolder={onSelectFolder}
        onSelectPlacemark={noop}
        onFlyToPlacemark={noop}
        onCreateFolder={noop}
        onRenameFolder={noop}
        onDeleteFolder={noop}
        onToggleFolderVisibility={noop}
        onMoveFolder={noop}
        onDeletePlacemark={noop}
        onTogglePlacemarkVisibility={noop}
        onMovePlacemark={noop}
      />,
    );

    await user.click(screen.getByText("Trips"));

    expect(onSelectFolder).toHaveBeenCalledWith("f1");
  });

  it("creates a new root folder via the New Folder form", async () => {
    const onCreateFolder = vi.fn();
    const user = userEvent.setup();
    render(
      <FolderTree
        folders={[]}
        placemarks={[]}
        selectedFolderId={null}
        selectedPlacemarkId={null}
        onSelectFolder={noop}
        onSelectPlacemark={noop}
        onFlyToPlacemark={noop}
        onCreateFolder={onCreateFolder}
        onRenameFolder={noop}
        onDeleteFolder={noop}
        onToggleFolderVisibility={noop}
        onMoveFolder={noop}
        onDeletePlacemark={noop}
        onTogglePlacemarkVisibility={noop}
        onMovePlacemark={noop}
      />,
    );

    await user.click(screen.getByRole("button", { name: "New Folder" }));
    await user.type(screen.getByPlaceholderText("Folder name"), "My Places");
    await user.click(screen.getByRole("button", { name: "Create" }));

    expect(onCreateFolder).toHaveBeenCalledWith(null, "My Places");
  });

  it("deletes a folder via its Delete button", async () => {
    const onDeleteFolder = vi.fn();
    const user = userEvent.setup();
    render(
      <FolderTree
        folders={[folder({ id: "f1", name: "Trips" })]}
        placemarks={[]}
        selectedFolderId={null}
        selectedPlacemarkId={null}
        onSelectFolder={noop}
        onSelectPlacemark={noop}
        onFlyToPlacemark={noop}
        onCreateFolder={noop}
        onRenameFolder={noop}
        onDeleteFolder={onDeleteFolder}
        onToggleFolderVisibility={noop}
        onMoveFolder={noop}
        onDeletePlacemark={noop}
        onTogglePlacemarkVisibility={noop}
        onMovePlacemark={noop}
      />,
    );

    const folderRow = screen.getByText("Trips").closest("li")!;
    await user.click(within(folderRow).getByRole("button", { name: "Delete" }));

    expect(onDeleteFolder).toHaveBeenCalledWith("f1");
  });

  it("deletes a placemark via its Delete button", async () => {
    const onDeletePlacemark = vi.fn();
    const user = userEvent.setup();
    render(
      <FolderTree
        folders={[]}
        placemarks={[placemark({ id: "p1", name: "Berlin", folderId: null })]}
        selectedFolderId={null}
        selectedPlacemarkId={null}
        onSelectFolder={noop}
        onSelectPlacemark={noop}
        onFlyToPlacemark={noop}
        onCreateFolder={noop}
        onRenameFolder={noop}
        onDeleteFolder={noop}
        onToggleFolderVisibility={noop}
        onMoveFolder={noop}
        onDeletePlacemark={onDeletePlacemark}
        onTogglePlacemarkVisibility={noop}
        onMovePlacemark={noop}
      />,
    );

    const placemarkRow = screen.getByText("Berlin").closest("li")!;
    await user.click(within(placemarkRow).getByRole("button", { name: "Delete" }));

    expect(onDeletePlacemark).toHaveBeenCalledWith("p1");
  });

  it("calls onSelectPlacemark when a placemark name is clicked", async () => {
    const onSelectPlacemark = vi.fn();
    const user = userEvent.setup();
    render(
      <FolderTree
        folders={[]}
        placemarks={[placemark({ id: "p1", name: "Berlin", folderId: null })]}
        selectedFolderId={null}
        selectedPlacemarkId={null}
        onSelectFolder={noop}
        onSelectPlacemark={onSelectPlacemark}
        onFlyToPlacemark={noop}
        onCreateFolder={noop}
        onRenameFolder={noop}
        onDeleteFolder={noop}
        onToggleFolderVisibility={noop}
        onMoveFolder={noop}
        onDeletePlacemark={noop}
        onTogglePlacemarkVisibility={noop}
        onMovePlacemark={noop}
      />,
    );

    await user.click(screen.getByText("Berlin"));

    expect(onSelectPlacemark).toHaveBeenCalledWith("p1");
  });

  it("calls onFlyToPlacemark when a placemark name is double-clicked", async () => {
    const onFlyToPlacemark = vi.fn();
    const user = userEvent.setup();
    render(
      <FolderTree
        folders={[]}
        placemarks={[placemark({ id: "p1", name: "Berlin", folderId: null })]}
        selectedFolderId={null}
        selectedPlacemarkId={null}
        onSelectFolder={noop}
        onSelectPlacemark={noop}
        onFlyToPlacemark={onFlyToPlacemark}
        onCreateFolder={noop}
        onRenameFolder={noop}
        onDeleteFolder={noop}
        onToggleFolderVisibility={noop}
        onMoveFolder={noop}
        onDeletePlacemark={noop}
        onTogglePlacemarkVisibility={noop}
        onMovePlacemark={noop}
      />,
    );

    await user.dblClick(screen.getByText("Berlin"));

    expect(onFlyToPlacemark).toHaveBeenCalledWith("p1");
  });

  it("toggles folder visibility via its checkbox", async () => {
    const onToggleFolderVisibility = vi.fn();
    const user = userEvent.setup();
    render(
      <FolderTree
        folders={[folder({ id: "f1", name: "Trips", visibility: true })]}
        placemarks={[]}
        selectedFolderId={null}
        selectedPlacemarkId={null}
        onSelectFolder={noop}
        onSelectPlacemark={noop}
        onFlyToPlacemark={noop}
        onCreateFolder={noop}
        onRenameFolder={noop}
        onDeleteFolder={noop}
        onToggleFolderVisibility={onToggleFolderVisibility}
        onMoveFolder={noop}
        onDeletePlacemark={noop}
        onTogglePlacemarkVisibility={noop}
        onMovePlacemark={noop}
      />,
    );

    const folderRow = screen.getByText("Trips").closest("li")!;
    await user.click(within(folderRow).getByRole("checkbox"));

    expect(onToggleFolderVisibility).toHaveBeenCalledWith("f1");
  });

  it("renames a folder via inline edit", async () => {
    const onRenameFolder = vi.fn();
    const user = userEvent.setup();
    render(
      <FolderTree
        folders={[folder({ id: "f1", name: "Trips" })]}
        placemarks={[]}
        selectedFolderId={null}
        selectedPlacemarkId={null}
        onSelectFolder={noop}
        onSelectPlacemark={noop}
        onFlyToPlacemark={noop}
        onCreateFolder={noop}
        onRenameFolder={onRenameFolder}
        onDeleteFolder={noop}
        onToggleFolderVisibility={noop}
        onMoveFolder={noop}
        onDeletePlacemark={noop}
        onTogglePlacemarkVisibility={noop}
        onMovePlacemark={noop}
      />,
    );

    const folderRow = screen.getByText("Trips").closest("li")!;
    await user.click(within(folderRow).getByRole("button", { name: "Rename" }));
    const input = within(folderRow).getByDisplayValue("Trips");
    await user.clear(input);
    await user.type(input, "Travel");
    await user.keyboard("{Enter}");

    expect(onRenameFolder).toHaveBeenCalledWith("f1", "Travel");
  });

  it("collapses and expands a folder's children", async () => {
    const user = userEvent.setup();
    render(
      <FolderTree
        folders={[folder({ id: "f1", name: "Trips" })]}
        placemarks={[placemark({ id: "p1", name: "Berlin", folderId: "f1" })]}
        selectedFolderId={null}
        selectedPlacemarkId={null}
        onSelectFolder={noop}
        onSelectPlacemark={noop}
        onFlyToPlacemark={noop}
        onCreateFolder={noop}
        onRenameFolder={noop}
        onDeleteFolder={noop}
        onToggleFolderVisibility={noop}
        onMoveFolder={noop}
        onDeletePlacemark={noop}
        onTogglePlacemarkVisibility={noop}
        onMovePlacemark={noop}
      />,
    );

    expect(screen.getByText("Berlin")).toBeVisible();

    await user.click(screen.getByRole("button", { name: "Collapse Trips" }));
    expect(screen.queryByText("Berlin")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Expand Trips" }));
    expect(screen.getByText("Berlin")).toBeVisible();
  });

  describe("search filtering", () => {
    it("hides folders and placemarks that don't match the query", () => {
      const folders = [
        folder({ id: "f1", name: "Berlin Trip", parentId: null }),
        folder({ id: "f2", name: "Paris Trip", parentId: null }),
      ];
      render(
        <FolderTree
          disabled={false}
          folders={folders}
          placemarks={[]}
          selectedFolderId={null}
          selectedPlacemarkId={null}
          searchQuery="berlin"
          onSelectFolder={noop}
          onSelectPlacemark={noop}
          onFlyToPlacemark={noop}
          onCreateFolder={noop}
          onRenameFolder={noop}
          onDeleteFolder={noop}
          onToggleFolderVisibility={noop}
          onMoveFolder={noop}
          onDeletePlacemark={noop}
          onTogglePlacemarkVisibility={noop}
          onMovePlacemark={noop}
        />,
      );

      expect(screen.getByText("Berlin Trip")).toBeInTheDocument();
      expect(screen.queryByText("Paris Trip")).not.toBeInTheDocument();
    });

    it("shows a matched folder's children but starts it collapsed", () => {
      const folders = [folder({ id: "f1", name: "Berlin Trip", parentId: null })];
      const placemarks = [placemark({ id: "p1", name: "Brandenburg Gate", folderId: "f1" })];
      render(
        <FolderTree
          disabled={false}
          folders={folders}
          placemarks={placemarks}
          selectedFolderId={null}
          selectedPlacemarkId={null}
          searchQuery="berlin"
          onSelectFolder={noop}
          onSelectPlacemark={noop}
          onFlyToPlacemark={noop}
          onCreateFolder={noop}
          onRenameFolder={noop}
          onDeleteFolder={noop}
          onToggleFolderVisibility={noop}
          onMoveFolder={noop}
          onDeletePlacemark={noop}
          onTogglePlacemarkVisibility={noop}
          onMovePlacemark={noop}
        />,
      );

      expect(screen.getByText("Berlin Trip")).toBeInTheDocument();
      expect(screen.queryByText("Brandenburg Gate")).not.toBeInTheDocument();

      screen.getByRole("button", { name: "Expand Berlin Trip" });
    });

    it("keeps an ancestor folder expanded when a descendant placemark matches", () => {
      const folders = [folder({ id: "f1", name: "Trips", parentId: null })];
      const placemarks = [
        placemark({ id: "p1", name: "Berlin", folderId: "f1" }),
        placemark({ id: "p2", name: "Tokyo", folderId: "f1" }),
      ];
      render(
        <FolderTree
          disabled={false}
          folders={folders}
          placemarks={placemarks}
          selectedFolderId={null}
          selectedPlacemarkId={null}
          searchQuery="berlin"
          onSelectFolder={noop}
          onSelectPlacemark={noop}
          onFlyToPlacemark={noop}
          onCreateFolder={noop}
          onRenameFolder={noop}
          onDeleteFolder={noop}
          onToggleFolderVisibility={noop}
          onMoveFolder={noop}
          onDeletePlacemark={noop}
          onTogglePlacemarkVisibility={noop}
          onMovePlacemark={noop}
        />,
      );

      expect(screen.getByText("Trips")).toBeInTheDocument();
      expect(screen.getByText("Berlin")).toBeVisible();
      expect(screen.queryByText("Tokyo")).not.toBeInTheDocument();
    });
  });

  // The row-level "before/after/inside" zone logic depends on real
  // getBoundingClientRect() + pointer coordinates, both of which jsdom's
  // DragEvent support fakes too poorly to simulate reliably - that logic is
  // unit-tested directly (no DOM at all) in treeDragDrop.test.ts. These tests
  // only cover the plain container-drop path (append at the end), which
  // needs nothing but a real dragstart/drop pair to exercise the wiring.
  describe("drag and drop", () => {
    it("dropping a placemark onto the tree's empty space appends it at the root", () => {
      const onMovePlacemark = vi.fn();
      const placemarks = [placemark({ id: "p1", name: "Berlin", folderId: null, order: 0 })];
      render(
        <FolderTree
          disabled={false}
          folders={[]}
          placemarks={placemarks}
          selectedFolderId={null}
          selectedPlacemarkId={null}
          onSelectFolder={noop}
          onSelectPlacemark={noop}
          onFlyToPlacemark={noop}
          onCreateFolder={noop}
          onRenameFolder={noop}
          onDeleteFolder={noop}
          onToggleFolderVisibility={noop}
          onMoveFolder={noop}
          onDeletePlacemark={noop}
          onTogglePlacemarkVisibility={noop}
          onMovePlacemark={onMovePlacemark}
        />,
      );

      const placemarkRow = screen.getByText("Berlin").closest(".tree-row")!;
      const rootList = document.querySelector(".tree > ul")!;

      fireEvent.dragStart(placemarkRow);
      fireEvent.drop(rootList);
      fireEvent.dragEnd(placemarkRow);

      expect(onMovePlacemark).toHaveBeenCalledWith("p1", null, 0);
    });

    it("does not allow draggable rows while disabled", () => {
      render(
        <FolderTree
          disabled={true}
          folders={[folder({ id: "f1", name: "Trips" })]}
          placemarks={[]}
          selectedFolderId={null}
          selectedPlacemarkId={null}
          onSelectFolder={noop}
          onSelectPlacemark={noop}
          onFlyToPlacemark={noop}
          onCreateFolder={noop}
          onRenameFolder={noop}
          onDeleteFolder={noop}
          onToggleFolderVisibility={noop}
          onMoveFolder={noop}
          onDeletePlacemark={noop}
          onTogglePlacemarkVisibility={noop}
          onMovePlacemark={noop}
        />,
      );

      const folderRow = screen.getByText("Trips").closest(".tree-row")!;
      expect(folderRow).toHaveAttribute("draggable", "false");
    });
  });
});
