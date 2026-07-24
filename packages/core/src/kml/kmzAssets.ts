const BASE64_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/** Cross-environment base64 encode (no Buffer/btoa dependency - works in Node tests, browser, and the Tauri webview). */
export function bytesToBase64(bytes: Uint8Array): string {
  let result = "";
  for (let i = 0; i < bytes.length; i += 3) {
    const b0 = bytes[i]!;
    const b1 = bytes[i + 1];
    const b2 = bytes[i + 2];
    result += BASE64_CHARS[b0 >> 2];
    result += BASE64_CHARS[((b0 & 0x03) << 4) | (b1 === undefined ? 0 : b1 >> 4)];
    result += b1 === undefined ? "=" : BASE64_CHARS[((b1 & 0x0f) << 2) | (b2 === undefined ? 0 : b2 >> 6)];
    result += b2 === undefined ? "=" : BASE64_CHARS[b2 & 0x3f];
  }
  return result;
}

export function base64ToBytes(base64: string): Uint8Array {
  const clean = base64.replace(/=+$/, "");
  const bytes: number[] = [];
  let buffer = 0;
  let bits = 0;
  for (const char of clean) {
    const value = BASE64_CHARS.indexOf(char);
    if (value === -1) continue;
    buffer = (buffer << 6) | value;
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      bytes.push((buffer >> bits) & 0xff);
    }
  }
  return new Uint8Array(bytes);
}

const MIME_TYPES: Record<string, string> = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  gif: "image/gif",
  svg: "image/svg+xml",
  webp: "image/webp",
  bmp: "image/bmp",
  dae: "model/vnd.collada+xml",
  gltf: "model/gltf+json",
  glb: "model/gltf-binary",
  bin: "application/octet-stream",
};

function extensionOf(path: string): string {
  const match = /\.([a-z0-9]+)$/i.exec(path);
  return match ? match[1]!.toLowerCase() : "";
}

export function guessMimeType(path: string): string {
  return MIME_TYPES[extensionOf(path)] ?? "application/octet-stream";
}

export function bytesToDataUrl(bytes: Uint8Array, path: string): string {
  return `data:${guessMimeType(path)};base64,${bytesToBase64(bytes)}`;
}

export interface DecodedDataUrl {
  mimeType: string;
  bytes: Uint8Array;
}

export function decodeDataUrl(url: string): DecodedDataUrl | undefined {
  const match = /^data:([^;]+);base64,(.*)$/s.exec(url);
  if (!match) return undefined;
  return { mimeType: match[1]!, bytes: base64ToBytes(match[2]!) };
}

const EXTENSION_BY_MIME: Record<string, string> = Object.fromEntries(
  Object.entries(MIME_TYPES).map(([ext, mime]) => [mime, ext]),
);

export function extensionForMimeType(mimeType: string): string {
  return EXTENSION_BY_MIME[mimeType] ?? "bin";
}

/** Normalizes a KMZ-internal relative path (strips leading "./" and "/"). */
export function normalizeAssetPath(path: string): string {
  return path.replace(/^\.\//, "").replace(/^\//, "");
}
