import { describe, expect, it } from "vitest";
import {
  base64ToBytes,
  bytesToBase64,
  bytesToDataUrl,
  decodeDataUrl,
  extensionForMimeType,
  guessMimeType,
  normalizeAssetPath,
} from "../../src/kml/kmzAssets.js";

describe("kmzAssets", () => {
  it("round-trips arbitrary bytes through base64", () => {
    for (const bytes of [
      new Uint8Array([]),
      new Uint8Array([0]),
      new Uint8Array([1, 2]),
      new Uint8Array([1, 2, 3]),
      new Uint8Array([255, 0, 128, 64, 32, 16, 8, 4, 2, 1]),
    ]) {
      expect(base64ToBytes(bytesToBase64(bytes))).toEqual(bytes);
    }
  });

  it("guesses mime types from file extension", () => {
    expect(guessMimeType("icon.png")).toBe("image/png");
    expect(guessMimeType("model.glb")).toBe("model/gltf-binary");
    expect(guessMimeType("unknown.xyz")).toBe("application/octet-stream");
  });

  it("round-trips bytes through a data URL", () => {
    const bytes = new Uint8Array([10, 20, 30]);
    const url = bytesToDataUrl(bytes, "photo.jpg");
    const decoded = decodeDataUrl(url);
    expect(decoded?.mimeType).toBe("image/jpeg");
    expect(decoded?.bytes).toEqual(bytes);
  });

  it("returns undefined decoding a non data: URL", () => {
    expect(decodeDataUrl("https://example.com/icon.png")).toBeUndefined();
  });

  it("maps mime types back to a file extension", () => {
    expect(extensionForMimeType("image/png")).toBe("png");
    expect(extensionForMimeType("application/x-unknown")).toBe("bin");
  });

  it("normalizes local asset paths", () => {
    expect(normalizeAssetPath("./files/icon.png")).toBe("files/icon.png");
    expect(normalizeAssetPath("/files/icon.png")).toBe("files/icon.png");
    expect(normalizeAssetPath("files/icon.png")).toBe("files/icon.png");
  });
});
