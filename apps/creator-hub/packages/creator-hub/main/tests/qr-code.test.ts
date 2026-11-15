import { createHash } from 'crypto';
import { describe, expect, it } from 'vitest';

import { encodeQrCode, qrCodeDataUrl } from '../src/modules/qr-code';

// Each vector is the module matrix that the `qrcode` package 1.5.4 gives for the same text in
// byte mode at error correction level M.
const VECTORS: [text: string, version: number, sha256: string][] = [
  ['a', 1, '20c5121183b26a4b1c7b088fc94a60ac955699662e7d591e3e791a5456f6c87e'],
  ['x'.repeat(14), 1, '6d1a4b24d69a3096bd16b1d9d73d1981e7c99f5293a58c88adf0c5f0f339fc28'],
  ['x'.repeat(15), 2, '6f3528792c65e2328422fab8e41f38b87f6c1a2b2ff78e23e8aadbf7f140562b'],
  ['año 日本 🙂', 2, 'ac88b34b8460f0f9166f87cbfb652eb746e4839dd557e5f2d878ad7e200f2dc4'],
  [
    'decentraland://?scene-inspector=ws%3A%2F%2F192.168.0.10%3A8765',
    4,
    '374d9586d7f94389501cd7f78dcebf4da0f82cde5c6d95ab618af49a4d9ec0e4',
  ],
  ['x'.repeat(122), 7, 'c6c49609272c857f5a072b86089d603dbe48413d2acfdfc81d6c62cea036ac8c'],
  ['x'.repeat(213), 10, 'b27c5d8141e70c48573b858359d3a9712689c88ec3a8599df1b0f32aefe96fce'],
  ['x'.repeat(214), 11, 'e4869f5b63d1780e576df12d7d8ce1ee55e9b402fd79ff5d90e068740e023a68'],
  ['q'.repeat(1000), 26, '55b5189f4074fb9286b08b5677bcfc2fc0d8abd0589bfe914426d63c7b3a14c8'],
  ['z'.repeat(2331), 40, '9db2bff06ebfe6caf3de6fcc5c29413c257241d5acdf0cfe520d103e75cb947a'],
];

const LETTER_A = [
  '#######..#.##.#######',
  '#.....#.#.##..#.....#',
  '#.###.#.##.#..#.###.#',
  '#.###.#.#.##..#.###.#',
  '#.###.#..#..#.#.###.#',
  '#.....#...##..#.....#',
  '#######.#.#.#.#######',
  '........##...........',
  '#.....#.#.##.##..###.',
  '#..##......###.###..#',
  '..#.###..##.#.##.....',
  '.#.#.#.##..#####.#.#.',
  '##.#..####.##########',
  '........##..#.....#.#',
  '#######..###.#..####.',
  '#.....#...#...#...###',
  '#.###.#..###.#..###..',
  '#.###.#..#.#####.#...',
  '#.###.#..#.###.###.##',
  '#.....#...######.#...',
  '#######.#.#.#..#..##.',
];

function rows({ size, modules }: { size: number; modules: Uint8Array }): string[] {
  return Array.from({ length: size }, (_, row) =>
    Array.from(modules.subarray(row * size, (row + 1) * size), dark => (dark ? '#' : '.')).join(''),
  );
}

describe('encodeQrCode', () => {
  describe('when the text fits in a QR code', () => {
    it('should give the modules of the smallest version, row by row', () => {
      expect(rows(encodeQrCode('a'))).toEqual(LETTER_A);
    });

    it.each(VECTORS)('should give the reference matrix for %j', (text, version, sha256) => {
      const { size, modules } = encodeQrCode(text);

      expect(size).toBe(version * 4 + 17);
      expect(modules).toHaveLength(size * size);
      expect(createHash('sha256').update(modules).digest('hex')).toBe(sha256);
    });
  });

  describe('when the text is longer than the largest QR code holds', () => {
    it('should throw an error that states the length', () => {
      expect(() => encodeQrCode('z'.repeat(2332))).toThrow(
        'The text is too long for a QR code: 2332 bytes',
      );
    });
  });
});

describe('qrCodeDataUrl', () => {
  function svgOf(text: string): string {
    const [header, body] = qrCodeDataUrl(text).split(',');
    expect(header).toBe('data:image/svg+xml;base64');
    return Buffer.from(body, 'base64').toString('utf8');
  }

  describe('when the text fits in a QR code', () => {
    it('should give a square SVG image with a quiet zone of two modules', () => {
      expect(svgOf('a')).toMatch(
        /^<svg xmlns="http:\/\/www\.w3\.org\/2000\/svg" width="512" height="512" viewBox="0 0 25 25" shape-rendering="crispEdges"><path fill="#fff" d="M0 0h25v25H0z"\/><path fill="#000" d="[^"]+"\/><\/svg>$/,
      );
    });

    it('should draw each run of dark modules of a row as one rectangle', () => {
      const svg = svgOf('a');
      const runs = LETTER_A.flatMap((row, y) =>
        [...row.matchAll(/#+/g)].map(
          run => `M${run.index + 2} ${y + 2}h${run[0].length}v1H${run.index + 2}z`,
        ),
      );

      expect(svg).toContain(`<path fill="#000" d="${runs.join('')}"/>`);
    });
  });
});
