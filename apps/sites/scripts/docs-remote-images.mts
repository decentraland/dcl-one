// Vendor the images the upstream docs load from other hosts (gen-docs-content.mts), so
// /docs never hotlinks. Each lands once under _remote/<sha256-24>.<ext>, a raster as its WebP
// replacement (docs-images.mts); _remote/sources.json records the URL it came from, the
// original's sha256 and whether a local mirror or the network supplied it.
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";

import { type DocsMedia, rasterKind } from "./docs-images.mts";

const MAX_BYTES = 50 * 1024 * 1024;
const TYPES: Record<string, string> = {
  "image/png": "png", "image/jpeg": "jpg", "image/gif": "gif", "image/webp": "webp", "image/svg+xml": "svg", "image/avif": "avif",
};

export type RemoteEntry = { file: string; sha256: string; bytes: number; from: string; replaced?: true } | { error: string };

function sniff(bytes: Buffer): string | null {
  if (bytes.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]))) return "png";
  if (bytes[0] === 0xff && bytes[1] === 0xd8) return "jpg";
  if (bytes.subarray(0, 4).toString("ascii") === "GIF8") return "gif";
  if (bytes.subarray(0, 4).toString("ascii") === "RIFF" && bytes.subarray(8, 12).toString("ascii") === "WEBP") return "webp";
  if (/^\s*(<\?xml[^>]*>\s*)?<svg\b/i.test(bytes.subarray(0, 512).toString("utf8"))) return "svg";
  return null;
}

/** github.com/<o>/<r>/blob/<ref>/<p> and raw.githubusercontent.com/<o>/<r>/<ref>/<p> -> [raw URL, repo, path]. */
export function githubRaw(url: string): { raw: string; owner: string; repo: string; file: string } | null {
  const m = url.match(/^https?:\/\/(?:github\.com\/([^/]+)\/([^/]+)\/(?:blob|raw)\/[^/]+|raw\.githubusercontent\.com\/([^/]+)\/([^/]+)\/[^/]+)\/([^?#]+)/i);
  if (!m) return null;
  const owner = m[1] ?? m[3], repo = m[2] ?? m[4];
  const ref = url.match(/\/(?:blob|raw)\/([^/]+)\//)?.[1] ?? url.split("/")[5];
  return { raw: `https://raw.githubusercontent.com/${owner}/${repo}/${ref}/${m[5]}`, owner, repo, file: decodeURIComponent(m[5]) };
}

export class RemoteImages {
  readonly entries = new Map<string, RemoteEntry>();
  readonly missing: string[] = [];
  private previous: Record<string, RemoteEntry>;
  private opts: { output: string; mirrors: string[]; media: DocsMedia; base?: string; readOnly?: boolean };

  constructor(opts: { output: string; mirrors: string[]; media: DocsMedia; base?: string; readOnly?: boolean }) {
    this.opts = opts;
    const manifest = path.join(opts.output, "_remote/sources.json");
    this.previous = fs.existsSync(manifest) ? JSON.parse(fs.readFileSync(manifest, "utf8")) : {};
  }

  /** The local /docs URL of a vendored image, or null when it could not be fetched. */
  local(url: string): string | null {
    const entry = this.entries.get(url);
    return entry && "file" in entry ? `${this.opts.base ?? "/docs"}/${entry.file}` : null;
  }

  files(): string[] {
    return [...this.entries.values()].flatMap((e) => ("file" in e ? [e.file] : []));
  }

  async vendor(urls: Iterable<string>): Promise<void> {
    for (const url of [...new Set(urls)].sort()) {
      const prior = this.previous[url];
      if (prior && "file" in prior && fs.existsSync(path.join(this.opts.output, prior.file))
        && (prior.replaced ? this.opts.media.keep(prior.file, prior.from) : !/\.(png|jpe?g|gif)$/i.test(prior.file))) {
        this.entries.set(url, prior);
        continue;
      }
      // A vendored original from before the WebP replacement: convert it instead of refetching.
      if (!this.opts.readOnly && prior && "file" in prior && !prior.replaced && fs.existsSync(path.join(this.opts.output, prior.file))) {
        const stored = this.store(fs.readFileSync(path.join(this.opts.output, prior.file)), prior.from);
        if (stored) { this.entries.set(url, stored); continue; }
      }
      if (this.opts.readOnly) {
        if (prior && "error" in prior) this.entries.set(url, prior);
        else this.missing.push(url);
        continue;
      }
      this.entries.set(url, await this.fetchOne(url));
    }
  }

  private async fetchOne(url: string): Promise<RemoteEntry> {
    const gh = githubRaw(url);
    if (gh && gh.owner.toLowerCase() === "decentraland") {
      for (const root of this.opts.mirrors) {
        const file = path.join(root, gh.repo, gh.file);
        if (fs.existsSync(file) && fs.statSync(file).isFile()) {
          const stored = this.store(fs.readFileSync(file), `mirror:${gh.repo}/${gh.file}`);
          if (stored) return stored;
        }
      }
    }
    const target = gh?.raw ?? url;
    try {
      const response = await fetch(target, { redirect: "follow", signal: AbortSignal.timeout(30_000) });
      if (!response.ok) return { error: `HTTP ${response.status}` };
      const bytes = Buffer.from(await response.arrayBuffer());
      if (bytes.length > MAX_BYTES) return { error: `too large (${bytes.length} bytes)` };
      const type = (response.headers.get("content-type") ?? "").split(";")[0].trim().toLowerCase();
      return this.store(bytes, target, TYPES[type]) ?? { error: `not an image (${type || "no content-type"})` };
    } catch (error) {
      return { error: (error as Error).message };
    }
  }

  private store(bytes: Buffer, from: string, typeExt?: string): RemoteEntry | null {
    const ext = sniff(bytes) ?? typeExt;
    if (!ext) return null;
    const sha256 = createHash("sha256").update(bytes).digest("hex");
    if (rasterKind(bytes)) {
      const file = `_remote/${sha256.slice(0, 24)}.webp`;
      this.opts.media.replace(bytes, file, from);
      return { file, sha256, bytes: bytes.length, from, replaced: true };
    }
    const file = `_remote/${sha256.slice(0, 24)}.${ext}`;
    const destination = path.join(this.opts.output, file);
    if (!fs.existsSync(destination)) {
      fs.mkdirSync(path.dirname(destination), { recursive: true });
      fs.writeFileSync(destination, bytes);
    }
    return { file, sha256, bytes: bytes.length, from };
  }

  manifest(): string {
    const sorted = Object.fromEntries([...this.entries.entries()].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)));
    return JSON.stringify(sorted, null, 1) + "\n";
  }
}
