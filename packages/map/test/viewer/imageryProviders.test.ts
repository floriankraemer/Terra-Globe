import { describe, expect, it } from "vitest";
import { openStreetMapSource, openTopoMapSource } from "../../src/viewer/imageryProviders/index.js";

describe("openStreetMapSource", () => {
  it("points at the OSM standard tile endpoint", () => {
    const source = openStreetMapSource();
    expect(source.url).toBe("https://tile.openstreetmap.org/{z}/{x}/{y}.png");
    expect(source.credit).toMatch(/OpenStreetMap/i);
    expect(source.maximumLevel).toBe(19);
  });
});

describe("openTopoMapSource", () => {
  it("points at the OpenTopoMap tile endpoint", () => {
    const source = openTopoMapSource();
    expect(source.url).toBe("https://{s}.tile.opentopomap.org/{z}/{x}/{y}.png");
    expect(source.subdomains).toEqual(["a", "b", "c"]);
    expect(source.credit).toMatch(/OpenTopoMap/i);
    expect(source.maximumLevel).toBe(17);
  });
});
