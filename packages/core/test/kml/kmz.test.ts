import { describe, expect, it } from "vitest";
import { createFolder } from "../../src/domain/folder.js";
import { createPointGeometry } from "../../src/domain/geometry.js";
import { createPlacemark } from "../../src/domain/placemark.js";
import { parseKmz } from "../../src/kml/kmz.js";
import { serializeKmz } from "../../src/kml/kmz.js";

describe("serializeKmz / parseKmz", () => {
  it("zips doc.kml and can read it back, round-tripping folders and placemarks", async () => {
    const folder = createFolder({ name: "Trips", parentId: null, order: 0 });
    const placemark = createPlacemark({
      name: "Berlin",
      folderId: folder.id,
      geometry: createPointGeometry({ lon: 13.4, lat: 52.5 }),
    });

    const bytes = await serializeKmz({ folders: [folder], placemarks: [placemark], styles: [] });
    expect(bytes).toBeInstanceOf(Uint8Array);

    const parsed = await parseKmz(bytes);

    expect(parsed.folders).toEqual([folder]);
    expect(parsed.placemarks).toEqual([placemark]);
  });

  it("rejects a KMZ archive with no doc.kml entry", async () => {
    const { zipSync, strToU8 } = await import("fflate");
    const bytes = zipSync({ "readme.txt": strToU8("not a kml file") });

    await expect(parseKmz(bytes)).rejects.toThrow(/doc\.kml/i);
  });
});
