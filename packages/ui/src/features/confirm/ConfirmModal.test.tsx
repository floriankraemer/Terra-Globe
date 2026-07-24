import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ConfirmModal } from "./ConfirmModal.js";

describe("ConfirmModal", () => {
  it("renders the title and message", () => {
    render(
      <ConfirmModal
        title="Delete placemark?"
        message="This cannot be undone."
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    expect(screen.getByRole("alertdialog", { name: "Delete placemark?" })).toBeInTheDocument();
    expect(screen.getByText("This cannot be undone.")).toBeInTheDocument();
  });

  it("calls onConfirm when the confirm button is clicked", async () => {
    const onConfirm = vi.fn();
    const user = userEvent.setup();
    render(
      <ConfirmModal
        title="Delete placemark?"
        message="This cannot be undone."
        onConfirm={onConfirm}
        onCancel={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Delete" }));

    expect(onConfirm).toHaveBeenCalled();
  });

  it("calls onCancel when the cancel button is clicked", async () => {
    const onCancel = vi.fn();
    const user = userEvent.setup();
    render(
      <ConfirmModal
        title="Delete placemark?"
        message="This cannot be undone."
        onConfirm={vi.fn()}
        onCancel={onCancel}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(onCancel).toHaveBeenCalled();
  });

  it("calls onCancel when the overlay is clicked, but not when the panel itself is clicked", async () => {
    const onCancel = vi.fn();
    const user = userEvent.setup();
    render(
      <ConfirmModal
        title="Delete placemark?"
        message="This cannot be undone."
        onConfirm={vi.fn()}
        onCancel={onCancel}
      />,
    );

    await user.click(screen.getByText("This cannot be undone."));
    expect(onCancel).not.toHaveBeenCalled();

    await user.click(screen.getByRole("alertdialog").parentElement!);
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("supports custom confirm/cancel labels", () => {
    render(
      <ConfirmModal
        title="Delete folder?"
        message="Its contents will also be deleted."
        confirmLabel="Delete Folder"
        cancelLabel="Keep it"
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: "Delete Folder" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Keep it" })).toBeInTheDocument();
  });

  it("renders and wires up an optional third action", async () => {
    const onExtra = vi.fn();
    const user = userEvent.setup();
    render(
      <ConfirmModal
        title="Unsaved changes"
        message="Save changes before closing?"
        confirmLabel="Discard"
        extraLabel="Save"
        onExtra={onExtra}
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(onExtra).toHaveBeenCalled();
  });

  it("omits the third action when extraLabel/onExtra aren't given", () => {
    render(
      <ConfirmModal
        title="Delete placemark?"
        message="This cannot be undone."
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );

    expect(screen.queryByRole("button", { name: "Save" })).not.toBeInTheDocument();
  });
});
