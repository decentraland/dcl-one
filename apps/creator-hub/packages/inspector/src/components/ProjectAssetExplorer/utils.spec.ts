import { Filter } from './Filters/types';
import { getThumbnailHashNameForAsset } from '../../lib/utils/hash';
import * as utils from './utils';

import type { AssetNodeFolder, AssetNodeItem } from './types';

describe('ProjectAssetExplorer/utils', () => {
  describe('getFullNodePath', () => {
    it('should return full node path given a child', () => {
      const parentA = { name: 'parentA', parent: null };
      const parentB = { name: 'parentB', parent: parentA };
      const child = { name: 'file.gltf', parent: parentB } as AssetNodeItem;
      expect(utils.getFullNodePath(child)).toBe('/parentA/parentB/file.gltf');
    });
    it('should return child name if node has no parent', () => {
      const child = { name: 'file.gltf', parent: null } as AssetNodeItem;
      expect(utils.getFullNodePath(child)).toBe('/file.gltf');
    });
  });
  describe('isAssetNode', () => {
    it('should return true when node type "asset"', () => {
      const node: AssetNodeItem = {
        name: 'some-name',
        parent: null,
        type: 'asset',
        asset: {
          src: 'some-file.glb',
          type: 'gltf',
        },
      };
      expect(utils.isAssetNode(node)).toBe(true);
    });
    it('should return false when node type is different from "asset"', () => {
      const node: AssetNodeFolder = {
        name: 'some-name',
        parent: null,
        type: 'folder',
        children: [],
      };
      expect(utils.isAssetNode(node)).toBe(false);
    });
  });
  describe('mapAssetTypeToFilter', () => {
    describe('when a model file name ends in _emote', () => {
      it('should group it under Avatar Animations', () => {
        const asset = { type: 'gltf' as const, src: 'asset-packs/dance/Avatar_Dance_emote.glb' };
        expect(utils.mapAssetTypeToFilter(asset)).toBe(Filter.Emotes);
      });
    });

    describe('when a model file name does not end in _emote', () => {
      it('should group it under Models', () => {
        const asset = { type: 'gltf' as const, src: 'models/emote_stage.glb' };
        expect(utils.mapAssetTypeToFilter(asset)).toBe(Filter.Models);
      });
    });

    describe('when a model file name ends in a wearable category', () => {
      it('should group it under Wearables', () => {
        const asset = { type: 'gltf' as const, src: 'assets/wearables/denim_upper_body.glb' };
        expect(utils.mapAssetTypeToFilter(asset)).toBe(Filter.Wearables);
      });
    });

    describe('when a model file is named like a category with no name before it', () => {
      it('should group it under Models', () => {
        const asset = { type: 'gltf' as const, src: 'models/hat.glb' };
        expect(utils.mapAssetTypeToFilter(asset)).toBe(Filter.Models);
      });
    });

    describe('when a non-model file name ends in _emote', () => {
      it('should not group it under Avatar Animations', () => {
        const asset = { type: 'image' as const, src: 'images/wave_emote.png' };
        expect(utils.mapAssetTypeToFilter(asset)).toBe(Filter.Images);
      });
    });
  });
});

describe('findAssetThumbnails', () => {
  const bytes = (value: number) => new Uint8Array([value]);

  it('should find a thumbnail by the hash of its model path', async () => {
    const name = await getThumbnailHashNameForAsset('models/jetpack_suit.glb');
    const map = await utils.findAssetThumbnails(
      ['models/jetpack_suit.glb'],
      [{ path: `thumbnails/${name}`, content: bytes(1) }],
    );
    expect(map.get('models/jetpack_suit.glb')).toEqual(bytes(1));
  });

  it("should fall back to an older scene's thumbnail named after the file", async () => {
    const map = await utils.findAssetThumbnails(
      ['models/jetpack_suit.glb'],
      [{ path: 'thumbnails/jetpack_suit.png', content: bytes(2) }],
    );
    expect(map.get('models/jetpack_suit.glb')).toEqual(bytes(2));
  });

  it('should leave out a model with no thumbnail', async () => {
    const map = await utils.findAssetThumbnails(['models/tree.glb'], []);
    expect(map.has('models/tree.glb')).toBe(false);
  });
});
