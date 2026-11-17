import type { Dirent } from 'fs';
import { readdirSync } from 'fs';
import { dirname, extname, join } from 'path';
import { fileURLToPath } from 'url';
import { describe, expect, it } from 'vitest';

import { CONTENT_TYPES, contentType } from '../scripts/utils/content-type.ts';

const packsDir = join(dirname(fileURLToPath(import.meta.url)), '..', 'packs');

describe('contentType', () => {
  describe('when the file is of a kind that the packs hold', () => {
    it.each([
      ['packs/a/assets/tree/tree.glb', 'model/gltf-binary'],
      ['packs/a/assets/tree/thumbnail.png', 'image/png'],
      ['packs/a/assets/tree/Photo.JPG', 'image/jpeg'],
      ['packs/a/assets/tree/data.json', 'application/json'],
      ['packs/a/assets/radio/song.mp3', 'audio/mpeg'],
      ['packs/a/assets/tree/image.webp', 'image/webp'],
      ['packs/a/assets/door/README.md', 'text/markdown'],
    ])('should give the content type of %s', (file, expected) => {
      expect(contentType(file)).toBe(expected);
    });
  });

  describe('when the kind of the file is not known', () => {
    it.each(['packs/a/assets/tree/LICENSE', 'packs/a/assets/tree/mesh.bin', 'packs/a/.hidden'])(
      'should give the binary content type for %s',
      file => {
        expect(contentType(file)).toBe('application/octet-stream');
      },
    );
  });

  describe('when the packs get a file', () => {
    it('should have a content type for its extension', () => {
      const extensions = new Set(
        (readdirSync(packsDir, { recursive: true, withFileTypes: true }) as Dirent[])
          .filter(entry => entry.isFile())
          .map(entry => extname(entry.name).toLowerCase()),
      );

      expect([...extensions].filter(extension => !(extension in CONTENT_TYPES))).toEqual([]);
    });
  });
});
