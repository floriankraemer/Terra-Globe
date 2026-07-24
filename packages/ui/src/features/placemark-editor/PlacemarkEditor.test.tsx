import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Placemark } from "@webglobe/core";
import { PlacemarkEditor, type PlacemarkStyleEdits } from "./PlacemarkEditor.js";

function placemark(overrides: Partial<Placemark> = {}): Placemark {
  return {
    id: "p1",
    folderId: null,
    name: "Berlin",
    description: "Capital of Germany",
    geometry: { type: "Point", coordinates: { lon: 13.4, lat: 52.5 } },
    styleId: null,
    visibility: true,
    createdAt: "now",
    updatedAt: "now",
    ...overrides,
  };
}

function style(overrides: Partial<PlacemarkStyleEdits> = {}): PlacemarkStyleEdits {
  return {
    outlineEnabled: true,
    outlineColor: "#ff0000",
    outlineWidth: 2,
    filled: false,
    fillColor: "#ff0000",
    ...overrides,
  };
}

describe("PlacemarkEditor - Point (marker)", () => {
  it("pre-fills name, description and a single color field", () => {
    render(
      <PlacemarkEditor
        placemark={placemark()}
        style={style()}
        onSave={vi.fn()}
        onClose={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getByLabelText("Name")).toHaveValue("Berlin");
    expect(screen.getByLabelText("Description")).toHaveValue("Capital of Germany");
    expect(screen.getByLabelText("Color")).toHaveValue("#ff0000");
    expect(screen.queryByLabelText("Outline Color")).not.toBeInTheDocument();
  });

  it("shows the marker's location as Lat, Lon", () => {
    render(
      <PlacemarkEditor
        placemark={placemark()}
        style={style()}
        onSave={vi.fn()}
        onClose={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getByText("Location")).toBeInTheDocument();
    expect(screen.getByText("52.50000, 13.40000")).toBeInTheDocument();
  });

  it("calls onSave with a filled/outlined style built from the single color", async () => {
    const onSave = vi.fn();
    const user = userEvent.setup();
    render(
      <PlacemarkEditor placemark={placemark()} style={style()} onSave={onSave} onClose={vi.fn()} />,
    );

    await user.clear(screen.getByLabelText("Name"));
    await user.type(screen.getByLabelText("Name"), "Munich");
    fireEvent.change(screen.getByLabelText("Color"), { target: { value: "#00ff00" } });
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(onSave).toHaveBeenCalledWith({
      name: "Munich",
      description: "Capital of Germany",
      style: {
        outlineEnabled: true,
        outlineColor: "#00ff00",
        outlineWidth: 2,
        filled: true,
        fillColor: "#00ff00",
      },
    });
  });

  it("calls onClose when Close is clicked", async () => {
    const onClose = vi.fn();
    const user = userEvent.setup();
    render(
      <PlacemarkEditor
        placemark={placemark()}
        style={style()}
        onSave={vi.fn()}
        onClose={onClose}
        onDelete={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Close" }));

    expect(onClose).toHaveBeenCalled();
  });

  it("calls onDragStart when the header is pressed", () => {
    const onDragStart = vi.fn();
    render(
      <PlacemarkEditor
        placemark={placemark()}
        style={style()}
        onSave={vi.fn()}
        onClose={vi.fn()}
        onDelete={vi.fn()}
        onDragStart={onDragStart}
      />,
    );

    fireEvent.mouseDown(screen.getByText("Edit Placemark"));

    expect(onDragStart).toHaveBeenCalled();
  });

  it("shows the placemark id", () => {
    render(
      <PlacemarkEditor
        placemark={placemark({ id: "p-42" })}
        style={style()}
        onSave={vi.fn()}
        onClose={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getByText("ID: p-42")).toBeInTheDocument();
  });

  it("asks for confirmation before deleting, and only calls onDelete once confirmed", async () => {
    const onDelete = vi.fn();
    const user = userEvent.setup();
    render(
      <PlacemarkEditor
        placemark={placemark()}
        style={style()}
        onSave={vi.fn()}
        onClose={vi.fn()}
        onDelete={onDelete}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Delete" }));
    expect(onDelete).not.toHaveBeenCalled();

    const dialog = screen.getByRole("alertdialog", { name: "Delete placemark?" });
    expect(dialog).toHaveTextContent("Berlin");

    await user.click(within(dialog).getByRole("button", { name: "Delete" }));

    expect(onDelete).toHaveBeenCalledTimes(1);
  });

  it("dismisses the confirmation without deleting when Cancel is clicked", async () => {
    const onDelete = vi.fn();
    const user = userEvent.setup();
    render(
      <PlacemarkEditor
        placemark={placemark()}
        style={style()}
        onSave={vi.fn()}
        onClose={vi.fn()}
        onDelete={onDelete}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Delete" }));
    const dialog = screen.getByRole("alertdialog", { name: "Delete placemark?" });

    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));

    expect(onDelete).not.toHaveBeenCalled();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("handles a placemark with no description", () => {
    render(
      <PlacemarkEditor
        placemark={placemark({ description: undefined })}
        style={style()}
        onSave={vi.fn()}
        onClose={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getByLabelText("Description")).toHaveValue("");
  });

  it("re-syncs when the style prop arrives after mount (async style lookup)", () => {
    const { rerender } = render(
      <PlacemarkEditor
        placemark={placemark()}
        style={style()}
        onSave={vi.fn()}
        onClose={vi.fn()}
        onDelete={vi.fn()}
      />,
    );
    expect(screen.getByLabelText("Color")).toHaveValue("#ff0000");

    rerender(
      <PlacemarkEditor
        placemark={placemark()}
        style={style({ outlineColor: "#00ff00", fillColor: "#00ff00" })}
        onSave={vi.fn()}
        onClose={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getByLabelText("Color")).toHaveValue("#00ff00");
  });
});

describe("PlacemarkEditor - LineString", () => {
  const linePlacemark = placemark({
    geometry: {
      type: "LineString",
      path: [
        { lon: 0, lat: 0 },
        { lon: 2, lat: 0 },
      ],
    },
  });

  it("shows outline controls but no fill controls", () => {
    render(
      <PlacemarkEditor
        placemark={linePlacemark}
        style={style()}
        onSave={vi.fn()}
        onClose={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getByLabelText("Outline")).toBeChecked();
    expect(screen.getByLabelText("Outline Color")).toHaveValue("#ff0000");
    expect(screen.getByLabelText("Outline Width")).toHaveValue(2);
    expect(screen.queryByLabelText("Filled")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Fill Color")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Color")).not.toBeInTheDocument();
  });

  it("shows the path midpoint as Center", () => {
    render(
      <PlacemarkEditor
        placemark={linePlacemark}
        style={style()}
        onSave={vi.fn()}
        onClose={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getByText("Center")).toBeInTheDocument();
    expect(screen.getByText("0.00000, 1.00000")).toBeInTheDocument();
  });
});

describe("PlacemarkEditor - area shapes (circle/rectangle/polygon)", () => {
  const rectanglePlacemark = placemark({
    geometry: { type: "Rectangle", north: 10, south: 0, east: 10, west: 0 },
  });

  it("shows outline and fill controls instead of a single color field", () => {
    render(
      <PlacemarkEditor
        placemark={rectanglePlacemark}
        style={style()}
        onSave={vi.fn()}
        onClose={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getByLabelText("Outline")).toBeChecked();
    expect(screen.getByLabelText("Outline Color")).toHaveValue("#ff0000");
    expect(screen.getByLabelText("Outline Width")).toHaveValue(2);
    expect(screen.getByLabelText("Filled")).not.toBeChecked();
    expect(screen.getByLabelText("Fill Color")).toBeDisabled();
    expect(screen.queryByLabelText("Color")).not.toBeInTheDocument();
    expect(screen.getByText("Center")).toBeInTheDocument();
    expect(screen.getByText("5.00000, 5.00000")).toBeInTheDocument();
  });

  it("defaults to outline enabled and filled disabled", () => {
    render(
      <PlacemarkEditor
        placemark={rectanglePlacemark}
        style={style({ outlineEnabled: true, filled: false })}
        onSave={vi.fn()}
        onClose={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getByLabelText("Outline")).toBeChecked();
    expect(screen.getByLabelText("Filled")).not.toBeChecked();
  });

  it("enables the fill color field once Filled is checked, and saves the toggled style", async () => {
    const onSave = vi.fn();
    const user = userEvent.setup();
    render(
      <PlacemarkEditor
        placemark={rectanglePlacemark}
        style={style()}
        onSave={onSave}
        onClose={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    await user.click(screen.getByLabelText("Filled"));
    expect(screen.getByLabelText("Fill Color")).toBeEnabled();

    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        style: expect.objectContaining({ filled: true }),
      }),
    );
  });

  it("unchecking Outline disables the outline color/width fields and saves outlineEnabled: false", async () => {
    const onSave = vi.fn();
    const user = userEvent.setup();
    render(
      <PlacemarkEditor
        placemark={rectanglePlacemark}
        style={style()}
        onSave={onSave}
        onClose={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    await user.click(screen.getByLabelText("Outline"));
    expect(screen.getByLabelText("Outline Color")).toBeDisabled();
    expect(screen.getByLabelText("Outline Width")).toBeDisabled();

    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        style: expect.objectContaining({ outlineEnabled: false }),
      }),
    );
  });

  it("changing outline width updates the saved style", async () => {
    const onSave = vi.fn();
    const user = userEvent.setup();
    render(
      <PlacemarkEditor
        placemark={rectanglePlacemark}
        style={style()}
        onSave={onSave}
        onClose={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    await user.clear(screen.getByLabelText("Outline Width"));
    await user.type(screen.getByLabelText("Outline Width"), "5");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(onSave).toHaveBeenCalledWith(
      expect.objectContaining({
        style: expect.objectContaining({ outlineWidth: 5 }),
      }),
    );
  });
});

describe("PlacemarkEditor - measurements", () => {
  it("shows radius, circumference and area in meters for a circle", () => {
    render(
      <PlacemarkEditor
        placemark={placemark({
          geometry: { type: "Circle", center: { lon: 0, lat: 0 }, radiusMeters: 1000 },
        })}
        style={style()}
        onSave={vi.fn()}
        onClose={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getByText("Radius")).toBeInTheDocument();
    expect(screen.getByText("1.00 km")).toBeInTheDocument();
    expect(screen.getByText("Circumference")).toBeInTheDocument();
    expect(screen.getByText("6.28 km")).toBeInTheDocument();
    expect(screen.getByText("Area")).toBeInTheDocument();
    expect(screen.getByText("3.14 km²")).toBeInTheDocument();
  });

  it("shows area in square meters for a rectangle", () => {
    render(
      <PlacemarkEditor
        placemark={placemark({
          geometry: { type: "Rectangle", north: 0.01, south: 0, east: 0.01, west: 0 },
        })}
        style={style()}
        onSave={vi.fn()}
        onClose={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getByText("Area")).toBeInTheDocument();
    expect(screen.queryByText("Radius")).not.toBeInTheDocument();
  });

  it("shows area in square meters for a polygon", () => {
    render(
      <PlacemarkEditor
        placemark={placemark({
          geometry: {
            type: "Polygon",
            outerRing: [
              { lon: 0, lat: 0 },
              { lon: 0.01, lat: 0 },
              { lon: 0.01, lat: 0.01 },
              { lon: 0, lat: 0.01 },
            ],
          },
        })}
        style={style()}
        onSave={vi.fn()}
        onClose={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.getByText("Area")).toBeInTheDocument();
  });

  it("shows no measurements block for a point or line", () => {
    render(
      <PlacemarkEditor
        placemark={placemark()}
        style={style()}
        onSave={vi.fn()}
        onClose={vi.fn()}
        onDelete={vi.fn()}
      />,
    );

    expect(screen.queryByText("Measurements")).not.toBeInTheDocument();
  });
});
