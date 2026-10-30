// Anonymize a raw recording before it enters git: wallet and contract addresses become
// 0xfa4e…-prefixed sequence numbers, signatures and salts same-length filler, content ids (CIDs, which
// a public Catalyst resolves back to the profile they name) same-length stand-ins, people (profile
// names, bios, emails, links, NAMEs, birth dates) same-shape pseudo-text or zero. Product text (item, collection and outfit
// names, item descriptions) is catalogue content and stays. Addresses the two apps bundle as
// constants (protocol contracts) and the zero address stay, so the apps' own comparisons hold.
//
// The real→fake map is written next to the raw recording (state/, gitignored) so missing answers
// can be filled later; it never leaves state/.
import { createHash } from "node:crypto";
import { mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { extname, join } from "node:path";
import { STATE } from "./paths.mts";
import { entryKey, type Recording } from "./recording.mts";
import { canonicalQuery, ORIGINS } from "./services.mts";

export const ASSETS = join(STATE, "assets");

const ADDRESS = /(?<![0-9a-fA-F])0[xX][0-9a-fA-F]{40}(?![0-9a-fA-F])/g;
/** 38 or 39 digits: a wallet with a typo in hand-kept lists, trivially completed. */
const NEAR_ADDRESS = /(?<![0-9a-fA-F])0[xX][0-9a-fA-F]{38,39}(?![0-9a-fA-F])/g;
const LONG_HEX = /0[xX][0-9a-fA-F]{41,}/g;
const EMAIL = /[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/g;
/** An email inside a query string or path (`%40` for `@`). */
const EMAIL_ENCODED = /(?:[A-Za-z0-9._+-]|%[0-9A-Fa-f]{2})+?%40[A-Za-z0-9.-]+\.[A-Za-z]{2,}/g;
/** IPFS content ids, v1 (base32) and v0: entity and file ids on a Catalyst. */
const CID = /(?<![A-Za-z0-9])(?:baf[a-z2-7]{50,}|Qm[1-9A-HJ-NP-Za-km-z]{44})(?![A-Za-z0-9])/g;
/** A NAME (`label.dcl.eth`): its owner is public on-chain. */
const DCL_NAME = /(?<![A-Za-z0-9-])([A-Za-z0-9-]{2,})(\.dcl\.eth)(?![A-Za-z0-9])/gi;
/** An ENS label: ASCII letters, digits and hyphens, or any non-ASCII character but punctuation and spaces (emoji included). */
const ENS_LABEL = String.raw`(?:[A-Za-z0-9\-]|[^\x00-\x7f\p{P}\p{Z}\p{Cc}])`;
/** Any other ENS name (`label.eth`, `sub.label.eth`), owned on-chain just the same. */
const ENS_NAME = new RegExp(String.raw`(?<![.]|${ENS_LABEL})((?:${ENS_LABEL}+\.)*${ENS_LABEL}+)(\.eth)(?![A-Za-z0-9])`, "giu");
const JWT = /eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]*/g;
/** A bare 40-digit hex word outside ABI data: an address without its prefix. */
const BARE_HEX40 = /(?<![0-9a-zA-Z])[0-9a-fA-F]{40}(?![0-9a-zA-Z])/g;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
/** The path segment after one of these on a first-party page names a person, a community or a world. */
const PERSONAL_KINDS = new Set(["accounts", "communities", "profile", "world", "worlds"]);
const PERSONAL_URL = /https?:\/\/(?:[a-z0-9-]+\.)*decentraland\.(?:org|zone|today)(?:\/[^"\s]*?)?\/(?:accounts|communities|profile|world|worlds)\/([^/?#"\s]+)/gi;
/** What a stand-in replaces in a person's text: any letter, mark, digit or symbol, in any script. */
const SHAPED = /[\p{L}\p{M}\p{N}\p{S}]/u;
const ZERO = "0x0000000000000000000000000000000000000000";

/** Keys whose value names or describes a person wherever they appear. */
const PERSON_KEYS = new Set([
  "creatorName",
  "ownerName",
  "sellerName",
  "buyerName",
  "authorName",
  "displayName",
  "username",
  "userName",
  "nickname",
  "unclaimedName",
]);
/** Inside a profile (an object carrying ethAddress/userId), these describe the person too. */
const PROFILE_KEYS = new Set(["name", "description", "email", "links", "country", "employmentstatus", "gender", "pronouns", "relationshipstatus", "sexualorientation", "language", "profession", "birthdate", "realname", "hobbies"]);
/** Profile keys whose value is dropped whatever its type (a number here is a date). */
const ZEROED_KEYS = new Set(["birthdate"]);
const SOCIAL_KEYS = new Set(["twitter", "discord", "website", "instagram", "facebook", "youtube", "tiktok", "telegram", "x"]);

const CONSONANTS = "bcdfghklmnprstvz";
const VOWELS = "aeiou";

function decoded(s: string): string {
  try {
    return decodeURIComponent(s);
  } catch {
    return s;
  }
}

export type ScrubMap = {
  addresses: Record<string, string>;
  hex: Record<string, string>;
  /** People's names: replaced wherever they appear as a word. */
  names: Record<string, string>;
  /** Free text about a person (bios, profile attributes): replaced where it stands only. */
  texts: Record<string, string>;
  emails: Record<string, string>;
  cids: Record<string, string>;
};

export class Scrubber {
  readonly keep: Set<string>;
  readonly map: ScrubMap;
  private counters = { address: 0, hex: 0, name: 0, email: 0, link: 0, cid: 0 };
  /** Every stand-in already handed out, so scrubbing scrubbed text is a no-op. */
  private fakes = new Set<string>();

  constructor(keep: Iterable<string>, previous?: ScrubMap) {
    this.keep = new Set([...keep].map((a) => a.toLowerCase()));
    this.keep.add(ZERO);
    this.map = { addresses: {}, hex: {}, names: {}, texts: {}, emails: {}, cids: {}, ...(previous ?? {}) };
    // A stand-in from before every script was shaped may still carry the original's letters: redo it.
    for (const table of [this.map.names, this.map.texts]) for (const [o, f] of Object.entries(table)) if (f === o || [...f].some((c) => c > "\x7f" && SHAPED.test(c))) delete table[o];
    this.counters.address = Object.keys(this.map.addresses).length;
    this.counters.hex = Object.keys(this.map.hex).length;
    this.counters.name = Object.keys(this.map.names).length;
    this.counters.email = Object.keys(this.map.emails).length;
    this.counters.cid = Object.keys(this.map.cids).length;
    for (const m of [this.map.addresses, this.map.hex, this.map.names, this.map.texts, this.map.emails, this.map.cids]) for (const f of Object.values(m)) this.fakes.add(f);
  }

  isFake(v: string): boolean {
    return this.fakes.has(v) || this.fakes.has(v.toLowerCase());
  }

  /** Originals replaced in this run: what the leak check looks for. */
  readonly used = new Set<string>();

  private give(table: Record<string, string>, original: string, make: () => string): string {
    this.used.add(original);
    if (!table[original]) {
      table[original] = make();
      this.fakes.add(table[original]!);
    }
    return table[original]!;
  }

  address(a: string): string {
    const low = a.toLowerCase();
    if (this.keep.has(low)) return a;
    if (this.fakes.has(low)) return low;
    return this.give(this.map.addresses, low, () => `0xfa4e${(++this.counters.address).toString(16).padStart(36, "0")}`);
  }

  hex(h: string): string {
    const low = h.toLowerCase();
    if (this.fakes.has(low)) return low;
    return this.give(this.map.hex, low, () => {
      const n = (++this.counters.hex).toString(16);
      return `0x${`fa4e${"0".repeat(Math.max(0, low.length - 6 - n.length))}${n}`.slice(0, low.length - 2)}`;
    });
  }

  /** A wallet with a typo: a same-length stand-in in the address sequence's form. */
  nearAddress(a: string): string {
    const low = a.toLowerCase();
    if (this.fakes.has(low)) return low;
    return this.give(this.map.addresses, low, () => `0xfa4e${(++this.counters.address).toString(16).padStart(low.length - 6, "0")}`);
  }

  /** A content id: same version prefix and length, a sequence number in the content id's alphabet. */
  cid(c: string): string {
    if (this.fakes.has(c)) return c;
    return this.give(this.map.cids, c, () => {
      const n = (++this.counters.cid).toString(32).replace(/[0-9]/g, (d) => "abcdefghij"[Number(d)]!);
      const head = c.startsWith("Qm") ? "QmFa4e" : `${c.slice(0, 7)}fa4e`;
      return `${head}${"z".repeat(c.length - head.length - n.length)}${n}`;
    });
  }

  /** A NAME's label: same shape, replaced where it stands only (a label can be a common word). */
  dclName(label: string): string {
    return this.text(label.toLowerCase());
  }

  /** A UUID in a link to a community or a world: the same form, a sequence number. */
  uuid(u: string): string {
    const low = u.toLowerCase();
    if (this.fakes.has(low)) return low;
    return this.give(this.map.hex, low, () => `fa4e0000-0000-4000-8000-${(++this.counters.hex).toString(16).padStart(12, "0")}`);
  }

  /**
   * A pronounceable ASCII stand-in, one character per character of the original, upper case where the
   * original is. Letters, marks and symbols of any script become letters and digits become digits;
   * spaces and punctuation stay.
   */
  private shape(original: string, seed: number): string {
    let n = seed;
    let out = "";
    let i = 0;
    for (const c of original) {
      if (/\p{N}/u.test(c)) out += String((n + i) % 10);
      else if (SHAPED.test(c)) {
        const pool = out.length % 2 === 0 ? CONSONANTS : VOWELS;
        const letter = pool[(n * 7 + i * 13) % pool.length]!;
        out += c !== c.toLowerCase() ? letter.toUpperCase() : letter;
        n = (n * 31 + 17) % 9973;
      } else out += c;
      i++;
    }
    return out;
  }

  /**
   * A stand-in nobody else has that differs from the original: two people of the same shape must not
   * share one (their requests would collide), and a digits-only name can come out as itself.
   */
  private fresh(original: string, seed: number): string {
    let out = this.shape(original, seed);
    for (let k = 1; k < 64 && (out === original || this.fakes.has(out)); k++) out = this.shape(original, seed + k * 7919);
    return out;
  }

  name(original: string): string {
    if (!original || this.fakes.has(original)) return original;
    return this.give(this.map.names, original, () => this.fresh(original, ++this.counters.name));
  }

  /** A person's free text: same shape, replaced in place only. */
  text(original: string): string {
    if (!original || this.fakes.has(original)) return original;
    return this.give(this.map.texts, original, () => this.fresh(original, 5000 + Object.keys(this.map.texts).length));
  }

  email(e: string): string {
    if (this.fakes.has(e) || /@example\.(org|com)$/.test(e)) return e;
    return this.give(this.map.emails, e, () => `person-${++this.counters.email}@example.org`);
  }

  private namePattern: RegExp | null = null;
  private patternSize = -1;

  /** Whole-word occurrences of a known person's name anywhere (an item named after its creator). */
  private names(s: string): string {
    const known = Object.keys(this.map.names).filter((n) => n.length >= 3);
    if (!known.length) return s;
    if (this.patternSize !== known.length) {
      const alts = known.sort((a, b) => b.length - a.length).map((n) => n.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
      this.namePattern = new RegExp(`(?<![\\p{L}\\p{N}])(?:${alts.join("|")})(?![\\p{L}\\p{N}])`, "giu");
      this.patternSize = known.length;
    }
    const lower = new Map(known.map((n) => [n.toLowerCase(), this.map.names[n]!]));
    return s.replace(this.namePattern!, (m) => lower.get(m.toLowerCase()) ?? m);
  }

  /** Scrubbing JSON-RPC: hex is ABI data (calldata, return values), not a signature or a hash. */
  abi = false;
  /** ABI data that is not whole words: nothing tells where an address would sit, so the write is refused. */
  readonly unaligned: string[] = [];

  /** ABI words: a left-padded address is mapped in place; numbers and other words are left alone. */
  private abiData(h: string): string {
    const body = h.slice(2);
    const head = body.length % 64 === 8 ? 8 : body.length % 64 === 0 ? 0 : -1;
    if (head < 0) {
      this.unaligned.push(`${h.slice(0, 10)}… (${body.length} digits)`);
      return h;
    }
    let out = `0x${body.slice(0, head)}`;
    for (let i = head; i < body.length; i += 64) {
      const w = body.slice(i, i + 64);
      const m = w.match(/^0{24}([0-9a-fA-F]{40})$/);
      out += m && !/^0{9}/.test(m[1]!) ? `${"0".repeat(24)}${this.address(`0x${m[1]}`).slice(2)}` : w;
    }
    return out;
  }

  string(s: string): string {
    return this.names(
      s
        .replace(LONG_HEX, (h) => (this.abi ? this.abiData(h) : this.hex(h)))
        .replace(ADDRESS, (a) => this.address(a))
        .replace(NEAR_ADDRESS, (a) => this.nearAddress(a))
        .replace(CID, (c) => this.cid(c))
        .replace(DCL_NAME, (_m, label: string, tld: string) => `${this.dclName(label)}${tld.toLowerCase()}`)
        .replace(ENS_NAME, (_m, labels: string, tld: string) => `${labels.split(".").map((l) => (l.toLowerCase() === "dcl" || this.isFake(l) ? l : this.dclName(l))).join(".")}${tld.toLowerCase()}`)
        .replace(EMAIL, (e) => this.email(e))
        .replace(EMAIL_ENCODED, (e) => encodeURIComponent(this.email(decodeURIComponent(e)))),
    );
  }

  /** First pass: register every person's name so later text mentioning it is caught too. */
  learn(v: unknown, key?: string, inProfile = false): void {
    if (typeof v === "string") {
      if (key && (PERSON_KEYS.has(key) || (inProfile && key === "name"))) this.name(v);
      return;
    }
    if (Array.isArray(v)) return v.forEach((x) => this.learn(x, key, inProfile));
    if (v && typeof v === "object") {
      const o = v as Record<string, unknown>;
      const profile = inProfile || "ethAddress" in o || ("userId" in o && "avatar" in o);
      for (const [k, x] of Object.entries(o)) this.learn(x, k, profile);
    }
  }

  /** The segment naming a person, a community or a world on a first-party page. */
  private personalSegment(segment: string): string {
    if (this.isFake(segment)) return segment;
    const plain = decoded(segment);
    // An address, a NAME or a person already known keeps the stand-in it has everywhere else.
    const named = this.string(plain);
    if (named !== plain || /\.eth$/i.test(plain)) return named === plain ? segment : encodeURIComponent(named);
    if (UUID.test(segment)) return this.uuid(segment);
    return encodeURIComponent(this.text(plain));
  }

  /**
   * A URL string: addresses and emails replaced; on a first-party page the account, community or world
   * it points at too; a link off the known content hosts becomes example.org.
   */
  link(s: string): string {
    if (!/^https?:\/\//.test(s)) return this.string(s);
    try {
      const u = new URL(s);
      if (/(^|\.)decentraland\.(org|zone|today)$/.test(u.hostname)) {
        // The segments first, decoded (a NAME in another script is percent-encoded in a path), then the rest.
        const cut = s.search(/[?#]/);
        const parts = (cut < 0 ? s : s.slice(0, cut)).split("/");
        // parts: scheme, "", host, then the path's segments.
        for (let i = 4; i < parts.length; i++) {
          if (parts[i] && PERSONAL_KINDS.has(parts[i - 1]!.toLowerCase()) && !PERSONAL_KINDS.has(parts[i]!.toLowerCase())) parts[i] = this.personalSegment(parts[i]!);
        }
        return this.string(parts.join("/") + (cut < 0 ? "" : s.slice(cut)));
      }
    } catch {
      return this.string(s);
    }
    return `https://example.org/link-${++this.counters.link}`;
  }

  json(v: unknown, key?: string, inProfile = false): unknown {
    if (typeof v === "string") {
      if (key && (PERSON_KEYS.has(key) || (inProfile && key === "name"))) return this.name(v);
      if (inProfile && key === "description") return this.text(v);
      if (key && SOCIAL_KEYS.has(key)) return v ? this.link(v.startsWith("http") ? v : `https://example.org/${v}`) : v;
      if (key === "email") return v ? this.email(v) : v;
      if (key === "url" && inProfile) return this.link(v);
      return this.string(v);
    }
    if (Array.isArray(v)) return v.map((x) => this.json(x, key, inProfile));
    if (v && typeof v === "object") {
      const o = v as Record<string, unknown>;
      const profile = inProfile || "ethAddress" in o || ("userId" in o && "avatar" in o);
      const out: Record<string, unknown> = {};
      for (const [k, x] of Object.entries(o)) {
        const kk = this.string(k);
        if (profile && k === "links" && Array.isArray(x)) {
          out[kk] = x.map((l) => (l && typeof l === "object" ? { ...(l as object), title: this.text(String((l as Record<string, unknown>).title ?? "")), url: this.link(String((l as Record<string, unknown>).url ?? "")) } : l));
        } else if (profile && ZEROED_KEYS.has(k.toLowerCase())) {
          out[kk] = typeof x === "number" ? 0 : typeof x === "string" ? "" : x === null ? null : undefined;
        } else if (profile && PROFILE_KEYS.has(k.toLowerCase()) && typeof x === "string" && k !== "name" && k !== "description" && k !== "email") {
          out[kk] = x ? this.text(x) : x;
        } else out[kk] = this.json(x, k, profile);
      }
      return out;
    }
    return v;
  }
}

/** Every address either app ships in its own code: public protocol constants, never user data. */
export function bundledAddresses(roots: string[]): Set<string> {
  const found = new Set<string>();
  const walk = (p: string) => {
    const st = statSync(p);
    if (st.isDirectory()) {
      if (/node_modules|\.git$|fixtures?$/.test(p)) return;
      for (const n of readdirSync(p)) walk(join(p, n));
    } else if ([".js", ".mjs", ".ts", ".tsx"].includes(extname(p)) && !/\.(test|spec|stories)\.[jt]sx?$/.test(p) && st.size < 20_000_000) {
      for (const m of readFileSync(p, "utf8").matchAll(ADDRESS)) found.add(m[0].toLowerCase());
    }
  };
  for (const r of roots) walk(r);
  return found;
}

/**
 * raw recording (state/) → committed recording. Fonts are third-party binaries, not data: the
 * entry keeps their URL and sha256 and the bytes stay in state/shop-parity/assets/, refetched
 * and verified when missing.
 */
export function anonymizeRecording(raw: Recording, out: Recording, keep: Set<string>, previous?: ScrubMap): { map: ScrubMap; leaks: string[] } {
  const scrub = new Scrubber(keep, previous);
  for (const e of raw.entries.values()) {
    if (!/json/.test(e.contentType) || e.external) continue;
    try {
      scrub.learn(JSON.parse(raw.body(e).toString("utf8")));
    } catch {
      // not JSON after all
    }
  }
  out.entries.clear();
  out.images.clear();
  for (const e of [...raw.entries.values()].sort((a, b) => (a.key < b.key ? -1 : 1))) {
    scrub.abi = e.service === "rpc";
    const path = scrub.string(e.path);
    const query = canonicalQuery(scrub.string(e.query));
    const request = e.request === undefined ? undefined : scrub.json(e.request);
    const key = entryKey(e.method, e.service, path, query, request);
    const body = raw.body(e);
    const base = { key, method: e.method, service: e.service, path, query, ...(request === undefined ? {} : { request }), status: e.status, contentType: e.contentType };
    if (/font|woff|ttf|otf/.test(e.contentType)) {
      const sha256 = createHash("sha256").update(body).digest("hex");
      mkdirSync(ASSETS, { recursive: true });
      writeFileSync(join(ASSETS, sha256), body);
      out.putExternal({ ...base, external: { url: `${ORIGINS[e.service]}${e.path}${e.query ? `?${e.query}` : ""}`, sha256 } });
      continue;
    }
    let text: string;
    if (/json/.test(e.contentType)) {
      try {
        text = `${JSON.stringify(scrub.json(JSON.parse(body.toString("utf8"))), null, 1)}\n`;
      } catch {
        text = scrub.string(body.toString("utf8"));
      }
    } else if (/^text\/|javascript|xml|html|css/.test(e.contentType) || body.length === 0) {
      text = scrub.string(body.toString("utf8"));
    } else {
      continue;
    }
    out.putBody(base, Buffer.from(text));
  }
  scrub.abi = false;
  for (const [seed, size] of raw.images) out.images.set(scrub.string(seed), size);
  const targets = Object.fromEntries(Object.entries(raw.manifest?.targets ?? {}).map(([k, v]) => [k, scrub.string(v)]));
  out.save({ ...(raw.manifest ?? {}), targets, anonymization: ANONYMIZATION });

  // Nothing real may survive: scan every written text for every original value.
  const used = (table: Record<string, string>) => Object.keys(table).filter((k) => scrub.used.has(k));
  const originals = [...used(scrub.map.addresses), ...used(scrub.map.hex), ...used(scrub.map.emails)];
  // A name that is another person's stand-in is found in the output legitimately; its own stand-in equal to
  // itself replaced nothing and is refused below.
  const people = used(scrub.map.names).filter((n) => n.length >= 3 && !Object.entries(scrub.map.names).some(([o, f]) => f === n && o !== n));
  const prose = used(scrub.map.texts).filter((t) => t.length >= 12);
  const word = (n: string) => new RegExp(`(?<![\\p{L}\\p{N}])${n.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}(?![\\p{L}\\p{N}])`, "iu");
  const unchanged = [scrub.map.names, scrub.map.texts].flatMap((t) => Object.entries(t).filter(([o, f]) => scrub.used.has(o) && f === o && SHAPED.test(o)));
  const leaks: string[] = [
    ...unchanged.map(([o]) => `a person's name or text (${o.length} chars) came out unchanged`),
    ...scrub.unaligned.map((h) => `rpc: ABI data not in whole words, ${h}`),
  ];
  const files = ["index.json", "images.json", "manifest.json", ...readdirSync(join(out.dir, "bodies")).map((n) => join("bodies", n))];
  for (const f of files) {
    const text = readFileSync(join(out.dir, f), "utf8");
    const low = text.toLowerCase();
    for (const o of originals) if (low.includes(o.toLowerCase())) leaks.push(`${f}: ${o.slice(0, 6)}…`);
    for (const n of people) if (word(n).test(text)) leaks.push(`${f}: a person's name (${n.length} chars)`);
    for (const t of prose) if (text.includes(JSON.stringify(t).slice(1, -1))) leaks.push(`${f}: a person's text (${t.length} chars)`);
    leaks.push(...unrecognised(f, text, scrub, f === "manifest.json" ? [raw.manifest?.reference, raw.manifest?.dclOne] : []));
  }
  return { map: scrub.map, leaks };
}

/**
 * What the scrubber never recognised can still identify someone: every identifier-shaped value left in
 * the output must be a stand-in, a bundled constant or (in the manifest) a commit id.
 */
export function unrecognised(file: string, raw: string, scrub: Scrubber, commits: Array<string | undefined> = []): string[] {
  const fake = (v: string) => scrub.isFake(v) || scrub.keep.has(v.toLowerCase());
  /** An ENS name left as written: any of its labels but `dcl` that is not a stand-in. */
  const ensLeft = (labels: string) => labels.split(".").some((l) => l.toLowerCase() !== "dcl" && !fake(l));
  // A JSON escape (`\n` before a name) would read as part of the name that follows it.
  const text = raw.replace(/\\[nrtbf]/g, " ");
  const out: string[] = [];
  const flag = (kind: string, v: string) => out.push(`${file}: ${kind} ${v.slice(0, 6)}…`);
  for (const [m] of text.matchAll(ADDRESS)) if (!fake(m)) flag("address", m);
  for (const [m] of text.matchAll(NEAR_ADDRESS)) if (!fake(m)) flag("address-like hex", m);
  for (const [m] of text.matchAll(CID)) if (!fake(m)) flag("content id", m);
  for (const [, label] of text.matchAll(DCL_NAME)) if (!fake(label!)) flag("NAME", label!);
  // `label.dcl.eth` is the NAME check's, just above.
  for (const [, labels] of text.matchAll(ENS_NAME)) if (!/^[A-Za-z0-9-]+\.dcl$/i.test(labels!) && ensLeft(labels!)) flag("ENS name", labels!);
  for (const [m] of text.matchAll(EMAIL)) if (!/@example\.(org|com)$/.test(m) && !fake(m)) flag("email", m);
  for (const [m] of text.matchAll(EMAIL_ENCODED)) if (!/%40example\.(org|com)$/.test(m) && !fake(decoded(m))) flag("email", m);
  for (const [, seg] of text.matchAll(PERSONAL_URL)) {
    const plain = decoded(seg!);
    if (PERSONAL_KINDS.has(plain.toLowerCase())) continue;
    if (/\.eth$/i.test(plain) ? ensLeft(plain.slice(0, -4)) : !fake(seg!) && !fake(plain)) flag("account, community or world", seg!);
  }
  for (const [m] of text.matchAll(JWT)) flag("token", m);
  for (const [m] of text.matchAll(BARE_HEX40)) if (!fake(`0x${m}`) && !commits.includes(m.toLowerCase())) flag("bare 40-digit hex", m);
  for (const [m] of text.matchAll(/"birth_?date"\s*:\s*"?[1-9]/gi)) flag("birth date", m);
  return out;
}

export const ANONYMIZATION = "v3: addresses→0xfa4e<seq>, long hex→same-length filler, content ids→same-length stand-ins, people, NAMEs and ENS names→same-shape ASCII pseudo-text (any script), linked accounts, communities and worlds→stand-ins, birth dates→0, emails (plain or URL-encoded)→example.org, off-site links→example.org, fonts external by sha256";
