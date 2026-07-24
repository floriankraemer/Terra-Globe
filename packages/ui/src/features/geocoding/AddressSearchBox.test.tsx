import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { AddressSearchBox } from "./AddressSearchBox.js";

describe("AddressSearchBox", () => {
  it("calls onSearch with the typed query on submit", async () => {
    const onSearch = vi.fn();
    const user = userEvent.setup();
    render(
      <AddressSearchBox
        disabled={false}
        status="idle"
        results={[]}
        error={null}
        onSearch={onSearch}
        onSelectResult={vi.fn()}
      />,
    );

    await user.type(screen.getByLabelText("Address"), "Berlin");
    await user.click(screen.getByRole("button", { name: "Search" }));

    expect(onSearch).toHaveBeenCalledWith("Berlin");
  });

  it("shows result labels and calls onSelectResult when one is clicked", async () => {
    const onSelectResult = vi.fn();
    const user = userEvent.setup();
    const result = { label: "Berlin, Germany", point: { lon: 13.4, lat: 52.5 } };
    render(
      <AddressSearchBox
        disabled={false}
        status="ready"
        results={[result]}
        error={null}
        onSearch={vi.fn()}
        onSelectResult={onSelectResult}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Berlin, Germany" }));

    expect(onSelectResult).toHaveBeenCalledWith(result);
  });

  it("clears the input after a result is selected", async () => {
    const user = userEvent.setup();
    const result = { label: "Berlin, Germany", point: { lon: 13.4, lat: 52.5 } };
    render(
      <AddressSearchBox
        disabled={false}
        status="ready"
        results={[result]}
        error={null}
        onSearch={vi.fn()}
        onSelectResult={vi.fn()}
      />,
    );

    await user.type(screen.getByLabelText("Address"), "Berlin");
    await user.click(screen.getByRole("button", { name: "Berlin, Germany" }));

    expect(screen.getByLabelText("Address")).toHaveValue("");
  });

  it("shows an empty state when the search returned no results", () => {
    render(
      <AddressSearchBox
        disabled={false}
        status="ready"
        results={[]}
        error={null}
        onSearch={vi.fn()}
        onSelectResult={vi.fn()}
      />,
    );

    expect(screen.getByText("No results")).toBeInTheDocument();
  });

  it("shows the error message on error status", () => {
    render(
      <AddressSearchBox
        disabled={false}
        status="error"
        results={[]}
        error="network down"
        onSearch={vi.fn()}
        onSelectResult={vi.fn()}
      />,
    );

    expect(screen.getByText("network down")).toBeInTheDocument();
  });

  it("disables the search button while loading", () => {
    render(
      <AddressSearchBox
        disabled={false}
        status="loading"
        results={[]}
        error={null}
        onSearch={vi.fn()}
        onSelectResult={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: "Searching..." })).toBeDisabled();
  });
});
