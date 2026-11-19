import { existsSync, readFileSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';
import { describe, it, expect } from 'vitest';

/**
 * The invisible smart items block the player in game and show nothing there.
 * Their `GltfContainer` loads a model whose every mesh node is named
 * `*_collider`, which explorers load as invisible colliders. The editor shows
 * the visible model through `asset-packs::Placeholder`, which never reaches the
 * game and has no collision.
 */
const assetsDir = join(
  dirname(fileURLToPath(import.meta.url)),
  '..',
  'packs',
  'smart_items',
  'assets',
);

const INVISIBLE_ITEMS = [
  'invisible_wall',
  'invisible_sphere',
  'invisible_dome',
  'invisible_cylinder',
  'invisible_ramp',
];

// ColliderLayer.CL_PHYSICS: blocks the player without catching pointer events.
const CL_PHYSICS = 2;

type Component = { name: string; data: Record<string, { json: Record<string, unknown> }> };

function readComposite(item: string): Component[] {
  const composite = JSON.parse(readFileSync(join(assetsDir, item, 'composite.json'), 'utf8'));
  return composite.components;
}

function rootValue(components: Component[], name: string) {
  return components.find(component => component.name === name)?.data['0']?.json;
}

function localFile(item: string, src: unknown): string {
  expect(typeof src).toBe('string');
  return join(assetsDir, item, (src as string).replace('{assetPath}/', ''));
}

// Asset-pack media is Git LFS; a checkout without it holds pointer files.
function isLfsPointer(buffer: Buffer): boolean {
  return buffer.subarray(0, 24).toString('utf8').startsWith('version https://git-lfs');
}

function readGlbJson(buffer: Buffer) {
  expect(buffer.readUInt32LE(0)).toBe(0x46546c67); // 'glTF'
  const jsonLength = buffer.readUInt32LE(12);
  expect(buffer.readUInt32LE(16)).toBe(0x4e4f534a); // 'JSON'
  return JSON.parse(buffer.subarray(20, 20 + jsonLength).toString('utf8')) as {
    nodes?: Array<{ name?: string; mesh?: number }>;
    materials?: unknown[];
  };
}

describe.each(INVISIBLE_ITEMS)('when loading the %s smart item', item => {
  const components = readComposite(item);
  const gltf = rootValue(components, 'core::GltfContainer');
  const placeholder = rootValue(components, 'asset-packs::Placeholder');

  it('should not hide the entity with a VisibilityComponent', () => {
    expect(rootValue(components, 'core::VisibilityComponent')).toBeUndefined();
  });

  it('should give the collider model physics collision only', () => {
    expect(gltf).toBeDefined();
    expect(gltf!.invisibleMeshesCollisionMask).toBe(CL_PHYSICS);
    expect(gltf!.visibleMeshesCollisionMask).toBe(0);
  });

  it('should show a different model in the editor through a Placeholder', () => {
    expect(placeholder).toBeDefined();
    expect(placeholder!.src).not.toBe(gltf!.src);
    expect(existsSync(localFile(item, placeholder!.src))).toBe(true);
  });

  it('should load a model whose every mesh node is a collider', () => {
    const buffer = readFileSync(localFile(item, gltf!.src));
    // A checkout without the media cannot check the model; CI fetches it and must not skip.
    if (isLfsPointer(buffer)) {
      expect(process.env.REQUIRE_COLLIDER_MODELS, 'the collider model is an LFS pointer').toBe(
        undefined,
      );
      return;
    }
    const json = readGlbJson(buffer);
    const meshNodes = (json.nodes ?? []).filter(node => node.mesh !== undefined);
    expect(meshNodes.length).toBeGreaterThan(0);
    for (const node of meshNodes) expect(node.name).toMatch(/_collider$/);
    expect(json.materials ?? []).toEqual([]);
  });
});
