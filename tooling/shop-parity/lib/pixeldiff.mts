// Perceptual pixel diff: the pixelmatch method (YIQ colour delta with a perceptual threshold, and
// anti-aliasing detection so a pixel that is an edge's anti-aliasing on either side is tolerated).
// Area only one image has counts. A mask hides what cannot be equal inside a box both images have
// at the same place, so a pixel only one image masks counts too (the box moved, grew, or is missing),
// unless the other image's box comes within 1px of it: the structural diff's tolerance. A region is a
// deliberate difference's area, whose extent differs between the sides, and hides its pixels on both.
import type { Box } from "./collect.mts";
import type { Rgba } from "./png.mts";

export type Band = { y0: number; y1: number; x0: number; x1: number; pixels: number };
export type PixelResult = {
  width: number;
  height: number;
  /** Differing pixels outside masks and anti-aliasing. */
  diff: number;
  /** Pixels tolerated as anti-aliasing. */
  antialiased: number;
  masked: number;
  ratio: number;
  bands: Band[];
  image: Rgba;
};

const rgb2y = (r: number, g: number, b: number) => r * 0.29889531 + g * 0.58662247 + b * 0.11448223;
const rgb2i = (r: number, g: number, b: number) => r * 0.59597799 - g * 0.2741761 - b * 0.32180189;
const rgb2q = (r: number, g: number, b: number) => r * 0.21147017 - g * 0.52261711 + b * 0.31114694;
const blend = (c: number, a: number) => 255 + (c - 255) * a;

function colorDelta(a: Uint8Array, b: Uint8Array, k: number, m: number, yOnly: boolean): number {
  let r1 = a[k]!, g1 = a[k + 1]!, b1 = a[k + 2]!;
  let r2 = b[m]!, g2 = b[m + 1]!, b2 = b[m + 2]!;
  const a1 = a[k + 3]!, a2 = b[m + 3]!;
  if (a1 === a2 && r1 === r2 && g1 === g2 && b1 === b2) return 0;
  if (a1 < 255) {
    const f = a1 / 255;
    r1 = blend(r1, f); g1 = blend(g1, f); b1 = blend(b1, f);
  }
  if (a2 < 255) {
    const f = a2 / 255;
    r2 = blend(r2, f); g2 = blend(g2, f); b2 = blend(b2, f);
  }
  const y1 = rgb2y(r1, g1, b1), y2 = rgb2y(r2, g2, b2), y = y1 - y2;
  if (yOnly) return y;
  const i = rgb2i(r1, g1, b1) - rgb2i(r2, g2, b2);
  const q = rgb2q(r1, g1, b1) - rgb2q(r2, g2, b2);
  const delta = 0.5053 * y * y + 0.299 * i * i + 0.1957 * q * q;
  return y1 > y2 ? -delta : delta;
}

function hasManySiblings(img: Uint8Array, x1: number, y1: number, w: number, h: number): boolean {
  const x0 = Math.max(x1 - 1, 0), y0 = Math.max(y1 - 1, 0), x2 = Math.min(x1 + 1, w - 1), y2 = Math.min(y1 + 1, h - 1);
  const pos = (y1 * w + x1) * 4;
  let zeroes = x1 === x0 || x1 === x2 || y1 === y0 || y1 === y2 ? 1 : 0;
  for (let x = x0; x <= x2; x++) {
    for (let y = y0; y <= y2; y++) {
      if (x === x1 && y === y1) continue;
      const p = (y * w + x) * 4;
      if (img[pos] === img[p] && img[pos + 1] === img[p + 1] && img[pos + 2] === img[p + 2] && img[pos + 3] === img[p + 3]) zeroes++;
      if (zeroes > 2) return true;
    }
  }
  return false;
}

function antialiased(img: Uint8Array, x1: number, y1: number, w: number, h: number, other: Uint8Array): boolean {
  const x0 = Math.max(x1 - 1, 0), y0 = Math.max(y1 - 1, 0), x2 = Math.min(x1 + 1, w - 1), y2 = Math.min(y1 + 1, h - 1);
  const pos = (y1 * w + x1) * 4;
  let zeroes = x1 === x0 || x1 === x2 || y1 === y0 || y1 === y2 ? 1 : 0;
  let min = 0, max = 0, minX = 0, minY = 0, maxX = 0, maxY = 0;
  for (let x = x0; x <= x2; x++) {
    for (let y = y0; y <= y2; y++) {
      if (x === x1 && y === y1) continue;
      const delta = colorDelta(img, img, pos, (y * w + x) * 4, true);
      if (delta === 0) {
        zeroes++;
        if (zeroes > 2) return false;
      } else if (delta < min) {
        min = delta; minX = x; minY = y;
      } else if (delta > max) {
        max = delta; maxX = x; maxY = y;
      }
    }
  }
  if (min === 0 || max === 0) return false;
  return (hasManySiblings(img, minX, minY, w, h) && hasManySiblings(other, minX, minY, w, h)) || (hasManySiblings(img, maxX, maxY, w, h) && hasManySiblings(other, maxX, maxY, w, h));
}

function pad(img: Rgba, w: number, h: number): { data: Uint8Array; inside: (x: number, y: number) => boolean } {
  if (img.width === w && img.height === h) return { data: img.data, inside: () => true };
  const data = new Uint8Array(w * h * 4);
  for (let y = 0; y < img.height; y++) data.set(img.data.subarray(y * img.width * 4, (y + 1) * img.width * 4), y * w * 4);
  return { data, inside: (x, y) => x < img.width && y < img.height };
}

const MASK_A = 1, MASK_B = 2, REGION = 4, NEAR_A = 8, NEAR_B = 16;

export function pixelDiff(a: Rgba, b: Rgba, opts: { threshold?: number; masks?: { a: Box[]; b: Box[] }; regions?: Box[] } = {}): PixelResult {
  const w = Math.max(a.width, b.width), h = Math.max(a.height, b.height);
  const A = pad(a, w, h), B = pad(b, w, h);
  const masked = new Uint8Array(w * h);
  const mark = (boxes: Box[], bit: number, grow = 0) => {
    for (const m of boxes) {
      for (let y = Math.max(0, Math.floor(m.y) - grow); y < Math.min(h, Math.ceil(m.y + m.h) + grow); y++) {
        for (let x = Math.max(0, Math.floor(m.x) - grow); x < Math.min(w, Math.ceil(m.x + m.w) + grow); x++) masked[y * w + x]! |= bit;
      }
    }
  };
  mark(opts.masks?.a ?? [], MASK_A);
  mark(opts.masks?.b ?? [], MASK_B);
  mark(opts.masks?.a ?? [], NEAR_A, 1);
  mark(opts.masks?.b ?? [], NEAR_B, 1);
  mark(opts.regions ?? [], REGION);
  const maxDelta = 35215 * (opts.threshold ?? 0.1) ** 2;
  const out = new Uint8Array(w * h * 4);
  const rowDiff = new Uint32Array(h);
  const rowX0 = new Int32Array(h).fill(w), rowX1 = new Int32Array(h).fill(-1);
  let diff = 0, aa = 0, nMasked = 0;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const i = y * w + x, p = i * 4;
      const m = masked[i]!;
      if (m & REGION || (m & MASK_A && m & NEAR_B) || (m & MASK_B && m & NEAR_A)) {
        nMasked++;
        out.set([120, 60, 200, 255], p);
        continue;
      }
      let bad = false, tolerated = false;
      if (m & (MASK_A | MASK_B) || !A.inside(x, y) || !B.inside(x, y)) bad = true;
      else {
        const delta = colorDelta(A.data, B.data, p, p, false);
        if (Math.abs(delta) > maxDelta) {
          if (antialiased(A.data, x, y, w, h, B.data) || antialiased(B.data, x, y, w, h, A.data)) tolerated = true;
          else bad = true;
        }
      }
      if (bad) {
        diff++;
        rowDiff[y]!++;
        if (x < rowX0[y]!) rowX0[y] = x;
        if (x > rowX1[y]!) rowX1[y] = x;
        out.set([255, 0, 60, 255], p);
      } else if (tolerated) {
        aa++;
        out.set([255, 210, 0, 255], p);
      } else {
        const src = A.inside(x, y) ? A.data : B.data;
        const g = 255 + (rgb2y(src[p]!, src[p + 1]!, src[p + 2]!) - 255) * 0.15;
        out.set([g, g, g, 255], p);
      }
    }
  }
  // Rows with differences, merged across gaps under 12px, as the places to look.
  const bands: Band[] = [];
  for (let y = 0; y < h; y++) {
    if (!rowDiff[y]) continue;
    const last = bands.at(-1);
    if (last && y - last.y1 <= 12) {
      last.y1 = y;
      last.pixels += rowDiff[y]!;
      last.x0 = Math.min(last.x0, rowX0[y]!);
      last.x1 = Math.max(last.x1, rowX1[y]!);
    } else bands.push({ y0: y, y1: y, x0: rowX0[y]!, x1: rowX1[y]!, pixels: rowDiff[y]! });
  }
  bands.sort((p, q) => q.pixels - p.pixels);
  return { width: w, height: h, diff, antialiased: aa, masked: nMasked, ratio: diff / Math.max(1, w * h - nMasked), bands, image: { width: w, height: h, data: out } };
}
