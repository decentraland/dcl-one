import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { createServer, request } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { deflateSync } from "node:zlib";
import { assertFreshBuild, buildSites, dclOneIdentity, isCommitted, localEnvFiles, stampFile } from "./build.mts";
import type { Collected, Node } from "./collect.mts";
import { foreignRequest, guardLoopback } from "./guard.mts";
import { referencePin } from "./paths.mts";
import { decodePng, encodePng, imageSize, pasteTile, placeholderPng, type Rgba } from "./png.mts";
import { pixelDiff } from "./pixeldiff.mts";
import { fillMisses, learnable, Recording, savingAfter } from "./recording.mts";
import { classifyMiss, classifyUnknown, gateFailures, isIdentical } from "./verdict.mts";
import { fromBrowser, imageSeed, startReplay } from "./servers.mts";
import { canonicalQuery, fromNodePath, fromShopApi } from "./services.mts";
import { compareStructure, coverByRegions, line } from "./structure.mts";

const solid = (w: number, h: number, rgb: [number, number, number]): Rgba => {
  const data = new Uint8Array(w * h * 4);
  for (let i = 0; i < w * h; i++) data.set([...rgb, 255], i * 4);
  return { width: w, height: h, data };
};

/** A PNG of unfiltered rows, in the given colour type, its compressed data split over `idats` chunks. */
const rawPng = (w: number, h: number, colorType: number, px: Uint8Array, extra: [string, Buffer][] = [], idats = 1): Buffer => {
  const stride = px.length / h;
  const raw = Buffer.alloc((stride + 1) * h);
  for (let y = 0; y < h; y++) raw.set(px.subarray(y * stride, (y + 1) * stride), y * (stride + 1) + 1);
  const chunk = (type: string, data: Buffer) => {
    const head = Buffer.alloc(8);
    head.writeUInt32BE(data.length, 0);
    head.write(type, 4, "ascii");
    return Buffer.concat([head, data, Buffer.alloc(4)]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8;
  ihdr[9] = colorType;
  const z = deflateSync(raw);
  const step = Math.ceil(z.length / idats);
  const parts = Array.from({ length: idats }, (_, i) => chunk("IDAT", z.subarray(i * step, (i + 1) * step)));
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk("IHDR", ihdr), ...extra.map(([t, d]) => chunk(t, d)), ...parts, chunk("IEND", Buffer.alloc(0))]);
};
const pixels = (img: Rgba) => Array.from({ length: img.width * img.height }, (_, i) => [...img.data.subarray(i * 4, i * 4 + 4)]);

describe("png", () => {
  it("round-trips RGBA and reads its size", () => {
    const img = solid(7, 5, [10, 200, 30]);
    img.data.set([1, 2, 3, 4], 0);
    const png = encodePng(img);
    assert.deepEqual(imageSize(png), { width: 7, height: 5 });
    assert.deepEqual(decodePng(png), img);
    assert.deepEqual(imageSize(placeholderPng("seed", 4000, 1000)), { width: 1024, height: 256 });
  });
  it("undoes each of the row filters 1 to 4, in RGB and RGBA", () => {
    for (const channels of [3, 4]) {
      const w = 5, h = 4, stride = w * channels;
      const px = Uint8Array.from({ length: stride * h }, (_, i) => (i * 37 + (i >> 3) * 11) & 0xff);
      for (const filter of [1, 2, 3, 4]) {
        const raw = Buffer.alloc((stride + 1) * h);
        for (let y = 0; y < h; y++) {
          raw[y * (stride + 1)] = filter;
          for (let x = 0; x < stride; x++) {
            const a = x >= channels ? px[y * stride + x - channels]! : 0;
            const b = y > 0 ? px[(y - 1) * stride + x]! : 0;
            const c = x >= channels && y > 0 ? px[(y - 1) * stride + x - channels]! : 0;
            const p = a + b - c;
            const pred = filter === 1 ? a : filter === 2 ? b : filter === 3 ? (a + b) >> 1 : Math.abs(p - a) <= Math.abs(p - b) && Math.abs(p - a) <= Math.abs(p - c) ? a : Math.abs(p - b) <= Math.abs(p - c) ? b : c;
            raw[y * (stride + 1) + 1 + x] = (px[y * stride + x]! - pred) & 0xff;
          }
        }
        const chunk = (type: string, data: Buffer) => {
          const head = Buffer.alloc(8);
          head.writeUInt32BE(data.length, 0);
          head.write(type, 4, "ascii");
          return Buffer.concat([head, data, Buffer.alloc(4)]);
        };
        const ihdr = Buffer.alloc(13);
        ihdr.writeUInt32BE(w, 0);
        ihdr.writeUInt32BE(h, 4);
        ihdr[8] = 8;
        ihdr[9] = channels === 4 ? 6 : 2;
        const png = Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]);
        const got = decodePng(png);
        for (let i = 0; i < w * h; i++) {
          const want = channels === 4 ? [...px.subarray(i * 4, i * 4 + 4)] : [...px.subarray(i * 3, i * 3 + 3), 255];
          assert.deepEqual([...got.data.subarray(i * 4, i * 4 + 4)], want, `filter ${filter}, ${channels} channels, pixel ${i}`);
        }
      }
    }
  });
  it("stitches tiles by their real scroll offset, keeping the rows a clamped last tile overlaps", () => {
    // A 250-row page seen through a 100-row viewport: the third scroll clamps at 150, not 200.
    const out: Rgba = { width: 1, height: 250, data: new Uint8Array(250 * 4) };
    const tile = (top: number, mark?: number): Rgba => ({ width: 1, height: 100, data: Uint8Array.from({ length: 400 }, (_, i) => mark ?? (top + (i >> 2)) % 256) });
    let filled = pasteTile(out, 0, 0, tile(0));
    filled = pasteTile(out, filled, 100, tile(100));
    // Rows 150–199 were filled by the second tile; only the third tile's last 50 rows are new.
    const third = tile(150);
    third.data.fill(255, 0, 50 * 4);
    filled = pasteTile(out, filled, 150, third);
    assert.equal(filled, 250);
    for (let row = 0; row < 250; row++) assert.equal(out.data[row * 4], row % 256, `row ${row}`);
    assert.equal(pasteTile(out, filled, 150, tile(150, 9)), 250);
  });
  it("pastes a tile narrower or wider than the page row by row", () => {
    const out: Rgba = { width: 3, height: 2, data: new Uint8Array(3 * 2 * 4) };
    // Two rows of two pixels: 1 2 / 3 4. Each must land at the start of its own row.
    const narrow: Rgba = { width: 2, height: 2, data: Uint8Array.from([1, 2, 3, 4].flatMap((v) => [v, v, v, 255])) };
    assert.equal(pasteTile(out, 0, 0, narrow), 2);
    assert.deepEqual(pixels(out).map((p) => p[0]), [1, 2, 0, 3, 4, 0]);
    const wide: Rgba = { width: 4, height: 2, data: Uint8Array.from([1, 2, 3, 4, 5, 6, 7, 8].flatMap((v) => [v, v, v, 255])) };
    pasteTile(out, 0, 0, wide);
    assert.deepEqual(pixels(out).map((p) => p[0]), [1, 2, 3, 5, 6, 7]);
  });
  it("decodes grayscale, grayscale with alpha, and palette images, and data split over several IDAT chunks", () => {
    assert.deepEqual(pixels(decodePng(rawPng(2, 1, 0, Uint8Array.from([0, 200])))), [[0, 0, 0, 255], [200, 200, 200, 255]]);
    assert.deepEqual(pixels(decodePng(rawPng(2, 1, 4, Uint8Array.from([10, 255, 90, 30])))), [[10, 10, 10, 255], [90, 90, 90, 30]]);
    const palette = Buffer.from([255, 0, 0, 0, 255, 0, 0, 0, 255]);
    // tRNS covers the first two entries only; the third stays opaque.
    const indexed = rawPng(3, 1, 3, Uint8Array.from([2, 0, 1]), [["PLTE", palette], ["tRNS", Buffer.from([128, 0])]]);
    assert.deepEqual(pixels(decodePng(indexed)), [[0, 0, 255, 255], [255, 0, 0, 128], [0, 255, 0, 0]]);
    const rgba = Uint8Array.from({ length: 6 * 5 * 4 }, (_, i) => (i * 13) & 0xff);
    assert.deepEqual(decodePng(rawPng(6, 5, 6, rgba, [], 3)).data, rgba);
  });
  it("paints the same placeholder for the same seed and another for another", () => {
    assert.deepEqual(placeholderPng("a", 32, 16), placeholderPng("a", 32, 16));
    assert.notDeepEqual(placeholderPng("a", 32, 16), placeholderPng("b", 32, 16));
    assert.deepEqual(imageSize(placeholderPng("a", 32, 16)), { width: 32, height: 16 });
  });
});

describe("pixelDiff", () => {
  it("finds nothing between equal images", () => {
    assert.equal(pixelDiff(solid(20, 20, [50, 50, 50]), solid(20, 20, [50, 50, 50])).diff, 0);
  });
  it("counts a changed pixel, reports its band, and skips masked ones", () => {
    const a = solid(20, 20, [255, 255, 255]);
    const b = solid(20, 20, [255, 255, 255]);
    for (let y = 8; y < 12; y++) for (let x = 8; x < 12; x++) b.data.set([0, 0, 0, 255], (y * 20 + x) * 4);
    const r = pixelDiff(a, b);
    assert.equal(r.diff, 16);
    assert.deepEqual(r.bands[0], { y0: 8, y1: 11, x0: 8, x1: 11, pixels: 16 });
    const box = { x: 8, y: 8, w: 4, h: 4 };
    assert.equal(pixelDiff(a, b, { masks: { a: [box], b: [box] } }).diff, 0);
    assert.equal(pixelDiff(a, b, { regions: [box] }).diff, 0);
  });
  it("counts what only one side masks: a masked box that moved or grew", () => {
    const same = solid(20, 20, [255, 255, 255]);
    // The 3D box sits three rows lower on one side: past the 1px tolerance, two rows at each end are only one side's.
    const r = pixelDiff(same, same, { masks: { a: [{ x: 0, y: 4, w: 20, h: 10 }], b: [{ x: 0, y: 7, w: 20, h: 10 }] } });
    assert.equal(r.diff, 4 * 20);
    assert.equal(r.masked, 9 * 20);
    assert.equal(pixelDiff(same, same, { masks: { a: [{ x: 0, y: 0, w: 5, h: 5 }], b: [] } }).diff, 25);
  });
  it("tolerates a masked box's edge 1px off on the other side, as the structural diff does", () => {
    const same = solid(20, 30, [255, 255, 255]);
    // 224.0 against 223.9 in a page: floor() puts the second box's top one row higher.
    assert.equal(pixelDiff(same, same, { masks: { a: [{ x: 2, y: 10, w: 10, h: 10 }], b: [{ x: 2, y: 9.9, w: 10, h: 10 }] } }).diff, 0);
    assert.equal(pixelDiff(same, same, { masks: { a: [{ x: 2, y: 10, w: 10, h: 10 }], b: [{ x: 3, y: 11, w: 10, h: 10 }] } }).diff, 0);
  });
  it("counts area only one image has", () => {
    assert.equal(pixelDiff(solid(10, 10, [0, 0, 0]), solid(10, 12, [0, 0, 0])).diff, 20);
  });
});

describe("services", () => {
  it("maps canonical /shop-api routes onto the upstream service and path", () => {
    assert.deepEqual(fromShopApi("GET", "/v3/catalog/unified"), { service: "marketplace", path: "/v3/catalog/unified" });
    assert.deepEqual(fromShopApi("GET", "/v1/packs"), { service: "credits", path: "/credits/packs" });
    assert.deepEqual(fromShopApi("GET", "/v1/trades/abc"), { service: "marketplace", path: "/v1/trades/abc" });
    assert.deepEqual(fromShopApi("GET", "/v1/trades-mirror"), { service: "marketplace", path: "/v1/trades-mirror" });
    assert.deepEqual(fromShopApi("GET", "/v1/outfits"), { service: "shop-server", path: "/v1/outfits" });
    assert.deepEqual(fromShopApi("POST", "/v1/purchase-offers"), { service: "offers", path: "/v1/purchase-offers" });
    assert.deepEqual(fromNodePath("GET", "/lambdas/profiles/0x1"), { service: "peer", path: "/lambdas/profiles/0x1" });
  });
  it("orders and re-encodes a query so equal queries key alike", () => {
    assert.equal(canonicalQuery("?b=2&a=x%20y&a=1"), canonicalQuery("a=1&a=x+y&b=2"));
  });
});

describe("isIdentical", () => {
  const clean = { pixels: { diff: 0 }, structure: [], sides: { reference: {}, "dcl-one": {} } } as unknown as Parameters<typeof isIdentical>[0];
  const miss = { key: "k", service: "peer", method: "POST", path: "/content/entities/active", query: "", side: "dcl-one", request: { pointers: ["urn:decentraland:off-chain:base-avatars:basemale"] } } as const;
  it("refuses a page with a request the recording could not answer unless the page declares it", () => {
    assert.equal(isIdentical({ ...clean, misses: [] }), true);
    assert.equal(isIdentical({ ...clean, misses: [miss] }), false);
    const declared = classifyMiss(miss, { unrecorded: [{ match: /base-avatars/, why: "the masked preview's avatar" }] });
    assert.equal(declared.expected, "the masked preview's avatar");
    assert.equal(isIdentical({ ...clean, misses: [declared] }), true);
    assert.equal(classifyMiss(miss, { unrecorded: [{ match: /outfits/, why: "x" }] }).expected, undefined);
  });
  it("refuses a page that asked a host the harness does not know unless the page declares it", () => {
    const request = "GET embeds.beehiiv.com/form";
    assert.equal(isIdentical({ ...clean, misses: [], unknown: [classifyUnknown("reference", request, {})] }), false);
    const declared = classifyUnknown("reference", request, { unrecorded: [{ match: /^GET embeds\.beehiiv\.com\//, why: "the newsletter iframe" }] });
    assert.equal(declared.expected, "the newsletter iframe");
    assert.equal(isIdentical({ ...clean, misses: [], unknown: [declared] }), true);
  });
});

describe("gateFailures", () => {
  const result = (over: Record<string, unknown> = {}) =>
    ({ page: "item", viewport: "desktop", pixels: { diff: 0 }, structure: [], lines: [], sides: { reference: {}, "dcl-one": {} }, misses: [], unknown: [], ...over }) as unknown as Parameters<typeof gateFailures>[1][number];
  it("passes a gated page that is still identical and fails one that regressed, naming why", () => {
    assert.deepEqual(gateFailures(["item@desktop"], [result()]), []);
    const regressed = result({
      pixels: { diff: 12 },
      structure: [{ expected: undefined }, { expected: "a rule" }],
      lines: ["price: top 412→416", "nav: only in dcl-one  [expected: a rule]"],
    });
    assert.deepEqual(gateFailures(["item@desktop"], [regressed]), [{ id: "item@desktop", reasons: ["12 differing pixels", "price: top 412→416"] }]);
  });
  it("names undeclared misses and unknown hosts, which fail a page with no differing pixel", () => {
    const quiet = result({
      misses: [{ key: "k", service: "peer", method: "GET", path: "/lambdas/x", query: "a=1", side: "dcl-one" }],
      unknown: [{ side: "reference", request: "GET cdn.example/x.js" }, { side: "reference", request: "GET embeds.beehiiv.com/f", expected: "declared" }],
    });
    assert.deepEqual(gateFailures(["item@desktop"], [quiet])[0]!.reasons, ["unanswered (dcl-one): GET peer /lambdas/x?a=1", "unknown host (reference): GET cdn.example/x.js"]);
  });
  it("fails a gated page the run did not capture", () => {
    assert.deepEqual(gateFailures(["cart@mobile"], [result()]), [{ id: "cart@mobile", reasons: ["it was not captured"] }]);
  });
});

const node = (id: string, box: [number, number, number, number], extra: Partial<Node> = {}): Node => ({
  id,
  parent: null,
  box: { x: box[0], y: box[1], w: box[2], h: box[3] },
  style: { fontFamily: "Inter", fontSize: "15px", fontWeight: "600", lineHeight: "normal", letterSpacing: "normal", color: "rgb(255, 255, 255)", background: "rgba(0, 0, 0, 0)", borderRadius: "0px", textTransform: "none" },
  text: "",
  ownText: false,
  ...extra,
});
const page = (nodes: Node[], texts: Node[] = [], height = 1000): Collected => ({ url: "", width: 1440, height, nodes, texts, masks: [] });

describe("compareStructure", () => {
  it("reports a shift once, on the element where it starts", () => {
    const ref = page([node("rail", [0, 100, 1000, 300], { chain: ["rail"] }), node("rail/card", [10, 150, 200, 200], { chain: ["rail", "card"] })]);
    const one = page([node("rail", [0, 120, 1000, 300], { chain: ["rail"] }), node("rail/card", [10, 170, 200, 200], { chain: ["rail", "card"] })]);
    const lines = compareStructure(ref, one).map(line);
    assert.deepEqual(lines, ["rail: top 100→120"]);
  });
  it("pairs through a wrapper testid only one side has", () => {
    const ref = page([node("rail", [0, 100, 1000, 300], { chain: ["rail"] })]);
    const one = page([node("page", [0, 0, 1440, 1000], { chain: ["page"] }), node("page/rail", [0, 100, 1000, 300], { chain: ["page", "rail"] })]);
    assert.deepEqual(compareStructure(ref, one).map(line), ["page: only in dcl-one"]);
  });
  it("names style and text differences of a key element and marks declared expectations", () => {
    const ref = page([node("key:price", [0, 0, 40, 20], { text: "20", ownText: true })]);
    const one = page([node("key:price", [0, 0, 40, 20], { text: "20 Credits", ownText: true, style: { ...node("x", [0, 0, 0, 0]).style, fontWeight: "700" } })]);
    const d = compareStructure(ref, one, { expected: [{ match: /^key:price$/, why: "a rule" }] });
    assert.equal(line(d[0]!), 'key:price: font-weight 600→700, text "20"→"20 Credits"  [expected: a rule]');
  });
  it("lets `only` cover the named property, not another that starts with the same word", () => {
    const ref = page([node("key:footer", [0, 0, 40, 20], { text: "a", ownText: true })]);
    const one = page([node("key:footer", [0, 0, 40, 20], { text: "b", ownText: true, style: { ...node("x", [0, 0, 0, 0]).style, textTransform: "uppercase" } })]);
    const d = compareStructure(ref, one, { expected: [{ match: /^key:footer$/, why: "copy", only: ["text"] }] });
    assert.equal(d[0]!.expected, undefined);
    const textOnly = compareStructure(ref, page([node("key:footer", [0, 0, 40, 20], { text: "b", ownText: true })]), { expected: [{ match: /^key:footer$/, why: "copy", only: ["text"] }] });
    assert.equal(textOnly[0]!.expected, "copy");
  });
  it("covers only the declared status when an expectation names one", () => {
    const ref = page([node("field", [0, 0, 40, 20], { chain: ["field"] })]);
    const one = page([node("field", [0, 4, 40, 20], { chain: ["field"] }), node("hook", [0, 0, 40, 20], { chain: ["hook"] })]);
    const d = compareStructure(ref, one, { expected: [{ match: /^(field|hook)$/, status: "only-dcl-one", why: "a hook" }] });
    assert.deepEqual(d.map(line), ["field: top 0→4", "hook: only in dcl-one  [expected: a hook]"]);
  });
});

describe("imageSeed", () => {
  it("seeds a profile picture the same whether it comes from the service or through the node", () => {
    const id = "bafkreierayvvvos6tnk7ioanae5jjltt5x2jjc6xl46jwj66rsk6bqotpq";
    assert.equal(imageSeed(new URL(`https://profile-images.example/entities/${id}/face.png`)), `profile:${id}/face`);
    assert.equal(imageSeed(new URL(`http://127.0.0.1:5178/profile-images/entities/${id}/face.png`)), `profile:${id}/face`);
  });
});

describe("coverByRegions", () => {
  it("gives a difference lying wholly inside a declared region that region's rule, and nothing else", () => {
    const ref = page([node("row", [0, 100, 400, 200], { chain: ["row"] }), node("row/card", [10, 110, 100, 100], { chain: ["row", "card"] })]);
    const one = page([node("row", [0, 100, 400, 200], { chain: ["row"] }), node("row/card", [10, 120, 100, 100], { chain: ["row", "card"] }), node("tail", [0, 400, 100, 20], { chain: ["tail"] })]);
    const d = compareStructure(ref, one);
    coverByRegions(d, [{ why: "shuffled" }], [[{ x: 0, y: 100, w: 400, h: 200 }]], [[{ x: 0, y: 100, w: 400, h: 200 }]]);
    assert.deepEqual(d.map(line), ["row/card: top (in row) 10→20  [expected: shuffled]", "tail: only in dcl-one"]);
  });
  it("leaves a regression under one side's region unexpected when the other side's node lies outside its own", () => {
    // dcl-one's region grew over the card; the reference's region is where it was declared.
    const ref = page([node("card", [10, 400, 100, 100], { chain: ["card"] })]);
    const one = page([node("card", [10, 420, 100, 100], { chain: ["card"] })]);
    const d = compareStructure(ref, one);
    coverByRegions(d, [{ why: "nav links" }], [[{ x: 0, y: 0, w: 400, h: 60 }]], [[{ x: 0, y: 0, w: 400, h: 600 }]]);
    assert.deepEqual(d.map(line), ["card: top 400→420"]);
  });
});

describe("Recording batched lookups", () => {
  it("answers an id batch from rows recorded in other batches, and only for ids it was asked", async () => {
    const dir = mkdtempSync(join(tmpdir(), "parity-"));
    try {
      const rec = new Recording(dir);
      const row = (id: string) => ({ id, name: id });
      rec.putBody({ key: "k1", method: "GET", service: "marketplace", path: "/v3/catalog/items", query: "first=2&id=a-0&id=b-0", status: 200, contentType: "application/json" }, Buffer.from(JSON.stringify({ data: [row("a-0"), row("b-0")] })));
      rec.putBody({ key: "k2", method: "GET", service: "marketplace", path: "/v3/catalog/items", query: "first=1&id=c-0", status: 200, contentType: "application/json" }, Buffer.from(JSON.stringify({ data: [] })));
      const got = await rec.answer({ method: "GET", service: "marketplace", path: "/v3/catalog/items", search: "?id=b-0&id=c-0&first=2", side: "t" });
      assert.deepEqual(JSON.parse(got.body.toString()), { data: [row("b-0")], total: 1 });
      const unknown = await rec.answer({ method: "GET", service: "marketplace", path: "/v3/catalog/items", search: "?id=z-0", side: "t" });
      assert.equal(unknown.status, 404);
      assert.equal(rec.misses.length, 1);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe("fillMisses", () => {
  it("keeps what it filled when one read fails, and sends a non-JSON body as it was", async () => {
    const dir = mkdtempSync(join(tmpdir(), "parity-"));
    const realFetch = globalThis.fetch;
    const sent: { url: string; body?: string; type?: string }[] = [];
    globalThis.fetch = (async (url: string, init?: RequestInit) => {
      const headers = init?.headers as Record<string, string> | undefined;
      sent.push({ url, body: init?.body ? Buffer.from(init.body as Uint8Array).toString() : undefined, type: headers?.["content-type"] });
      if (url.includes("/fails")) throw new Error("offline");
      return new Response('{"ok":true}', { headers: { "content-type": "application/json" } });
    }) as typeof fetch;
    try {
      const replay = new Recording(dir);
      await replay.answer({ method: "GET", service: "peer", path: "/fails", search: "", side: "t" });
      const form = Buffer.from("a=1&b=2");
      await replay.answer({ method: "POST", service: "peer", path: "/form", search: "", body: form, headers: { "content-type": "application/x-www-form-urlencoded" }, side: "t" });
      const { filled, failed } = await fillMisses(replay, new Recording(dir, { live: true, retryDelays: [0, 0] }), () => {});
      assert.equal(filled, 1);
      assert.equal(failed.length, 1);
      assert.match(failed[0]!, /GET peer \/fails/);
      const post = sent.find((s) => s.url.endsWith("/form"))!;
      assert.equal(post.body, "a=1&b=2");
      assert.equal(post.type, "application/x-www-form-urlencoded");
      // Saved, under the key the replay missed on.
      const saved = new Recording(dir);
      const again = await saved.answer({ method: "POST", service: "peer", path: "/form", search: "", body: form, side: "t" });
      assert.equal(again.status, 200);
      assert.equal(saved.misses.length, 0);
    } finally {
      globalThis.fetch = realFetch;
      rmSync(dir, { recursive: true, force: true });
    }
  });
  it("asks a server error again, keeps none, and lists it as not filled, an rpc one too", async () => {
    const dir = mkdtempSync(join(tmpdir(), "parity-"));
    const realFetch = globalThis.fetch;
    let asked = 0;
    globalThis.fetch = (async () => {
      asked++;
      return new Response("busy", { status: 503, headers: { "content-type": "text/plain" } });
    }) as typeof fetch;
    try {
      const replay = new Recording(dir);
      await replay.answer({ method: "GET", service: "peer", path: "/busy", search: "", side: "t" });
      await replay.answer({ method: "POST", service: "rpc", path: "/v1/mainnet", search: "", body: Buffer.from('{"jsonrpc":"2.0","id":1,"method":"eth_chainId"}'), side: "t" });
      const { filled, failed } = await fillMisses(replay, new Recording(dir, { live: true, retryDelays: [0, 0] }), () => {});
      assert.equal(asked, 6);
      assert.equal(filled, 0);
      assert.deepEqual(failed, ["GET peer /busy: answered 503", "POST rpc /v1/mainnet eth_chainId: answered 503"]);
      const saved = new Recording(dir);
      assert.equal(saved.entries.size, 0);
      assert.equal((await saved.answer({ method: "GET", service: "peer", path: "/busy", search: "", side: "t" })).status, 404);
    } finally {
      globalThis.fetch = realFetch;
      rmSync(dir, { recursive: true, force: true });
    }
  });
  it("waits between attempts, at least as long as a Retry-After asks", async () => {
    const dir = mkdtempSync(join(tmpdir(), "parity-"));
    const realFetch = globalThis.fetch;
    const at: number[] = [];
    globalThis.fetch = (async () => {
      at.push(Date.now());
      return at.length < 3 ? new Response("busy", { status: 503, headers: { "retry-after": at.length === 2 ? "1" : "0" } }) : new Response("{}", { headers: { "content-type": "application/json" } });
    }) as typeof fetch;
    try {
      const live = new Recording(dir, { live: true, retryDelays: [50, 50] });
      assert.equal((await live.answer({ method: "GET", service: "peer", path: "/slow", search: "", side: "t" })).status, 200);
      assert.ok(at[1]! - at[0]! >= 45, "the configured pause");
      assert.ok(at[2]! - at[1]! >= 950, "the Retry-After");
    } finally {
      globalThis.fetch = realFetch;
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe("Recording", () => {
  it("lists an rpc POST that is not a JSON-RPC call as a miss instead of failing", async () => {
    const dir = mkdtempSync(join(tmpdir(), "parity-"));
    try {
      const rec = new Recording(dir);
      for (const body of [undefined, Buffer.alloc(0), Buffer.from("not json"), Buffer.from('[{"id":1}]')]) {
        const got = await rec.answer({ method: "POST", service: "rpc", path: "/v1/mainnet", search: "", body, side: "t" });
        assert.equal(got.status, 200);
      }
      assert.equal(rec.misses.length, 4);
      const { failed } = await fillMisses(rec, new Recording(dir, { live: true }), () => {});
      // An absent and an empty body are the same call; each distinct one is listed once.
      assert.equal(failed.length, 3);
      for (const f of failed) assert.match(f, /^POST rpc \/v1\/mainnet: not a JSON-RPC call$/);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("learns a picture's size only from Decentraland's hosts, redirects included", async () => {
    const dir = mkdtempSync(join(tmpdir(), "parity-"));
    const realFetch = globalThis.fetch;
    const asked: string[] = [];
    globalThis.fetch = (async (url: URL) => {
      asked.push(url.href);
      if (url.pathname === "/hop") return new Response(null, { status: 302, headers: { location: "http://169.254.169.254/latest" } });
      if (url.pathname === "/moved") return new Response(null, { status: 301, headers: { location: "https://cdn.decentraland.org/x.png" } });
      return new Response(new Uint8Array(placeholderPng("x", 40, 20)), { headers: { "content-type": "image/png" } });
    }) as typeof fetch;
    try {
      const rec = new Recording(dir, { live: true });
      await rec.learnImage("lan", "http://192.168.1.10/a.png");
      await rec.learnImage("link-local", "https://169.254.169.254/a.png");
      await rec.learnImage("lookalike", "https://peer.decentraland.org.example.com/a.png");
      await rec.learnImage("hop", "https://peer.decentraland.org/hop");
      await rec.learnImage("moved", "https://peer.decentraland.org/moved");
      await rec.learnImage("ok", "https://peer.decentraland.org/content/contents/x");
      assert.deepEqual(asked, ["https://peer.decentraland.org/hop", "https://peer.decentraland.org/moved", "https://cdn.decentraland.org/x.png", "https://peer.decentraland.org/content/contents/x"]);
      assert.deepEqual([...rec.images.keys()], ["moved", "ok"]);
      assert.deepEqual(rec.images.get("moved"), { width: 40, height: 20 });
      assert.ok(learnable(new URL("https://decentraland.org/x.png")));
      assert.ok(!learnable(new URL("http://peer.decentraland.org/x.png")));
    } finally {
      globalThis.fetch = realFetch;
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("saves what a recording read when the run dies midway, marked incomplete until a run finishes", async () => {
    const dir = mkdtempSync(join(tmpdir(), "parity-"));
    try {
      const live = new Recording(dir, { live: true });
      const targets: Record<string, string> = {};
      const manifest = () => ({ set: "t", recordedAt: "2026-10-30T00:00:00.000Z", targets });
      const crash = async () => {
        live.putBody({ key: "k", method: "GET", service: "peer", path: "/a", query: "", status: 200, contentType: "application/json" }, Buffer.from("{}"));
        targets.item = "/shop/items/x";
        throw new Error("browser crashed");
      };
      await assert.rejects(savingAfter(live, crash, manifest), /browser crashed/);
      const saved = new Recording(dir);
      assert.deepEqual([...saved.entries.keys()], ["k"]);
      assert.deepEqual(saved.manifest?.targets, { item: "/shop/items/x" });
      assert.equal(saved.manifest?.incomplete, true);
      // Running again goes on from it, and a finished run clears the mark.
      const listeners = process.listenerCount("SIGINT");
      const again = new Recording(dir, { live: true });
      assert.equal(await savingAfter(again, async () => "done", manifest), "done");
      assert.equal(new Recording(dir).manifest?.incomplete, undefined);
      assert.equal(process.listenerCount("SIGINT"), listeners, "the interrupt handler is lifted");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("lists a server error met while recording, and asks again for one an older recording kept", async () => {
    const dir = mkdtempSync(join(tmpdir(), "parity-"));
    const realFetch = globalThis.fetch;
    let up = false;
    globalThis.fetch = (async () => (up ? new Response('{"ok":true}', { headers: { "content-type": "application/json" } }) : new Response("down", { status: 502 }))) as typeof fetch;
    try {
      const live = new Recording(dir, { live: true, retryDelays: [0, 0] });
      assert.equal((await live.answer({ method: "GET", service: "marketplace", path: "/v1/items", search: "", side: "t" })).status, 502);
      assert.deepEqual(live.failures, ["GET marketplace /v1/items: answered 502"]);
      assert.equal(live.entries.size, 0);
      // An entry a recording kept before server errors were refused.
      live.putBody({ key: "GET marketplace /v1/old", method: "GET", service: "marketplace", path: "/v1/old", query: "", status: 503, contentType: "text/plain" }, Buffer.from("down"));
      up = true;
      assert.equal((await live.answer({ method: "GET", service: "marketplace", path: "/v1/old", search: "", side: "t" })).status, 200);
      assert.equal(live.entries.get("GET marketplace /v1/old")?.status, 200);
    } finally {
      globalThis.fetch = realFetch;
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe("build identity", () => {
  const git = (root: string, ...args: string[]) => {
    const r = spawnSync("git", ["-c", "user.name=parity", "-c", "user.email=parity@example.com", "-c", "commit.gpgsign=false", ...args], { cwd: root, encoding: "utf8" });
    assert.equal(r.status, 0, r.stderr);
    return r.stdout.trim();
  };
  const put = (root: string, file: string, text: string) => {
    mkdirSync(join(root, file, ".."), { recursive: true });
    writeFileSync(join(root, file), text);
  };

  it("names every uncommitted state that can reach the build, and only those", () => {
    const root = mkdtempSync(join(tmpdir(), "parity-git-"));
    try {
      git(root, "init", "-q");
      put(root, ".gitignore", ".env\n.env.local\nbuild/\n");
      put(root, "apps/sites/src/a.ts", "a");
      put(root, "apps/sites/vite.config.ts", "v1");
      put(root, "tooling/shop-parity/pages.mts", "p1");
      git(root, "add", ".");
      git(root, "commit", "-q", "-m", "init");
      const head = git(root, "rev-parse", "--short=12", "HEAD");
      assert.equal(dclOneIdentity(root), head);
      assert.ok(isCommitted(root));

      put(root, "apps/sites/vite.config.ts", "v2");
      const first = dclOneIdentity(root);
      assert.match(first, new RegExp(`^${head}\\+[0-9a-f]{12}$`));
      put(root, "apps/sites/vite.config.ts", "v3");
      assert.notEqual(dclOneIdentity(root), first, "another uncommitted state");
      put(root, "apps/sites/vite.config.ts", "v1");
      assert.equal(dclOneIdentity(root), head);

      put(root, "library/catalyst-api/new.ts", "n");
      assert.notEqual(dclOneIdentity(root), head, "an untracked file outside apps/");
      rmSync(join(root, "library"), { recursive: true });

      for (const env of [".env", ".env.local"]) {
        put(root, `apps/sites/${env}`, "VITE_X=1");
        const withEnv = dclOneIdentity(root);
        assert.notEqual(withEnv, head, `a gitignored ${env} Vite reads`);
        put(root, `apps/sites/${env}`, "VITE_X=2");
        assert.notEqual(dclOneIdentity(root), withEnv, `another value in ${env}`);
        assert.ok(!isCommitted(root));
        rmSync(join(root, `apps/sites/${env}`));
      }

      // An untracked repository inside the tree is listed as a directory and counts by its own state.
      const nested = join(root, "vendor/nested");
      put(root, "vendor/nested/x.ts", "x1");
      git(nested, "init", "-q");
      git(nested, "add", ".");
      git(nested, "commit", "-q", "-m", "n");
      const withNested = dclOneIdentity(root);
      assert.notEqual(withNested, head);
      put(root, "vendor/nested/x.ts", "x2");
      assert.notEqual(dclOneIdentity(root), withNested);
      rmSync(join(root, "vendor"), { recursive: true });
      assert.equal(dclOneIdentity(root), head);

      // One with no commit yet counts by every file it lists, staged or not.
      const fresh = join(root, "vendor/fresh");
      put(root, "vendor/fresh/y.ts", "y1");
      git(fresh, "init", "-q");
      const unstaged = dclOneIdentity(root);
      assert.notEqual(unstaged, head);
      git(fresh, "add", ".");
      const staged = dclOneIdentity(root);
      assert.equal(staged, unstaged, "staging the same content changes nothing");
      put(root, "vendor/fresh/y.ts", "y2");
      assert.notEqual(dclOneIdentity(root), staged, "a staged file's content counts");
      rmSync(join(fresh, "y.ts"));
      assert.notEqual(dclOneIdentity(root), staged, "a staged file deleted since counts as deleted");
      rmSync(join(root, "vendor"), { recursive: true });

      put(root, "tooling/shop-parity/pages.mts", "p2");
      assert.equal(dclOneIdentity(root), head, "the harness is not a build input");
      assert.ok(!isCommitted(root), "but a promotion needs it committed");
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("refuses to build while apps/sites has an uncommitted env file Vite would read, ignored or not", () => {
    const root = mkdtempSync(join(tmpdir(), "parity-git-"));
    try {
      git(root, "init", "-q");
      put(root, ".gitignore", ".env.local\n");
      put(root, "apps/sites/.env.production", "VITE_X=committed");
      git(root, "add", ".");
      git(root, "commit", "-q", "-m", "init");
      assert.deepEqual(localEnvFiles(root), [], "a committed env file is a source like any other");
      put(root, "apps/sites/.env.local", "VITE_X=1");
      // Refused before anything runs: there is no package to build here.
      assert.throws(() => buildSites(root, join(root, "apps/sites")), /move apps\/sites\/\.env\.local aside before building/);
      rmSync(join(root, "apps/sites/.env.local"));
      // Vite's local production override, which no .gitignore here lists.
      put(root, "apps/sites/.env.production.local", "VITE_X=2");
      assert.throws(() => buildSites(root, join(root, "apps/sites")), /move apps\/sites\/\.env\.production\.local aside before building/);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });

  it("refuses a build stamped with other sources or another Node", () => {
    const sites = mkdtempSync(join(tmpdir(), "parity-sites-"));
    try {
      assert.throws(() => assertFreshBuild("abc", sites), /not one this tool built/);
      mkdirSync(join(sites, "build"));
      writeFileSync(stampFile(sites), JSON.stringify({ sources: "abc+111111111111", node: process.version }));
      assert.throws(() => assertFreshBuild("abc+222222222222", sites), /is of abc\+111111111111, the sources are at abc\+222222222222/);
      assertFreshBuild("abc+111111111111", sites);
      writeFileSync(stampFile(sites), JSON.stringify({ sources: "abc", node: "v0.0.0" }));
      assert.throws(() => assertFreshBuild("abc", sites), /built by Node v0\.0\.0/);
    } finally {
      rmSync(sites, { recursive: true, force: true });
    }
  });
});

describe("referencePin", () => {
  const pin = { repository: "https://github.com/decentraland/shop.git", revision: "3a322cf97490b6ae4ffcfac1fbe9946d5e3a6279", date: "2026-10-30", app: "app", buildEnv: { VITE_DCL_DEFAULT_ENV: "prod" } };
  const read = (over: Record<string, unknown>) => {
    const dir = mkdtempSync(join(tmpdir(), "parity-pin-"));
    try {
      writeFileSync(join(dir, "reference.json"), JSON.stringify({ ...pin, ...over }));
      return referencePin(join(dir, "reference.json"));
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  };
  it("accepts the committed pin and refuses what would reach git or rmSync unchecked", () => {
    assert.deepEqual(read({}), pin);
    assert.throws(() => read({ revision: "main" }), /40-hex/);
    assert.throws(() => read({ revision: "../../x" }), /40-hex/);
    assert.throws(() => read({ repository: "--upload-pack=x" }), /repository/);
    assert.throws(() => read({ app: "../outside" }), /relative path/);
    assert.throws(() => read({ buildEnv: { NODE_OPTIONS: "--require x" } }), /VITE_/);
  });
});

describe("network guards", () => {
  it("tells a browser's request from the preview's server-side one", () => {
    assert.ok(fromBrowser({ "sec-fetch-site": "cross-site", "sec-fetch-mode": "no-cors" }));
    assert.ok(fromBrowser({ origin: "https://example.com" }));
    // What Node's fetch sends: no site, no origin.
    assert.ok(!fromBrowser({ "sec-fetch-mode": "cors", accept: "*/*", "user-agent": "node" }));
  });

  it("has the preview serve its own pages and refuse other sites and rebound names", () => {
    const base = new URL("http://127.0.0.1:5178");
    const own = { host: "127.0.0.1:5178" };
    assert.ok(!foreignRequest({ ...own, "sec-fetch-site": "none" }, base), "the harness's navigation");
    assert.ok(!foreignRequest({ ...own, "sec-fetch-site": "same-origin", origin: "http://127.0.0.1:5178" }, base), "the page's own POST");
    assert.ok(!foreignRequest(own, base), "the capture's server-side calls");
    assert.ok(foreignRequest({ ...own, "sec-fetch-site": "cross-site" }, base));
    assert.ok(foreignRequest({ ...own, "sec-fetch-site": "same-site" }, base));
    assert.ok(foreignRequest({ ...own, origin: "http://127.0.0.1:5176" }, base));
    // A page on evil.example:5178 rebound to 127.0.0.1: same-origin to itself, but its Host names it.
    assert.ok(foreignRequest({ host: "evil.example:5178", "sec-fetch-site": "same-origin" }, base));
    assert.ok(foreignRequest({ host: "localhost:5178" }, base));
    assert.ok(foreignRequest({}, base));
  });

  it("has the replay answer its own loopback name only, so a rebound page is refused", async () => {
    const probe = createServer();
    await new Promise<void>((r) => probe.listen(0, "127.0.0.1", r));
    const port = (probe.address() as { port: number }).port;
    await new Promise((r) => probe.close(r));
    const dir = mkdtempSync(join(tmpdir(), "parity-"));
    const replay = await startReplay(new Recording(dir), port);
    const status = (host: string) =>
      new Promise<number>((resolve, reject) => {
        const req = request({ host: "127.0.0.1", port, path: "/lambdas/x", headers: { host } }, (res) => {
          res.resume();
          resolve(res.statusCode ?? 0);
        });
        req.on("error", reject);
        req.end();
      });
    try {
      assert.equal(await status(`evil.example:${port}`), 403);
      assert.equal(await status(`localhost:${port}`), 403);
      // Its own name reaches the recording, which has nothing for this path.
      assert.equal(await status(`127.0.0.1:${port}`), 404);
    } finally {
      await new Promise((r) => replay.close(r));
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("lets loopback through and refuses any other host", async () => {
    const server = createServer((_req, res) => res.end("ok"));
    await new Promise<void>((r) => server.listen(0, "127.0.0.1", r));
    const port = (server.address() as { port: number }).port;
    const blocked: string[] = [];
    const lift = guardLoopback((to) => blocked.push(to));
    try {
      assert.equal(await (await fetch(`http://127.0.0.1:${port}/`)).text(), "ok");
      await assert.rejects(fetch("http://example.invalid/"));
      assert.deepEqual(blocked, ["example.invalid:80"]);
    } finally {
      lift();
      server.close();
    }
  });
});
