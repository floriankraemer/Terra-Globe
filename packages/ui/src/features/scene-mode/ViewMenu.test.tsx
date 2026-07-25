import * as Cesium from "cesium";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ViewMenu } from "./ViewMenu.js";

describe("ViewMenu", () => {
  it("marks the current scene mode as checked", async () => {
    const user = userEvent.setup();
    render(<ViewMenu mode={Cesium.SceneMode.SCENE2D} onChange={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: "View" }));

    expect(screen.getByRole("menuitemradio", { name: "2D" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
    expect(screen.getByRole("menuitemradio", { name: "3D" })).toHaveAttribute(
      "aria-checked",
      "false",
    );
  });

  it("calls onChange and closes the menu when a mode is selected", async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(<ViewMenu mode={Cesium.SceneMode.SCENE3D} onChange={onChange} />);

    await user.click(screen.getByRole("button", { name: "View" }));
    await user.click(screen.getByRole("menuitemradio", { name: "Columbus View" }));

    expect(onChange).toHaveBeenCalledWith(Cesium.SceneMode.COLUMBUS_VIEW);
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });
});
