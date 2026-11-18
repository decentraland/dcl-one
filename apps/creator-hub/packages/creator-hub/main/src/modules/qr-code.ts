/**
 * QR code encoder (ISO/IEC 18004) for the links that a phone scans. It writes byte mode with
 * error correction level M, in the smallest version from 1 to 40 that holds the text.
 */

export type QrCode = { size: number; modules: Uint8Array };

/** Number of error correction blocks at level M, by version. */
// prettier-ignore
const BLOCKS = [
  1, 1, 1, 2, 2, 4, 4, 4, 5, 5, 5, 8, 9, 9, 10, 10, 11, 13, 14, 16,
  17, 17, 18, 20, 21, 23, 25, 26, 28, 29, 31, 33, 35, 37, 38, 40, 43, 45, 47, 49,
];
/** Error correction codewords in each block at level M, by version. */
// prettier-ignore
const BLOCK_EC_CODEWORDS = [
  10, 16, 26, 18, 24, 16, 18, 22, 22, 26, 30, 22, 22, 24, 24, 28, 28, 26, 26, 26,
  26, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28,
];
const MAX_VERSION = 40;
const MODE_BYTE = 0b0100;
const LEVEL_M = 0b00;
const FORMAT_GENERATOR = 0x537;
const FORMAT_MASK = 0x5412;
const VERSION_GENERATOR = 0x1f25;
const MASKS: ((row: number, col: number) => boolean)[] = [
  (row, col) => (row + col) % 2 === 0,
  row => row % 2 === 0,
  (_row, col) => col % 3 === 0,
  (row, col) => (row + col) % 3 === 0,
  (row, col) => (Math.floor(row / 2) + Math.floor(col / 3)) % 2 === 0,
  (row, col) => ((row * col) % 2) + ((row * col) % 3) === 0,
  (row, col) => (((row * col) % 2) + ((row * col) % 3)) % 2 === 0,
  (row, col) => (((row * col) % 3) + ((row + col) % 2)) % 2 === 0,
];
const QUIET_ZONE = 2;
const IMAGE_SIZE = 512;

const EXP = new Uint8Array(510);
const LOG = new Uint8Array(256);
for (let power = 0, value = 1; power < 255; power++) {
  EXP[power] = EXP[power + 255] = value;
  LOG[value] = power;
  value <<= 1;
  if (value & 0x100) value ^= 0x11d;
}

function multiply(a: number, b: number): number {
  return a === 0 || b === 0 ? 0 : EXP[LOG[a] + LOG[b]];
}

function totalCodewords(version: number): number {
  let modules = (16 * version + 128) * version + 64;
  if (version >= 2) {
    const patterns = Math.floor(version / 7) + 2;
    modules -= (25 * patterns - 10) * patterns - 55;
    if (version >= 7) modules -= 36;
  }
  return Math.floor(modules / 8);
}

function dataCodewords(version: number): number {
  return totalCodewords(version) - BLOCKS[version - 1] * BLOCK_EC_CODEWORDS[version - 1];
}

function lengthBits(version: number): number {
  return version < 10 ? 8 : 16;
}

function pickVersion(byteCount: number): number {
  for (let version = 1; version <= MAX_VERSION; version++) {
    if (4 + lengthBits(version) + 8 * byteCount <= 8 * dataCodewords(version)) return version;
  }
  throw new Error(`The text is too long for a QR code: ${byteCount} bytes`);
}

function dataBytes(bytes: Uint8Array, version: number): Uint8Array {
  const data = new Uint8Array(dataCodewords(version));
  let bit = 0;
  const put = (value: number, count: number) => {
    for (let shift = count - 1; shift >= 0; shift--, bit++) {
      if ((value >>> shift) & 1) data[bit >>> 3] |= 0x80 >>> (bit & 7);
    }
  };
  put(MODE_BYTE, 4);
  put(bytes.length, lengthBits(version));
  for (const byte of bytes) put(byte, 8);
  for (let at = Math.ceil((bit + 4) / 8), pad = 0xec; at < data.length; at++, pad ^= 0xfd) {
    data[at] = pad;
  }
  return data;
}

function generator(degree: number): Uint8Array {
  let poly = Uint8Array.of(1);
  for (let root = 0; root < degree; root++) {
    const next = new Uint8Array(poly.length + 1);
    poly.forEach((coefficient, index) => {
      next[index] ^= coefficient;
      next[index + 1] ^= multiply(coefficient, EXP[root]);
    });
    poly = next;
  }
  return poly;
}

function errorCorrection(data: Uint8Array, divisor: Uint8Array): Uint8Array {
  const degree = divisor.length - 1;
  const rest = new Uint8Array(degree);
  for (const byte of data) {
    const factor = byte ^ rest[0];
    rest.copyWithin(0, 1);
    rest[degree - 1] = 0;
    for (let index = 0; index < degree; index++)
      rest[index] ^= multiply(divisor[index + 1], factor);
  }
  return rest;
}

function codewords(data: Uint8Array, version: number): Uint8Array {
  const total = totalCodewords(version);
  const blockCount = BLOCKS[version - 1];
  const ecSize = BLOCK_EC_CODEWORDS[version - 1];
  const shortSize = Math.floor(total / blockCount) - ecSize;
  const shortBlocks = blockCount - (total % blockCount);
  const divisor = generator(ecSize);

  const blocks: Uint8Array[] = [];
  for (let index = 0, at = 0; index < blockCount; index++) {
    const size = shortSize + (index < shortBlocks ? 0 : 1);
    blocks.push(data.subarray(at, at + size));
    at += size;
  }
  const corrections = blocks.map(block => errorCorrection(block, divisor));

  const result = new Uint8Array(total);
  let at = 0;
  for (let index = 0; index <= shortSize; index++) {
    for (const block of blocks) if (index < block.length) result[at++] = block[index];
  }
  for (let index = 0; index < ecSize; index++) {
    for (const correction of corrections) result[at++] = correction[index];
  }
  return result;
}

function alignmentCoordinates(version: number): number[] {
  if (version === 1) return [];
  const size = version * 4 + 17;
  const count = Math.floor(version / 7) + 2;
  const step = version === 32 ? 26 : Math.ceil((size - 13) / (2 * count - 2)) * 2;
  const coordinates = [6];
  for (let index = count - 2; index >= 0; index--) coordinates.push(size - 7 - index * step);
  return coordinates;
}

/** Appends to `data` the rest of its division by `divisor`, a polynomial of the given degree. */
function withCheckBits(data: number, divisor: number, degree: number): number {
  let rest = data << degree;
  for (let top = 31 - Math.clz32(rest); top >= degree; top = 31 - Math.clz32(rest)) {
    rest ^= divisor << (top - degree);
  }
  return (data << degree) | rest;
}

function penalty(modules: Uint8Array, size: number): number {
  let points = 0;
  let dark = 0;
  for (let a = 0; a < size; a++) {
    for (const horizontal of [true, false]) {
      let run = 0;
      let last = -1;
      let window = 0;
      for (let b = 0; b < size; b++) {
        const module = horizontal ? modules[a * size + b] : modules[b * size + a];
        if (module === last) {
          run++;
        } else {
          if (run >= 5) points += run - 2;
          last = module;
          run = 1;
        }
        window = ((window << 1) & 0x7ff) | module;
        if (b >= 10 && (window === 0x5d0 || window === 0x05d)) points += 40;
      }
      if (run >= 5) points += run - 2;
    }
    for (let b = 0; b < size; b++) {
      const module = modules[a * size + b];
      dark += module;
      if (a === size - 1 || b === size - 1) continue;
      const block =
        module +
        modules[a * size + b + 1] +
        modules[(a + 1) * size + b] +
        modules[(a + 1) * size + b + 1];
      if (block === 0 || block === 4) points += 3;
    }
  }
  return points + Math.abs(Math.ceil((dark * 100) / modules.length / 5) - 10) * 10;
}

/** Encodes `text` (as UTF-8) and gives the modules row by row: 1 is a dark module. */
export function encodeQrCode(text: string): QrCode {
  const bytes = new TextEncoder().encode(text);
  const version = pickVersion(bytes.length);
  const size = version * 4 + 17;
  const modules = new Uint8Array(size * size);
  const reserved = new Uint8Array(size * size);
  const set = (row: number, col: number, dark: boolean | number) => {
    modules[row * size + col] = dark ? 1 : 0;
    reserved[row * size + col] = 1;
  };

  for (const [top, left] of [
    [0, 0],
    [0, size - 7],
    [size - 7, 0],
  ]) {
    for (let row = -1; row <= 7; row++) {
      for (let col = -1; col <= 7; col++) {
        if (top + row < 0 || top + row >= size || left + col < 0 || left + col >= size) continue;
        const ring = Math.max(Math.abs(row - 3), Math.abs(col - 3));
        set(top + row, left + col, ring <= 1 || ring === 3);
      }
    }
  }

  for (let at = 8; at < size - 8; at++) {
    set(at, 6, at % 2 === 0);
    set(6, at, at % 2 === 0);
  }

  const coordinates = alignmentCoordinates(version);
  const last = coordinates.length - 1;
  coordinates.forEach((centerRow, rowIndex) => {
    coordinates.forEach((centerCol, colIndex) => {
      const onFinder =
        (rowIndex === 0 && (colIndex === 0 || colIndex === last)) ||
        (rowIndex === last && colIndex === 0);
      if (onFinder) return;
      for (let row = -2; row <= 2; row++) {
        for (let col = -2; col <= 2; col++) {
          set(centerRow + row, centerCol + col, Math.max(Math.abs(row), Math.abs(col)) !== 1);
        }
      }
    });
  });

  const setFormat = (mask: number) => {
    const bits = withCheckBits((LEVEL_M << 3) | mask, FORMAT_GENERATOR, 10) ^ FORMAT_MASK;
    for (let index = 0; index < 15; index++) {
      const dark = (bits >> index) & 1;
      set(index < 6 ? index : index < 8 ? index + 1 : size - 15 + index, 8, dark);
      set(8, index < 8 ? size - index - 1 : index < 9 ? 7 : 14 - index, dark);
    }
    set(size - 8, 8, true);
  };
  setFormat(0);

  if (version >= 7) {
    const bits = withCheckBits(version, VERSION_GENERATOR, 12);
    for (let index = 0; index < 18; index++) {
      const row = Math.floor(index / 3);
      const col = (index % 3) + size - 11;
      set(row, col, (bits >> index) & 1);
      set(col, row, (bits >> index) & 1);
    }
  }

  const stream = codewords(dataBytes(bytes, version), version);
  let bit = 0;
  let row = size - 1;
  let step = -1;
  for (let right = size - 1; right > 0; right -= 2) {
    if (right === 6) right--;
    for (; row >= 0 && row < size; row += step) {
      for (const col of [right, right - 1]) {
        if (reserved[row * size + col]) continue;
        modules[row * size + col] = (stream[bit >>> 3] >>> (7 - (bit & 7))) & 1;
        bit++;
      }
    }
    step = -step;
    row += step;
  }

  const applyMask = (mask: number) => {
    for (let index = 0; index < modules.length; index++) {
      if (!reserved[index] && MASKS[mask](Math.floor(index / size), index % size)) {
        modules[index] ^= 1;
      }
    }
  };
  let best = 0;
  let lowest = Infinity;
  for (let mask = 0; mask < MASKS.length; mask++) {
    setFormat(mask);
    applyMask(mask);
    const points = penalty(modules, size);
    applyMask(mask);
    if (points < lowest) {
      lowest = points;
      best = mask;
    }
  }
  setFormat(best);
  applyMask(best);

  return { size, modules };
}

/** Draws the QR code of `text` as an SVG image and gives it as a data URL for an `<img>`. */
export function qrCodeDataUrl(text: string): string {
  const { size, modules } = encodeQrCode(text);
  const side = size + 2 * QUIET_ZONE;
  let path = '';
  for (let row = 0; row < size; row++) {
    for (let col = 0; col < size; col++) {
      if (!modules[row * size + col]) continue;
      const start = col;
      while (col + 1 < size && modules[row * size + col + 1]) col++;
      path += `M${start + QUIET_ZONE} ${row + QUIET_ZONE}h${col - start + 1}v1H${start + QUIET_ZONE}z`;
    }
  }
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" width="${IMAGE_SIZE}" height="${IMAGE_SIZE}" ` +
    `viewBox="0 0 ${side} ${side}" shape-rendering="crispEdges">` +
    `<path fill="#fff" d="M0 0h${side}v${side}H0z"/><path fill="#000" d="${path}"/></svg>`;
  return `data:image/svg+xml;base64,${Buffer.from(svg).toString('base64')}`;
}
