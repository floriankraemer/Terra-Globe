import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Notice } from "./Notice.js";

describe("Notice", () => {
  it("renders the message with a role of alert", () => {
    render(<Notice notice={{ level: "error", message: "Something broke" }} onDismiss={vi.fn()} />);

    expect(screen.getByRole("alert")).toHaveTextContent("Something broke");
  });

  it("calls onDismiss when the dismiss button is clicked", async () => {
    const onDismiss = vi.fn();
    const user = userEvent.setup();
    render(
      <Notice notice={{ level: "success", message: "Imported 3 items" }} onDismiss={onDismiss} />,
    );

    await user.click(screen.getByRole("button", { name: "Dismiss" }));

    expect(onDismiss).toHaveBeenCalled();
  });

  it("applies a level-specific class", () => {
    render(<Notice notice={{ level: "error", message: "Oops" }} onDismiss={vi.fn()} />);

    expect(screen.getByRole("alert")).toHaveClass("notice-error");
  });
});
