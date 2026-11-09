import { Blob as NodeBlob } from "node:buffer";
import { describe, expect, it } from "vitest";
import { base64ToBlob, blobToBase64 } from "./exportWorkerProtocol.js";

// jsdom's own Blob implementation doesn't support arrayBuffer() (unlike every real browser/webview
// this code actually runs in), so tests construct input blobs via Node's full Blob instead - see
// https://github.com/jsdom/jsdom/issues/2555. base64ToBlob's *output* still uses the ambient
// (jsdom) Blob constructor, which is fine since production code never calls .arrayBuffer() on it.
describe("blobToBase64 / base64ToBlob", () => {
  it("round-trips arbitrary bytes through base64", async () => {
    const bytes = new Uint8Array([0, 1, 2, 255, 254, 128, 10, 13, 65, 90]);
    const blob = new NodeBlob([bytes], { type: "image/png" }) as unknown as Blob;

    const base64 = await blobToBase64(blob);
    expect(base64).toBe(Buffer.from(bytes).toString("base64"));

    const decoded = base64ToBlob(base64, "image/png");
    expect(decoded.type).toBe("image/png");
    expect(decoded.size).toBe(bytes.length);
  });

  it("round-trips an empty blob", async () => {
    const blob = new NodeBlob([], { type: "image/png" }) as unknown as Blob;
    const base64 = await blobToBase64(blob);
    expect(base64).toBe("");

    const decoded = base64ToBlob(base64, "image/png");
    expect(decoded.size).toBe(0);
  });
});
