import { describe, expect, it } from "vitest";
import { createFolder } from "../../src/domain/folder.js";
import { createGroundOverlayGeometry, createPointGeometry } from "../../src/domain/geometry.js";
import { createPlacemark } from "../../src/domain/placemark.js";
import { createStyle } from "../../src/domain/style.js";
import { bytesToDataUrl } from "../../src/kml/kmzAssets.js";
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

  it("finds a .kml entry even when it isn't named doc.kml", async () => {
    const { zipSync, strToU8 } = await import("fflate");
    const folder = createFolder({ name: "Trips", parentId: null, order: 0 });
    const xml = (await import("../../src/kml/serializeKml.js")).serializeKml({
      folders: [folder],
      placemarks: [],
      styles: [],
    });
    const bytes = zipSync({ "map.kml": strToU8(xml) });

    const parsed = await parseKmz(bytes);
    expect(parsed.folders).toEqual([folder]);
  });

  it("round-trips an embedded icon image through export and re-import as a real archive entry", async () => {
    const iconBytes = new Uint8Array([137, 80, 78, 71, 1, 2, 3, 4]);
    const style = createStyle({
      outlineColor: "#000000",
      outlineWidth: 1,
      fillColor: "#000000",
      fillOpacity: 1,
      iconUrl: bytesToDataUrl(iconBytes, "icon.png"),
    });
    const placemark = createPlacemark({
      name: "Pinned",
      folderId: null,
      geometry: createPointGeometry({ lon: 0, lat: 0 }),
      styleId: style.id,
    });

    const bytes = await serializeKmz({ folders: [], placemarks: [placemark], styles: [style] });
    const { unzipSync } = await import("fflate");
    const entries = unzipSync(bytes);
    const assetEntry = Object.keys(entries).find((p) => p.startsWith("files/"));
    expect(assetEntry).toBeDefined();
    expect(entries[assetEntry!]).toEqual(iconBytes);

    const parsed = await parseKmz(bytes);
    const decoded = parsed.styles[0]!.iconUrl!;
    expect(decoded.startsWith("data:image/png;base64,")).toBe(true);
  });

  it("round-trips a GroundOverlay's embedded image", async () => {
    const imageBytes = new Uint8Array([1, 2, 3, 4, 5, 6]);
    const overlay = createPlacemark({
      name: "Scan",
      folderId: null,
      geometry: createGroundOverlayGeometry(
        { north: 1, south: 0, east: 1, west: 0 },
        bytesToDataUrl(imageBytes, "scan.jpg"),
      ),
    });

    const bytes = await serializeKmz({ folders: [], placemarks: [overlay], styles: [] });
    const parsed = await parseKmz(bytes);

    const geometry = parsed.placemarks[0]!.geometry;
    expect(geometry.type).toBe("GroundOverlay");
    if (geometry.type === "GroundOverlay") {
      expect(geometry.imageUrl.startsWith("data:image/jpeg;base64,")).toBe(true);
    }
  });
});
