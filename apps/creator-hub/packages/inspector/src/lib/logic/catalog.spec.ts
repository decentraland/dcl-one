import { describe, expect, it } from 'vitest';

import { type Asset, getAssetsByCategory, isFileOnly } from './catalog';

describe('isFileOnly', () => {
  describe('when the asset has no composite components', () => {
    it('should be file-only, so placing it adds no entity', () => {
      expect(isFileOnly({ composite: { version: 1, components: [] } })).toBe(true);
    });
  });

  describe('when the asset has a GltfContainer', () => {
    it('should not be file-only', () => {
      expect(
        isFileOnly({
          composite: {
            version: 1,
            components: [{ name: 'core::GltfContainer', data: { 0: { json: { src: 'a.glb' } } } }],
          },
        }),
      ).toBe(false);
    });
  });
});

describe('getAssetsByCategory', () => {
  const asset = (name: string, category?: string) => ({ name, category }) as unknown as Asset;

  describe('when two assets spell one category with different capitals', () => {
    it('should put them in one group', () => {
      const groups = getAssetsByCategory([asset('a', 'Text'), asset('b', 'text')]);
      expect(groups.get('text')?.map($ => $.name)).toEqual(['a', 'b']);
      expect(groups.has('Text')).toBe(false);
    });
  });

  describe('when an asset has no category', () => {
    it('should land in "other"', () => {
      expect(getAssetsByCategory([asset('a')]).get('other')?.length).toBe(1);
    });
  });

  describe('when the assets include ground tiles', () => {
    it('should list ground first', () => {
      const groups = getAssetsByCategory([asset('a', 'decorations'), asset('b', 'ground')]);
      const filled = Array.from(groups).filter(([, assets]) => assets.length > 0);
      expect(filled.map(([name]) => name)).toEqual(['ground', 'decorations']);
    });
  });
});
