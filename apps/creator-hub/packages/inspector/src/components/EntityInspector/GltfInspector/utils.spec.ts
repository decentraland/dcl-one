import { getNode } from '../../../lib/sdk/drag-drop';
import type { TreeNode } from '../../ProjectAssetExplorer/ProjectView';
import type { AssetNodeItem } from '../../ProjectAssetExplorer/types';
import * as utils from './utils';

describe('GltfInspector/utils', () => {
  describe('fromGltf', () => {
    it('should return a "PBGltfContainer" schema', () => {
      const result = {
        src: 'some-path',
        invisibleMeshesCollisionMask: 2,
        visibleMeshesCollisionMask: 0,
      };
      expect(utils.fromGltf(result)).toStrictEqual({
        src: 'some-path',
        invisibleMeshesCollisionMask: '2',
        visibleMeshesCollisionMask: '0',
      });
    });
  });
  describe('toGltf', () => {
    it('should return a "PBGltfContainer" schema', () => {
      const result = {
        src: 'some-path',
        invisibleMeshesCollisionMask: '2',
        visibleMeshesCollisionMask: '0',
      };
      expect(utils.toGltf(result)).toStrictEqual({
        src: 'some-path',
        invisibleMeshesCollisionMask: 2,
        visibleMeshesCollisionMask: 0,
      });
    });
  });
  describe('isValidInput', () => {
    it('should return true when the file is found', () => {
      const assets = {
        basePath: 'root',
        assets: [
          {
            path: 'root/path',
          },
        ],
      };
      expect(utils.isValidInput(assets, 'root/path')).toBe(true);
    });
    it('should return false when the file is not found', () => {
      const assets = {
        basePath: 'root',
        assets: [
          {
            path: 'root/path',
          },
        ],
      };
      expect(utils.isValidInput(assets, 'root/other-path')).toBe(false);
    });
  });
  describe('isAsset', () => {
    it('should return true if value has ".gltf" or ".glb" extension', () => {
      expect(utils.isAsset('test.gltf')).toBe(true);
      expect(utils.isAsset('test.glb')).toBe(true);
      expect(utils.isAsset('test.dll')).toBe(false);
      expect(utils.isAsset('test.gltff')).toBe(false);
    });
  });
  describe('isModel', () => {
    it('should return true if value is an "AssetNodeItem"', () => {
      const validNodeItem: AssetNodeItem = {
        type: 'asset',
        name: 'balloon.glb',
        parent: null,
        asset: { type: 'gltf', src: 'some/path' },
      };
      const invalidNodeItem = { ...validNodeItem, type: 'wrong' } as any;
      expect(utils.isModel(validNodeItem)).toBe(true);
      expect(utils.isModel(invalidNodeItem)).toBe(false);
    });
  });
  describe('getModel', () => {
    it('should get the model from a tree when found', () => {
      const buildAssetNode = (name: string, parent?: TreeNode): TreeNode => {
        if (parent?.children) parent.children.push(name);
        const path = `${name}.gltf`;
        return {
          name: path,
          parent: parent ?? null,
          type: 'asset',
          asset: { src: path, type: 'gltf' },
        } as TreeNode;
      };
      const buildFolderNode = (name: string, parent?: TreeNode): TreeNode => {
        if (parent?.children) parent.children.push(name);
        return {
          name,
          parent: parent ?? null,
          type: 'folder',
          children: [],
        } as TreeNode;
      };

      const root = buildFolderNode('root');
      const folder = buildFolderNode('folder', root);
      const asset = buildAssetNode('asset', folder);
      const tree: Map<string, TreeNode> = new Map([
        ['root', root],
        ['folder', folder],
        ['asset', asset],
      ]);
      const incompleteTree: Map<string, TreeNode> = new Map([
        ['root', root],
        ['folder', folder],
      ]);

      expect(getNode(root, tree, utils.isModel)).toBe(null);
      expect(getNode(folder, tree, utils.isModel)).toBe(asset);
      expect(getNode(asset, tree, utils.isModel)).toBe(asset);
      // need to create a new object since we are memoizing this function...
      expect(getNode({ ...folder }, incompleteTree, utils.isModel)).toBe(null);
    });
  });
  describe('countVisibleTriangles', () => {
    const gltf = {
      nodes: [
        { name: 'body', mesh: 0 },
        { name: 'body_copy', mesh: 0 },
        { name: 'wall_collider', mesh: 1 },
        { name: 'empty' },
      ],
      meshes: [
        {
          primitives: [
            { indices: 0, attributes: { POSITION: 1 } },
            { attributes: { POSITION: 1 } },
            { mode: 5, attributes: { POSITION: 1 } },
            { mode: 1, attributes: { POSITION: 1 } },
          ],
        },
        { primitives: [{ indices: 0 }] },
      ],
      accessors: [{ count: 300 }, { count: 90 }],
    };
    const json = new TextEncoder().encode(JSON.stringify(gltf));

    it('should count the visible triangles of a .gltf, once per node and without colliders', () => {
      // per instance: 300/3 indexed + 90/3 unindexed + (90 - 2) strip, lines ignored
      expect(utils.countVisibleTriangles(json)).toBe(2 * (100 + 30 + 88));
    });

    it('should read the JSON chunk of a .glb', () => {
      const glb = new Uint8Array(20 + json.length);
      const view = new DataView(glb.buffer);
      view.setUint32(0, 0x46546c67, true);
      view.setUint32(4, 2, true);
      view.setUint32(8, glb.length, true);
      view.setUint32(12, json.length, true);
      view.setUint32(16, 0x4e4f534a, true);
      glb.set(json, 20);
      expect(utils.countVisibleTriangles(glb)).toBe(436);
    });

    it('should count a triangle fan as its vertices minus two', () => {
      const fan = {
        nodes: [{ name: 'disc', mesh: 0 }],
        meshes: [{ primitives: [{ mode: 6, attributes: { POSITION: 0 } }] }],
        accessors: [{ count: 12 }],
      };
      expect(utils.countVisibleTriangles(new TextEncoder().encode(JSON.stringify(fan)))).toBe(10);
    });

    it('should leave out a collider named on the mesh, with any casing', () => {
      const named = {
        nodes: [{ name: 'body', mesh: 0 }, { name: 'wall', mesh: 1 }, { mesh: 2 }],
        meshes: [
          { primitives: [{ indices: 0 }] },
          { name: 'wallCollider', primitives: [{ indices: 0 }] },
          { name: 'floor_collider', primitives: [{ indices: 0 }] },
        ],
        accessors: [{ count: 300 }],
      };
      expect(utils.countVisibleTriangles(new TextEncoder().encode(JSON.stringify(named)))).toBe(
        100,
      );
    });

    it('should return null for a file that is not glTF', () => {
      expect(utils.countVisibleTriangles(new Uint8Array([1, 2, 3]))).toBe(null);
    });
  });
});
