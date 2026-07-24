import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ImportExportToolbar } from "./ImportExportToolbar.js";

describe("ImportExportToolbar", () => {
  it("calls onExportKml and onExportKmz when their buttons are clicked", async () => {
    const onExportKml = vi.fn();
    const onExportKmz = vi.fn();
    const user = userEvent.setup();
    render(
      <ImportExportToolbar
        onImportFile={vi.fn()}
        onExportKml={onExportKml}
        onExportKmz={onExportKmz}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Export KML" }));
    await user.click(screen.getByRole("button", { name: "Export KMZ" }));

    expect(onExportKml).toHaveBeenCalled();
    expect(onExportKmz).toHaveBeenCalled();
  });

  it("calls onImportFile with the selected file", async () => {
    const onImportFile = vi.fn();
    const user = userEvent.setup();
    const { container } = render(
      <ImportExportToolbar
        onImportFile={onImportFile}
        onExportKml={vi.fn()}
        onExportKmz={vi.fn()}
      />,
    );

    const file = new File(["<kml></kml>"], "places.kml", {
      type: "application/vnd.google-earth.kml+xml",
    });
    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    await user.upload(input, file);

    expect(onImportFile).toHaveBeenCalledWith(file);
  });
});
