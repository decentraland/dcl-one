// Minimal PNG codec over node:zlib: decodes the 8-bit non-interlaced images Chromium writes and
// encodes RGBA. Enough for screenshots, diff images and content placeholders without a dependency.
import { createHash } from "node:crypto";
import { deflateSync, inflateSync } from "node:zlib";

export type Rgba = { width: number; height: number; data: Uint8Array };

const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

function crc32(buf: Uint8Array): number {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = CRC_TABLE[(c ^ buf[i]!) & 0xff]! ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type: string, data: Uint8Array): Buffer {
  const head = Buffer.alloc(8);
  head.writeUInt32BE(data.length, 0);
  head.write(type, 4, "ascii");
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([head.subarray(4), data])), 0);
  return Buffer.concat([head, data, crc]);
}

export function encodePng(img: Rgba): Buffer {
  const { width, height, data } = img;
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  const raw = Buffer.alloc((width * 4 + 1) * height);
  for (let y = 0; y < height; y++) {
    raw[y * (width * 4 + 1)] = 0;
    raw.set(data.subarray(y * width * 4, (y + 1) * width * 4), y * (width * 4 + 1) + 1);
  }
  return Buffer.concat([SIGNATURE, chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw, { level: 6 })), chunk("IEND", new Uint8Array())]);
}

function paeth(a: number, b: number, c: number): number {
  const p = a + b - c;
  const pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
  return pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
}

export function decodePng(buf: Buffer): Rgba {
  if (!buf.subarray(0, 8).equals(SIGNATURE)) throw new Error("not a PNG");
  let off = 8;
  let width = 0, height = 0, depth = 0, colorType = 0, interlace = 0;
  let palette: Buffer | null = null;
  let trns: Buffer | null = null;
  const idat: Buffer[] = [];
  while (off < buf.length) {
    const len = buf.readUInt32BE(off);
    const type = buf.toString("ascii", off + 4, off + 8);
    const data = buf.subarray(off + 8, off + 8 + len);
    if (type === "IHDR") {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      depth = data[8]!;
      colorType = data[9]!;
      interlace = data[12]!;
    } else if (type === "PLTE") palette = data;
    else if (type === "tRNS") trns = data;
    else if (type === "IDAT") idat.push(data);
    else if (type === "IEND") break;
    off += 12 + len;
  }
  if (depth !== 8 || interlace !== 0) throw new Error(`unsupported PNG (depth ${depth}, interlace ${interlace})`);
  const channels = ({ 0: 1, 2: 3, 3: 1, 4: 2, 6: 4 } as Record<number, number>)[colorType];
  if (!channels) throw new Error(`unsupported PNG colour type ${colorType}`);
  const raw = inflateSync(Buffer.concat(idat));
  const stride = width * channels;
  const px = new Uint8Array(stride * height);
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)]!;
    const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    for (let x = 0; x < stride; x++) {
      const a = x >= channels ? px[y * stride + x - channels]! : 0;
      const b = y > 0 ? px[(y - 1) * stride + x]! : 0;
      const c = x >= channels && y > 0 ? px[(y - 1) * stride + x - channels]! : 0;
      const v = line[x]!;
      px[y * stride + x] = (filter === 0 ? v : filter === 1 ? v + a : filter === 2 ? v + b : filter === 3 ? v + ((a + b) >> 1) : v + paeth(a, b, c)) & 0xff;
    }
  }
  const out = new Uint8Array(width * height * 4);
  for (let i = 0, j = 0; i < width * height; i++, j += channels) {
    let r: number, g: number, b: number, a = 255;
    if (colorType === 0) r = g = b = px[j]!;
    else if (colorType === 4) { r = g = b = px[j]!; a = px[j + 1]!; }
    else if (colorType === 2) { r = px[j]!; g = px[j + 1]!; b = px[j + 2]!; }
    else if (colorType === 3) {
      const k = px[j]!;
      r = palette![k * 3]!; g = palette![k * 3 + 1]!; b = palette![k * 3 + 2]!;
      a = trns && k < trns.length ? trns[k]! : 255;
    } else { r = px[j]!; g = px[j + 1]!; b = px[j + 2]!; a = px[j + 3]!; }
    out.set([r, g, b, a], i * 4);
  }
  return { width, height, data: out };
}

/**
 * Paste one viewport tile, taken at scroll offset `top`, into the page image below the rows already
 * filled (a scroll clamped at the bottom overlaps the previous tile). Returns the rows now filled.
 */
export function pasteTile(out: Rgba, filled: number, top: number, shot: Rgba): number {
  const { width, height } = out;
  for (let row = Math.max(0, filled - top); row < shot.height && top + row < height; row++) {
    out.data.set(shot.data.subarray(row * shot.width * 4, row * shot.width * 4 + width * 4), (top + row) * width * 4);
  }
  return Math.max(filled, Math.min(height, top + shot.height));
}

/** Intrinsic size of a PNG, JPEG, GIF or WebP, read from its header; null when unknown. */
export function imageSize(buf: Buffer): { width: number; height: number } | null {
  if (buf.length > 24 && buf.subarray(0, 8).equals(SIGNATURE)) return { width: buf.readUInt32BE(16), height: buf.readUInt32BE(20) };
  if (buf.length > 10 && buf.toString("ascii", 0, 3) === "GIF") return { width: buf.readUInt16LE(6), height: buf.readUInt16LE(8) };
  if (buf.length > 30 && buf.toString("ascii", 0, 4) === "RIFF" && buf.toString("ascii", 8, 12) === "WEBP") {
    const kind = buf.toString("ascii", 12, 16);
    if (kind === "VP8X") return { width: 1 + buf.readUIntLE(24, 3), height: 1 + buf.readUIntLE(27, 3) };
    if (kind === "VP8 ") return { width: buf.readUInt16LE(26) & 0x3fff, height: buf.readUInt16LE(28) & 0x3fff };
    if (kind === "VP8L") {
      const b = buf.readUInt32LE(21);
      return { width: 1 + (b & 0x3fff), height: 1 + ((b >> 14) & 0x3fff) };
    }
  }
  if (buf.length > 4 && buf[0] === 0xff && buf[1] === 0xd8) {
    let off = 2;
    while (off + 9 < buf.length) {
      if (buf[off] !== 0xff) return null;
      const marker = buf[off + 1]!;
      const len = buf.readUInt16BE(off + 2);
      if (marker >= 0xc0 && marker <= 0xcf && marker !== 0xc4 && marker !== 0xc8 && marker !== 0xcc) {
        return { width: buf.readUInt16BE(off + 7), height: buf.readUInt16BE(off + 5) };
      }
      off += 2 + len;
    }
  }
  return null;
}

/**
 * A content placeholder: a deterministic colour field with a diagonal band and a border, seeded
 * by `seed`, so the same picture key paints the same pixels on both sides and a misplaced or
 * mis-cropped picture still shows in the diff.
 */
export function placeholderPng(seed: string, width = 256, height = 256): Buffer {
  const h = createHash("sha256").update(seed).digest();
  const scale = Math.min(1, 1024 / Math.max(width, height, 1));
  const w = Math.max(1, Math.round(width * scale)), ht = Math.max(1, Math.round(height * scale));
  const base = [h[0]!, h[1]!, h[2]!].map((v) => 64 + (v % 160));
  const band = [h[3]!, h[4]!, h[5]!].map((v) => 32 + (v % 200));
  const data = new Uint8Array(w * ht * 4);
  const bw = Math.max(2, Math.round(Math.min(w, ht) / 10));
  for (let y = 0; y < ht; y++) {
    for (let x = 0; x < w; x++) {
      const edge = x < 2 || y < 2 || x >= w - 2 || y >= ht - 2;
      const diag = Math.abs(x * ht - y * w) / Math.max(w, ht) < bw;
      const c = edge ? [20, 20, 20] : diag ? band : base;
      data.set([c[0]!, c[1]!, c[2]!, 255], (y * w + x) * 4);
    }
  }
  return encodePng({ width: w, height: ht, data });
}
