import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { ProviderConfig, SecretStore } from "@webglobe/core";
import * as core from "@webglobe/core";
import { ProvidersTab } from "./ProvidersTab.js";

vi.mock("@webglobe/core", async () => {
  const actual = await vi.importActual<typeof core>("@webglobe/core");
  return {
    ...actual,
    testTileProviderConfig: vi.fn(),
    testGeocodingProviderConfig: vi.fn(),
    testRoutingProviderConfig: vi.fn(),
  };
});

function fakeSecretStore(): SecretStore {
  return {
    async get() {
      return "sk-123";
    },
    async set() {},
    async remove() {},
  };
}

describe("ProvidersTab", () => {
  it("shows an empty state with no providers configured", () => {
    render(
      <ProvidersTab
        providers={[]}
        secretStore={fakeSecretStore()}
        onAdd={vi.fn(() => "id")}
        onSetEnabled={vi.fn()}
        onRemove={vi.fn()}
      />,
    );
    expect(screen.getByText("No providers configured yet.")).toBeInTheDocument();
  });

  it("submits the add-provider form with the selected kind/preset/name", async () => {
    const onAdd = vi.fn(() => "new-id");
    const user = userEvent.setup();
    render(
      <ProvidersTab
        providers={[]}
        secretStore={fakeSecretStore()}
        onAdd={onAdd}
        onSetEnabled={vi.fn()}
        onRemove={vi.fn()}
      />,
    );

    await user.type(screen.getByLabelText("Name"), "My Mapbox");
    await user.type(screen.getByLabelText("API Key"), "sk-abc");
    await user.click(screen.getByRole("button", { name: "Add Provider" }));

    expect(onAdd).toHaveBeenCalledWith({
      kind: "tile",
      preset: "mapbox-streets",
      name: "My Mapbox",
    });
  });

  it("submits the add-provider form for a routing provider", async () => {
    const onAdd = vi.fn(() => "new-id");
    const user = userEvent.setup();
    render(
      <ProvidersTab
        providers={[]}
        secretStore={fakeSecretStore()}
        onAdd={onAdd}
        onSetEnabled={vi.fn()}
        onRemove={vi.fn()}
      />,
    );

    await user.selectOptions(screen.getByLabelText("Type"), "routing");
    await user.type(screen.getByLabelText("Name"), "My GraphHopper");
    await user.click(screen.getByRole("button", { name: "Add Provider" }));

    expect(onAdd).toHaveBeenCalledWith({
      kind: "routing",
      preset: "osrm",
      name: "My GraphHopper",
    });
  });

  it("shows a success badge and enables the provider after a successful test", async () => {
    vi.mocked(core.testTileProviderConfig).mockResolvedValue({ ok: true });
    const onSetEnabled = vi.fn();
    const user = userEvent.setup();
    const config: ProviderConfig = {
      id: "p1",
      kind: "tile",
      preset: "mapbox-streets",
      name: "My Mapbox",
      enabled: false,
    };
    render(
      <ProvidersTab
        providers={[config]}
        secretStore={fakeSecretStore()}
        onAdd={vi.fn(() => "id")}
        onSetEnabled={onSetEnabled}
        onRemove={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Test" }));

    expect(await screen.findByText("success")).toBeInTheDocument();
    expect(onSetEnabled).toHaveBeenCalledWith("p1", true);
  });

  it("shows a failure badge and does not enable the provider on failed test", async () => {
    vi.mocked(core.testTileProviderConfig).mockResolvedValue({ ok: false, error: "HTTP 404" });
    const onSetEnabled = vi.fn();
    const user = userEvent.setup();
    const config: ProviderConfig = {
      id: "p1",
      kind: "tile",
      preset: "mapbox-streets",
      name: "My Mapbox",
      enabled: false,
    };
    render(
      <ProvidersTab
        providers={[config]}
        secretStore={fakeSecretStore()}
        onAdd={vi.fn(() => "id")}
        onSetEnabled={onSetEnabled}
        onRemove={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Test" }));

    expect(await screen.findByText("failure")).toBeInTheDocument();
    expect(onSetEnabled).not.toHaveBeenCalled();
  });

  it("calls onRemove when Remove is clicked", async () => {
    const onRemove = vi.fn();
    const user = userEvent.setup();
    const config: ProviderConfig = {
      id: "p1",
      kind: "tile",
      preset: "mapbox-streets",
      name: "My Mapbox",
      enabled: false,
    };
    render(
      <ProvidersTab
        providers={[config]}
        secretStore={fakeSecretStore()}
        onAdd={vi.fn(() => "id")}
        onSetEnabled={vi.fn()}
        onRemove={onRemove}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Remove" }));

    expect(onRemove).toHaveBeenCalledWith("p1");
  });
});
