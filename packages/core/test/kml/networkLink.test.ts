import { describe, expect, it, vi } from "vitest";
import { parseKml } from "../../src/kml/parseKml.js";
import { resolveNetworkLinks } from "../../src/kml/networkLink.js";

function fakeResponse(body: string, ok = true, status = 200): Response {
  return { ok, status, text: () => Promise.resolve(body) } as Response;
}

const CHILD_KML = `<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2">
  <Document>
    <Placemark><name>Remote Spot</name><Point><coordinates>1,1,0</coordinates></Point></Placemark>
  </Document>
</kml>`;

describe("resolveNetworkLinks", () => {
  it("fetches a NetworkLink and inlines its placemarks into the linking folder", async () => {
    const xml = `<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2">
  <Document>
    <Folder>
      <name>Remote</name>
      <NetworkLink><Link><href>https://example.com/child.kml</href></Link></NetworkLink>
    </Folder>
  </Document>
</kml>`;
    const parsed = parseKml(xml);
    expect(parsed.networkLinks).toEqual([
      { folderId: parsed.folders[0]!.id, href: "https://example.com/child.kml" },
    ]);

    const fetchMock = vi.fn().mockResolvedValue(fakeResponse(CHILD_KML));
    const resolved = await resolveNetworkLinks(parsed, { fetch: fetchMock });

    expect(fetchMock).toHaveBeenCalledWith("https://example.com/child.kml");
    expect(resolved.networkLinks).toEqual([]);
    expect(resolved.placemarks).toHaveLength(1);
    expect(resolved.placemarks[0]!.name).toBe("Remote Spot");
    expect(resolved.placemarks[0]!.folderId).toBe(parsed.folders[0]!.id);
  });

  it("records a warning instead of throwing when a NetworkLink fails", async () => {
    const parsed = {
      folders: [],
      placemarks: [],
      styles: [],
      screenOverlays: [],
      networkLinks: [{ folderId: null, href: "https://example.com/missing.kml" }],
      warnings: [],
    };
    const fetchMock = vi.fn().mockResolvedValue(fakeResponse("", false, 404));

    const resolved = await resolveNetworkLinks(parsed, { fetch: fetchMock });

    expect(resolved.placemarks).toEqual([]);
    expect(resolved.warnings[0]).toMatch(/Failed to resolve NetworkLink.*404/);
  });

  it("stops re-fetching a NetworkLink that forms a cycle", async () => {
    const cyclicXml = `<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2">
  <Document>
    <NetworkLink><Link><href>https://example.com/self.kml</href></Link></NetworkLink>
  </Document>
</kml>`;
    const parsed = {
      folders: [],
      placemarks: [],
      styles: [],
      screenOverlays: [],
      networkLinks: [{ folderId: null, href: "https://example.com/self.kml" }],
      warnings: [],
    };
    const fetchMock = vi.fn().mockResolvedValue(fakeResponse(cyclicXml));

    const resolved = await resolveNetworkLinks(parsed, { fetch: fetchMock });

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(resolved.warnings.some((w) => /cycle/.test(w))).toBe(true);
  });
});
