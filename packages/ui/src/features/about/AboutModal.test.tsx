import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { AboutModal } from "./AboutModal.js";

describe("AboutModal", () => {
  it("shows the product name, version, author and license", () => {
    render(<AboutModal onClose={vi.fn()} />);

    expect(screen.getByRole("dialog", { name: "About Terra Globe" })).toBeInTheDocument();
    expect(screen.getByText("Version 0.0.0-test")).toBeInTheDocument();
    expect(screen.getByText("Author: Florian Krämer")).toBeInTheDocument();
    expect(
      screen.getByText("Licensed under the GNU General Public License v3.0"),
    ).toBeInTheDocument();
  });

  it("links to the GitHub repo and its issue tracker", () => {
    render(<AboutModal onClose={vi.fn()} />);

    expect(screen.getByRole("link", { name: "View on GitHub" })).toHaveAttribute(
      "href",
      "https://github.com/floriankraemer/Terra-Globe",
    );
    expect(screen.getByRole("link", { name: "Report an issue" })).toHaveAttribute(
      "href",
      "https://github.com/floriankraemer/Terra-Globe/issues",
    );
  });

  it("calls onClose when the close button or overlay is clicked", async () => {
    const onClose = vi.fn();
    const user = userEvent.setup();
    render(<AboutModal onClose={onClose} />);

    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalledTimes(1);

    await user.click(screen.getByRole("dialog").parentElement!);
    expect(onClose).toHaveBeenCalledTimes(2);
  });
});
