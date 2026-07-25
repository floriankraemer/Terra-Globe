import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { SecretStore } from "@terra-globe/core";
import { SettingsModal } from "./SettingsModal.js";

function fakeSecretStore(): SecretStore {
  return {
    async get() {
      return undefined;
    },
    async set() {},
    async remove() {},
  };
}

function baseProps() {
  return {
    unitSystem: "metric" as const,
    coordinateFormat: "decimal" as const,
    language: "en" as const,
    onChangeUnitSystem: vi.fn(),
    onChangeCoordinateFormat: vi.fn(),
    onChangeLanguage: vi.fn(),
    providers: [],
    secretStore: fakeSecretStore(),
    onAddProvider: vi.fn(() => "id"),
    onSetProviderEnabled: vi.fn(),
    onRemoveProvider: vi.fn(),
    onClose: vi.fn(),
  };
}

describe("SettingsModal", () => {
  it("shows the Units section by default with the current values", () => {
    render(<SettingsModal {...baseProps()} />);

    expect(screen.getByRole("dialog", { name: "Settings" })).toBeInTheDocument();
    expect(screen.getByLabelText("Unit System")).toHaveValue("metric");
    expect(screen.getByLabelText("Coordinate Format")).toHaveValue("decimal");
  });

  it("calls onChangeUnitSystem when the unit system is changed", async () => {
    const props = baseProps();
    const user = userEvent.setup();
    render(<SettingsModal {...props} />);

    await user.selectOptions(screen.getByLabelText("Unit System"), "imperial");

    expect(props.onChangeUnitSystem).toHaveBeenCalledWith("imperial");
  });

  it("calls onChangeCoordinateFormat when the coordinate format is changed", async () => {
    const props = baseProps();
    const user = userEvent.setup();
    render(<SettingsModal {...props} />);

    await user.selectOptions(screen.getByLabelText("Coordinate Format"), "dms");

    expect(props.onChangeCoordinateFormat).toHaveBeenCalledWith("dms");
  });

  it("calls onClose when Close is clicked or the overlay is clicked", async () => {
    const props = baseProps();
    const user = userEvent.setup();
    render(<SettingsModal {...props} />);

    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(props.onClose).toHaveBeenCalledTimes(1);
  });

  it("does not close when clicking inside the modal panel", async () => {
    const props = baseProps();
    const user = userEvent.setup();
    render(<SettingsModal {...props} />);

    await user.click(screen.getByRole("dialog", { name: "Settings" }));
    expect(props.onClose).not.toHaveBeenCalled();
  });

  it("shows the Providers section when its nav item is clicked", async () => {
    const props = baseProps();
    const user = userEvent.setup();
    render(<SettingsModal {...props} />);

    await user.click(screen.getByRole("button", { name: "Providers" }));

    expect(screen.getByText("No providers configured yet.")).toBeInTheDocument();
  });
});
