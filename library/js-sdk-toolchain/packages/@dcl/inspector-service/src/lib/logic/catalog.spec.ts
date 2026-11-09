import { afterEach, describe, expect, it, vi } from 'vitest';

import { isFileOnly } from './catalog';

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

describe('fetchLatestCatalog', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    vi.resetModules();
  });

  describe('when the content server answers', () => {
    it('should rename the legacy component names of every composite', async () => {
      const composite = { version: 1, components: [{ name: 'asset-packs::Actions', data: {} }] };
      const asset = { id: 'a', name: 'A', category: 'other', tags: [], contents: {}, composite };
      const assetPacks = [{ id: 'p', name: 'P', thumbnail: '', assets: [asset] }];
      vi.stubGlobal('fetch', vi.fn(async () => ({ ok: true, json: async () => ({ assetPacks }) })));
      const { fetchLatestCatalog, getCatalogSource } = await import('./catalog');
      const packs = await fetchLatestCatalog();
      expect(packs[0].assets[0].composite.components[0].name).toBe('smart-items::Actions');
      expect(getCatalogSource()).toBe('remote');
    });
  });

  describe('when the content server cannot be reached', () => {
    it('should resolve to an empty catalog and report it', async () => {
      vi.stubGlobal('fetch', vi.fn(async () => Promise.reject(new Error('offline'))));
      vi.spyOn(console, 'warn').mockImplementation(() => {});
      const { fetchLatestCatalog, getCatalogSource } = await import('./catalog');
      await expect(fetchLatestCatalog()).resolves.toEqual([]);
      expect(getCatalogSource()).toBe('none');
    });
  });
});
