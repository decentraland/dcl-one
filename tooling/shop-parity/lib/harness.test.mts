import assert from "node:assert/strict";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import type { Collected, Node } from "./collect.mts";
import { decodePng, encodePng, imageSize, placeholderPng, type Rgba } from "./png.mts";
import { pixelDiff } from "./pixeldiff.mts";
import { Recording } from "./recording.mts";
import { classifyMiss, classifyUnknown, isIdentical } from "./run.mts";
import { imageSeed } from "./servers.mts";
import { canonicalQuery, fromNodePath, fromShopApi } from "./services.mts";
import { compareStructure, coverByRegions, line } from "./structure.mts";

const solid = (w: number, h: number, rgb: [number, number, number]): Rgba => {
  const data = new Uint8Array(w * h * 4);
  for (let i = 0; i < w * h; i++) data.set([...rgb, 255], i * 4);
  return { width: w, height: h, data };
};

describe("png", () => {
  it("round-trips RGBA and reads its size", () => {
    const img = solid(7, 5, [10, 200, 30]);
    img.data.set([1, 2, 3, 4], 0);
    const png = encodePng(img);
    assert.deepEqual(imageSize(png), { width: 7, height: 5 });
    assert.deepEqual(decodePng(png), img);
    assert.deepEqual(imageSize(placeholderPng("seed", 4000, 1000)), { width: 1024, height: 256 });
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
    assert.equal(pixelDiff(a, b, { masks: [{ x: 8, y: 8, w: 4, h: 4 }] }).diff, 0);
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
  const miss = { service: "peer", method: "POST", path: "/content/entities/active", query: "", side: "dcl-one", request: { pointers: ["urn:decentraland:off-chain:base-avatars:basemale"] } } as const;
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
