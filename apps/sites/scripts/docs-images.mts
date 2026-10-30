// Build-time WebP replacement of the vendored docs images (gen-docs-content.mts). Every
// raster image the pages reference (PNG, JPEG, GIF, animated ones included) is written
// once as WebP beside where the original would have gone (x.png -> x.webp); the original
// is never copied into content/docs. The decisions land in _media.json keyed by the
// served /docs URL; the SSR renderer (src/data/lib/docs/images.ts) reads the dimensions.
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import { execFile } from "node:child_process";
import { promisify } from "node:util";

const exec = promisify(execFile);

// Static screenshots: capped at STATIC_WIDTH, lossless when small (icons, diagrams),
// lossy otherwise (ImageMagick). Animated GIFs: gif2webp, which keeps the GIF's frame
// differencing that ImageMagick's coalesced frames lose: -mixed, then -lossy when that
// saved too little, the smaller kept. Animated PNGs (rare) go through ImageMagick.
export const STATIC_WIDTH = 1600;
export const ANIMATED_WIDTH = 1024;
const STATIC_QUALITY = 82;
const ANIMATED_QUALITY = 70;
const LOSSLESS_BELOW = 16 * 1024;
const RETRY_LOSSY_ABOVE = 0.6;
export const ENCODER = `docs-media-v3-s${STATIC_WIDTH}q${STATIC_QUALITY}l${LOSSLESS_BELOW}-g2w-q${ANIMATED_QUALITY}r${RETRY_LOSSY_ABOVE}-a${ANIMATED_WIDTH}:`;

export interface MediaEntry { source: string; encoder: string; key: string; width: number; height: number; bytes: number; sourceBytes: number; animated?: true }

/** The cache key of a source image under the current encoder settings. */
export function mediaKey(bytes: Buffer): string {
  return createHash("sha256").update(ENCODER).update(bytes).digest("hex").slice(0, 24);
}

export type RasterKind = "png" | "jpg" | "gif";

/** The raster format by magic bytes (GitBook assets often have no extension). */
export function rasterKind(bytes: Buffer): RasterKind | null {
  if (bytes.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]))) return "png";
  if (bytes[0] === 0xff && bytes[1] === 0xd8) return "jpg";
  if (bytes.subarray(0, 4).toString("ascii") === "GIF8") return "gif";
  return null;
}

// ImageMagick's PNG reader does not always expose APNG frames. Inspect the PNG
// animation-control chunk before decoding so an animation can never be flattened.
export function isAnimatedPng(bytes: Buffer): boolean {
  if (rasterKind(bytes) !== "png") return false;
  for (let offset = 8; offset + 12 <= bytes.length;) {
    const size = bytes.readUInt32BE(offset);
    const type = bytes.toString("ascii", offset + 4, offset + 8);
    if (type === "acTL") return true;
    if (type === "IDAT" || type === "IEND") return false;
    offset += size + 12;
  }
  return false;
}

/** x.png -> x.webp, extensionless -> x.webp; the served name of a replaced image. */
export function webpName(rel: string): string {
  return rel.replace(/\.(png|jpe?g|gif)$/i, "") + ".webp";
}

/**
 * Content-addressed WebP replacements. A previous decision is reused when the source
 * bytes (and encoder settings) hash to the same key and its WebP is present, so a
 * re-run calls ImageMagick only for new or changed images; `readOnly` (--check) never
 * calls it and reports what would have to be built instead.
 */
export class DocsMedia {
  readonly entries = new Map<string, MediaEntry>();
  readonly missing: string[] = [];
  private pending = new Map<string, { bytes: Buffer; rel: string; source: string; key: string }>();
  private previous: Record<string, MediaEntry>;
  private opts: { output: string; base?: string; magick?: string; readOnly?: boolean };

  constructor(opts: { output: string; base?: string; magick?: string; readOnly?: boolean }) {
    this.opts = opts;
    const manifest = path.join(opts.output, "_media.json");
    this.previous = fs.existsSync(manifest) ? JSON.parse(fs.readFileSync(manifest, "utf8")) : {};
  }

  private get base() { return this.opts.base ?? "/docs"; }
  private get magick() { return this.opts.magick ?? process.env.MAGICK ?? "magick"; }
  private get gif2webp() { return process.env.GIF2WEBP ?? "gif2webp"; }

  /** Output-relative paths of every WebP this run keeps. */
  files(): string[] {
    return [...this.entries.keys()].map((url) => url.slice(this.base.length + 1));
  }

  /**
   * Queue the replacement of a raster (its bytes) with <output>/<rel>, a WebP; flush()
   * encodes. `source` is what the manifest records as its origin (docs path or remote URL).
   */
  replace(bytes: Buffer, rel: string, source: string): void {
    const url = `${this.base}/${rel}`;
    if (this.entries.has(url) || this.pending.has(url)) return;
    const key = mediaKey(bytes);
    const destination = path.join(this.opts.output, rel);
    const prior = this.previous[url];
    if (prior && prior.key === key && prior.encoder === ENCODER && fs.existsSync(destination)) {
      this.entries.set(url, { ...prior, source });
      return;
    }
    if (this.opts.readOnly) {
      this.missing.push(url);
      return;
    }
    this.pending.set(url, { bytes, rel, source, key });
  }

  /** Encode every queued replacement, `concurrency` at a time. */
  async flush(concurrency = Math.max(1, Math.floor(os.availableParallelism() / 4))): Promise<void> {
    const queue = [...this.pending.entries()];
    this.pending.clear();
    const worker = async () => {
      for (let next = queue.shift(); next; next = queue.shift()) await this.encode(next[0], next[1]);
    };
    await Promise.all(Array.from({ length: Math.min(concurrency, queue.length) }, worker));
  }

  private async encode(url: string, job: { bytes: Buffer; rel: string; source: string; key: string }): Promise<void> {
    const { bytes, rel, source, key } = job;
    const destination = path.join(this.opts.output, rel);
    fs.mkdirSync(path.dirname(destination), { recursive: true });
    const scratch = fs.mkdtempSync(path.join(os.tmpdir(), "docs-media-"));
    try {
      const kind = rasterKind(bytes);
      const input = path.join(scratch, `source.${kind}`);
      fs.writeFileSync(input, bytes);
      const apng = isAnimatedPng(bytes);
      const animated = apng || (kind === "gif" && (await this.run(this.magick, ["identify", "-ping", "-format", "x", input], rel)).length > 1);
      const output = path.join(scratch, "out.webp");
      if (animated && !apng) {
        const mixed = path.join(scratch, "mixed.webp"), lossy = path.join(scratch, "lossy.webp");
        await this.run(this.gif2webp, ["-quiet", "-mixed", "-q", String(ANIMATED_QUALITY), "-m", "6", "-min_size", input, "-o", mixed], rel);
        let best = mixed;
        if (fs.statSync(mixed).size > bytes.length * RETRY_LOSSY_ABOVE) {
          await this.run(this.gif2webp, ["-quiet", "-lossy", "-q", String(ANIMATED_QUALITY), "-m", "6", "-min_size", input, "-o", lossy], rel);
          if (fs.statSync(lossy).size < fs.statSync(mixed).size) best = lossy;
        }
        fs.renameSync(best, output);
      } else {
        const args = animated
          ? ["-limit", "thread", "2", `apng:${input}`, "-coalesce", "-resize", `${ANIMATED_WIDTH}x>`, "-strip",
            "-quality", String(ANIMATED_QUALITY), "-define", "webp:method=4"]
          : ["-limit", "thread", "2", `${input}[0]`, "-auto-orient", "-resize", `${STATIC_WIDTH}x>`, "-strip",
            ...(bytes.length < LOSSLESS_BELOW ? ["-define", "webp:lossless=true"] : ["-quality", String(STATIC_QUALITY)]),
            "-define", "webp:method=6"];
        await this.run(this.magick, [...args, output], rel);
      }
      const [width, height] = (await this.run(this.magick, ["identify", "-ping", "-format", "%w %h\\n", output], rel))
        .trim().split("\n")[0].split(" ").map(Number);
      const temporary = `${destination}.${process.pid}.tmp`;
      fs.copyFileSync(output, temporary);
      fs.renameSync(temporary, destination);
      this.entries.set(url, {
        source, encoder: ENCODER, key, width, height, bytes: fs.statSync(destination).size, sourceBytes: bytes.length,
        ...(animated ? { animated: true as const } : {}),
      });
    } finally { fs.rmSync(scratch, { recursive: true, force: true }); }
  }

  /** Keep a previous replacement whose source is not re-read (a vendored remote image) when it is current. */
  keep(rel: string, source: string): boolean {
    const url = `${this.base}/${rel}`;
    const prior = this.previous[url];
    if (!prior || prior.encoder !== ENCODER || !fs.existsSync(path.join(this.opts.output, rel))) return false;
    this.entries.set(url, { ...prior, source });
    return true;
  }

  private async run(tool: string, args: string[], label: string): Promise<string> {
    try {
      return (await exec(tool, args, { encoding: "utf8", timeout: 1_800_000, maxBuffer: 64 * 1024 * 1024 })).stdout;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ENOENT") throw new Error(`docs media needs ${tool} (ImageMagick via MAGICK, libwebp's gif2webp via GIF2WEBP)`);
      throw new Error(`cannot encode docs image ${label}`, { cause: error });
    }
  }

  manifest(): string {
    const sorted = Object.fromEntries([...this.entries.entries()].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)));
    return JSON.stringify(sorted, null, 1) + "\n";
  }
}
