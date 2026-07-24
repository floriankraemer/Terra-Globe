import { describe, expect, it } from "vitest";
import { formatArea, formatCoordinate, formatDistance } from "../../src/domain/units.js";

describe("formatDistance", () => {
  it("formats meters under 1km in metric", () => {
    expect(formatDistance(500, "metric")).toBe("500 m");
  });

  it("formats distances over 1km in metric as km", () => {
    expect(formatDistance(1500, "metric")).toBe("1.50 km");
  });

  it("formats short distances in imperial as feet", () => {
    expect(formatDistance(30, "imperial")).toBe("98 ft");
  });

  it("formats long distances in imperial as miles", () => {
    expect(formatDistance(1609.344, "imperial")).toBe("1.00 mi");
  });
});

describe("formatArea", () => {
  it("formats small areas in metric as square meters", () => {
    expect(formatArea(500, "metric")).toBe("500 m²");
  });

  it("formats large areas in metric as hectares", () => {
    expect(formatArea(50_000, "metric")).toBe("5.00 ha");
  });

  it("formats small areas in imperial as square feet", () => {
    expect(formatArea(10, "imperial")).toBe("108 sq ft");
  });

  it("formats large areas in imperial as acres", () => {
    expect(formatArea(40_468.6, "imperial")).toBe("10.00 acres");
  });

  it("formats very large areas in metric as square kilometers", () => {
    expect(formatArea(5_000_000, "metric")).toBe("5.00 km²");
  });

  it("formats very large areas in imperial as square miles", () => {
    expect(formatArea(2_594_034.9667584, "imperial")).toBe("1.00 sq mi");
  });
});

describe("formatCoordinate", () => {
  it("formats as decimal degrees", () => {
    expect(formatCoordinate({ lat: 52.5, lon: 13.4 }, "decimal")).toBe("52.50000, 13.40000");
  });

  it("formats as degrees/minutes/seconds with hemisphere letters", () => {
    expect(formatCoordinate({ lat: 52.5, lon: 13.4 }, "dms")).toBe(`52°30'0.00"N, 13°24'0.00"E`);
  });

  it("uses S/W hemisphere letters for negative coordinates", () => {
    expect(formatCoordinate({ lat: -33.87, lon: -151.21 }, "dms")).toBe(
      `33°52'12.00"S, 151°12'36.00"W`,
    );
  });
});
