import assert from "node:assert/strict";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { Scrubber, unrecognised } from "./anonymize.mts";
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

describe("Scrubber", () => {
  const real = "0x1234567890abcdef1234567890abcdef12345678";
  const kept = "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd";
  it("maps an address the same way everywhere, keeps constants and the zero address, and is idempotent", () => {
    const s = new Scrubber([kept]);
    const once = s.string(`${real} ${real.toUpperCase().replace("0X", "0x")} ${kept} 0x0000000000000000000000000000000000000000`);
    const fake = s.address(real);
    assert.match(fake, /^0xfa4e0{35}1$/);
    assert.equal(once, `${fake} ${fake} ${kept} 0x0000000000000000000000000000000000000000`);
    assert.equal(s.string(once), once);
  });
  it("replaces a signature with same-length filler but leaves ABI numbers alone in RPC data", () => {
    const s = new Scrubber([]);
    const sig = `0x${"ab".repeat(65)}`;
    assert.equal(s.string(sig).length, sig.length);
    assert.notEqual(s.string(sig), sig);
    s.abi = true;
    const word = "0".repeat(56) + "0d3c9b00";
    assert.equal(s.string(`0x${word}`), `0x${word}`);
    const padded = `0x70a08231${"0".repeat(24)}${real.slice(2)}`;
    assert.equal(s.string(padded), `0x70a08231${"0".repeat(24)}${s.address(real).slice(2)}`);
  });
  it("renames people wherever their name appears as a word, and rewrites their bio in place", () => {
    const s = new Scrubber([]);
    s.learn({ avatars: [{ ethAddress: real, name: "Kenji", description: "I make hats" }] });
    const out = s.json({ avatars: [{ ethAddress: real, name: "Kenji", description: "I make hats", email: "kenji@example.net" }] }) as { avatars: Record<string, string>[] };
    const p = out.avatars[0]!;
    assert.notEqual(p.name, "Kenji");
    assert.equal(p.name!.length, 5);
    assert.notEqual(p.description, "I make hats");
    assert.match(p.email!, /@example\.org$/);
    assert.equal(s.string("Kenji Jacket by KENJI"), `${p.name} Jacket by ${p.name}`);
    assert.equal(s.string("Kenjiro"), "Kenjiro");
  });

  it("replaces content ids, NAMEs, near-addresses and birth dates, and flags whatever it never recognised", () => {
    const s = new Scrubber([]);
    const cid = `bafkrei${"q".repeat(52)}`;
    const v0 = `Qm${"W".repeat(44)}`;
    const out = s.json({
      avatars: [{ ethAddress: real, birthDate: 631152000, avatar: { snapshots: { face256: `https://profile-images.decentraland.org/entities/${cid}/face.png` } } }],
      links: "https://decentraland.org/play/world/kenji.dcl.eth",
      flags: [`0x${"ab".repeat(19)}`, `0x${"cd".repeat(19)}e`, v0],
    }) as { avatars: { birthDate: number; avatar: { snapshots: { face256: string } } }[]; links: string; flags: string[] };
    const face = out.avatars[0]!.avatar.snapshots.face256;
    assert.equal(out.avatars[0]!.birthDate, 0);
    assert.ok(!face.includes(cid));
    assert.equal(face.length, `https://profile-images.decentraland.org/entities/${cid}/face.png`.length);
    assert.equal(s.string(`/content/contents/${cid}`), `/content/contents/${face.split("/")[4]}`);
    assert.ok(!out.links.includes("kenji"));
    assert.match(out.links, /\/world\/[a-z]{5}\.dcl\.eth$/);
    assert.match(out.flags[0]!, /^0xfa4e[0-9a-f]{34}$/);
    assert.match(out.flags[1]!, /^0xfa4e[0-9a-f]{35}$/);
    assert.notEqual(out.flags[2], v0);
    assert.deepEqual(unrecognised("x.json", JSON.stringify(out), s), []);
    const left = unrecognised("x.json", JSON.stringify({ face: `/entities/${cid}/face.png`, birthDate: 631152000, owner: "1234567890abcdef1234567890abcdef12345678", who: "kenji.dcl.eth" }), s);
    assert.deepEqual(left.map((l) => l.split(" ")[1]), ["content", "NAME", "bare", "birth"]);
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

describe("Scrubber, beyond ASCII and plain text", () => {
  it("gives names and text in any script an ASCII stand-in of the same shape", () => {
    const s = new Scrubber([]);
    assert.match(s.name("Дмитрий"), /^[A-Z][a-z]{6}$/);
    assert.match(s.name("José María"), /^[A-Z][a-z]{3} [A-Z][a-z]{4}$/);
    assert.match(s.text("東京の猫 2"), /^[a-z]{4} [0-9]$/);
    assert.match(s.text("مرحبا"), /^[a-z]{5}$/);
  });
  it("redoes a stand-in from an earlier map that still carries the original's letters", () => {
    const s = new Scrubber([], { addresses: {}, hex: {}, names: { "José": "Basé" }, texts: { "abc": "abc" }, emails: {}, cids: {} });
    assert.deepEqual(s.map.names, {});
    assert.deepEqual(s.map.texts, {});
    assert.match(s.name("José"), /^[A-Z][a-z]{3}$/);
  });
  it("replaces ENS names, URL-encoded emails and the community, world or account a first-party link names", () => {
    const s = new Scrubber([]);
    assert.doesNotMatch(s.string("see vitalik.eth"), /vitalik/);
    assert.equal(s.string("dcl.eth"), "dcl.eth");
    assert.match(s.string("?mail=jane.doe%40example.net"), /^\?mail=person-\d+%40example\.org$/);
    const community = "https://decentraland.org/social/communities/3f9b2c1e-8a7d-4e6f-9b0a-1c2d3e4f5a6b";
    assert.match(s.link(community), /\/communities\/fa4e0000-0000-4000-8000-[0-9a-f]{12}$/);
    const account = s.link("https://decentraland.org/profile/accounts/0x1234567890abcdef1234567890abcdef12345678");
    assert.match(account, /\/profile\/accounts\/0xfa4e[0-9a-f]{36}$/);
    assert.match(s.link("https://decentraland.org/places/world/Mi%20Mundo"), /\/world\/[A-Z][a-z]+%20[A-Z][a-z]+$/);
  });
  it("lists what it cannot place in JSON-RPC data instead of passing it", () => {
    const s = new Scrubber([]);
    s.abi = true;
    s.string(`0x${"ab".repeat(50)}`);
    assert.equal(s.unaligned.length, 1);
  });
  it("flags ENS names, encoded emails and first-party links it never replaced", () => {
    const s = new Scrubber([]);
    const left = unrecognised("x.json", JSON.stringify({ who: "vitalik.eth", q: "a.b%40mail.net", url: "https://decentraland.org/social/communities/3f9b2c1e-8a7d-4e6f-9b0a-1c2d3e4f5a6b" }), s);
    assert.deepEqual(left.map((l) => l.split(" ").slice(1, 3).join(" ")), ["ENS name", "email a.b%40…", "account, community"]);
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

describe("Scrubber stand-ins and edge identifiers", () => {
  it("never gives two originals of the same shape one stand-in, nor an original itself", () => {
    const s = new Scrubber([], { addresses: {}, hex: {}, names: { "Ana": "Bér" }, texts: { "I’m a bio": "B’k e bul" }, emails: {}, cids: {} });
    assert.deepEqual(s.map.texts, { "I’m a bio": "B’k e bul" });
    const names = Array.from({ length: 60 }, (_, i) => `Name${String.fromCharCode(97 + (i % 26))}${String.fromCharCode(97 + Math.floor(i / 26))}`);
    assert.equal(new Set(names.map((n) => s.name(n))).size, 60);
    for (const digits of ["1234", "5678", "9"]) assert.notEqual(s.name(digits), digits);
    assert.notEqual(s.text("2468"), "2468");
  });
  it("replaces encoded emails with escapes, and ENS names in any script or with subdomains", () => {
    const s = new Scrubber([]);
    assert.match(s.string("jane.doe%2Bdcl%40example-mail.com"), /^person-\d+%40example\.org$/);
    for (const ens of ["café.eth", "🦊🦊🦊.eth", "sub.janedoe.eth"]) {
      const out = s.string(`by ${ens}`);
      assert.match(out, /^by [a-z.]+\.eth$/, ens);
      assert.deepEqual(unrecognised("x.json", JSON.stringify({ by: out }), s), []);
      assert.equal(unrecognised("x.json", JSON.stringify({ by: ens }), s).length, 1, ens);
    }
    assert.doesNotMatch(s.link("https://decentraland.org/play/world/caf%C3%A9.eth"), /caf|%C3/);
  });
  it("reads a name after a JSON escape as a name, and links whose first segment is the kind", () => {
    const s = new Scrubber([]);
    assert.deepEqual(unrecognised("x.json", JSON.stringify({ d: s.string("New drop by\nvitalik.eth\tbar.dcl.eth") }), s), []);
    assert.equal(unrecognised("x.json", JSON.stringify({ u: "https://decentraland.org/worlds/janedoe" }), s).length, 1);
    assert.match(s.link("https://decentraland.org/worlds/janedoe"), /\/worlds\/[a-z]{7}$/);
  });
});

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
