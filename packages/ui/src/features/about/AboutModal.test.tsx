import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { AboutModal } from "./AboutModal.js";

describe("AboutModal", () => {
  it("shows the product name and version", () => {
    render(<AboutModal onClose={vi.fn()} />);

    expect(screen.getByRole("dialog", { name: "About Terra Globe" })).toBeInTheDocument();
    expect(screen.getByText("Version 0.0.0-test")).toBeInTheDocument();
  });

  it("links the author name and license to their official pages, opening in a new window", () => {
    render(<AboutModal onClose={vi.fn()} />);

    const authorLink = screen.getByRole("link", { name: "Florian Krämer" });
    expect(authorLink).toHaveAttribute("href", "https://florian-kraemer.net/");
    expect(authorLink).toHaveAttribute("target", "_blank");

    const licenseLink = screen.getByRole("link", { name: "GNU General Public License v3.0" });
    expect(licenseLink).toHaveAttribute("href", "https://www.gnu.org/licenses/gpl-3.0.html");
    expect(licenseLink).toHaveAttribute("target", "_blank");
  });

  it("lists and links the third-party libraries the app is built with", () => {
    render(<AboutModal onClose={vi.fn()} />);

    const reactLink = screen.getByRole("link", { name: "react" });
    expect(reactLink).toHaveAttribute("href", "https://react.dev");
    expect(reactLink).toHaveAttribute("target", "_blank");
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
