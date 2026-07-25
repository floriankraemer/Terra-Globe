import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { MenuButton } from "./MenuButton.js";

describe("MenuButton", () => {
  it("opens the menu when the trigger is clicked and calls close() from an item", async () => {
    const onSelect = vi.fn();
    const user = userEvent.setup();
    render(
      <MenuButton label="File">
        {(close) => (
          <button
            role="menuitem"
            onClick={() => {
              onSelect();
              close();
            }}
          >
            Import
          </button>
        )}
      </MenuButton>,
    );

    expect(screen.queryByRole("menu")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "File" }));
    expect(screen.getByRole("menu")).toBeInTheDocument();

    await user.click(screen.getByRole("menuitem", { name: "Import" }));
    expect(onSelect).toHaveBeenCalled();
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("closes when clicking outside", async () => {
    const user = userEvent.setup();
    render(
      <div>
        <MenuButton label="File">{() => <button role="menuitem">Import</button>}</MenuButton>
        <button>Outside</button>
      </div>,
    );

    await user.click(screen.getByRole("button", { name: "File" }));
    expect(screen.getByRole("menu")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Outside" }));
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("closes on Escape", async () => {
    const user = userEvent.setup();
    render(<MenuButton label="File">{() => <button role="menuitem">Import</button>}</MenuButton>);

    await user.click(screen.getByRole("button", { name: "File" }));
    expect(screen.getByRole("menu")).toBeInTheDocument();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });
});
