import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterAll, describe, expect, it } from "vitest";

import { DocsMedia, ENCODER, isAnimatedPng, mediaKey, rasterKind, webpName } from "./docs-images.mts";

const root = fs.mkdtempSync(path.join(os.tmpdir(), "docs-images-"));
afterAll(() => fs.rmSync(root, { recursive: true, force: true }));
const output = path.join(root, "out");
const png = (chunks: string[], pad = 0) => Buffer.concat([
  Buffer.from("89504e470d0a1a0a", "hex"),
  ...chunks.map((type) => Buffer.concat([Buffer.alloc(4), Buffer.from(type), Buffer.alloc(4)])),
  Buffer.alloc(pad),
]);
const big = png(["IHDR", "IDAT", "IEND"], 64 * 1024);
const noMagick = { output, magick: path.join(root, "no-magick") };

describe("DocsMedia", () => {
  it("detects rasters by magic bytes and animated PNGs", () => {
    expect(rasterKind(big)).toBe("png");
    expect(rasterKind(Buffer.from("GIF89a"))).toBe("gif");
    expect(rasterKind(Buffer.from([0xff, 0xd8, 0xff]))).toBe("jpg");
    expect(rasterKind(Buffer.from("<svg/>"))).toBeNull();
    expect(isAnimatedPng(png(["IHDR", "acTL", "IDAT", "IEND"]))).toBe(true);
    expect(isAnimatedPng(big)).toBe(false);
  });

  it("names the replacement after the source", () => {
    expect(webpName("creator/a.PNG")).toBe("creator/a.webp");
    expect(webpName(".gitbook/assets/mesh-names")).toBe(".gitbook/assets/mesh-names.webp");
  });

  it("encodes nothing in check mode and reports the missing WebP", () => {
    const media = new DocsMedia({ ...noMagick, readOnly: true });
    media.replace(big, "creator/big.webp", "creator/big.png");
    expect(media.missing).toEqual(["/docs/creator/big.webp"]);
    expect(media.files()).toEqual([]);
  });

  it("reuses a current replacement without ImageMagick and re-encodes a stale one", async () => {
    const entry = { source: "creator/big.png", encoder: ENCODER, key: mediaKey(big), width: 10, height: 5, bytes: 4, sourceBytes: big.length };
    fs.mkdirSync(path.join(output, "creator"), { recursive: true });
    fs.writeFileSync(path.join(output, "creator/big.webp"), "webp");
    fs.writeFileSync(path.join(output, "_media.json"), JSON.stringify({ "/docs/creator/big.webp": entry }));
    const media = new DocsMedia(noMagick);
    media.replace(big, "creator/big.webp", "creator/big.png");
    expect(JSON.parse(media.manifest())).toEqual({ "/docs/creator/big.webp": entry });
    expect(media.files()).toEqual(["creator/big.webp"]);
    expect(new DocsMedia(noMagick).keep("creator/big.webp", "creator/big.png")).toBe(true);

    fs.writeFileSync(path.join(output, "_media.json"), JSON.stringify({ "/docs/creator/big.webp": { ...entry, encoder: "old" } }));
    expect(new DocsMedia(noMagick).keep("creator/big.webp", "creator/big.png")).toBe(false);
    const stale = new DocsMedia(noMagick);
    stale.replace(big, "creator/big.webp", "creator/big.png");
    await expect(stale.flush()).rejects.toThrow(/ImageMagick/);
  });
});
