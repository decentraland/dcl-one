import { describe, expect, it } from 'vitest';

import type { SdkContextValue } from '../../lib/sdk/context';
import type { IDataLayer } from '../../redux/data-layer';
import { collectUsedAssets, findAssetsNamedInCode, scanForUnusedAssets } from './utils';

// The engine is reduced to what the scan reads: the component definitions and, for each one, the
// entities that carry it.
function sdkWith(byComponent: Map<object, unknown[]>, components: Record<string, object>) {
  return {
    components,
    engine: {
      *getEntitiesWith(component: object) {
        for (let i = 0; i < (byComponent.get(component)?.length ?? 0); i++) yield [512 + i];
      },
    },
  } as unknown as SdkContextValue;
}

function withValues(values: unknown[]) {
  const component = { getOrNull: (entity: number) => values[entity - 512] ?? null };
  return { component, values };
}

describe('collectUsedAssets', () => {
  describe('when a Script param holds a file picked from the scene', () => {
    const scriptLayout = (value: unknown) =>
      JSON.stringify({ params: { skyboxTexture: { type: 'string', format: 'image', value } } });

    function collect(layout: string) {
      const script = withValues([
        { value: [{ path: 'assets/skybox/skybox-controller.ts', priority: 0, layout }] },
      ]);
      const components = { Script: script.component, Actions: {} };
      return collectUsedAssets(sdkWith(new Map([[script.component, script.values]]), components));
    }

    it('should count the file as used', () => {
      expect(collect(scriptLayout('assets/scene/Images/Sky.png'))).toContain(
        'assets/scene/images/sky.png',
      );
    });

    it('should skip an empty file param', () => {
      expect([...collect(scriptLayout(''))]).not.toContain('');
    });

    it('should skip a layout that does not parse', () => {
      expect(() => collect('{not json')).not.toThrow();
    });
  });
});

describe('findAssetsNamedInCode', () => {
  const paths = [
    'assets/scene/ui/Overlay.png',
    'assets/scene/ui/button.png',
    'assets/scene/ui/red-button.png',
    'assets/scene/models/monster3.glb',
  ];

  it('should find a file named by its full path, whatever the case', () => {
    const code = "uiBackground: { texture: { src: 'assets/scene/ui/overlay.png' } }";
    expect(findAssetsNamedInCode([code], paths)).toEqual(['assets/scene/ui/Overlay.png']);
  });

  it('should find a file whose folder and name are joined in the code', () => {
    const code = "const UI = 'assets/scene/ui/'\nconst src = UI + 'button.png'";
    expect(findAssetsNamedInCode([code], paths)).toEqual(['assets/scene/ui/button.png']);
  });

  it('should not match a name inside a longer file name', () => {
    const code = "const src = 'assets/scene/ui/red-button.png'";
    expect(findAssetsNamedInCode([code], paths)).toEqual(['assets/scene/ui/red-button.png']);
  });

  it('should not find a name built while the code runs', () => {
    const code = "const src = 'assets/scene/models/monster' + level + '.glb'";
    expect(findAssetsNamedInCode([code], paths)).toEqual([]);
  });

  it('should find names in a large source without walking it once per file', () => {
    const many = Array.from({ length: 2000 }, (_, i) => `assets/scene/ui/icon-${i}.png`);
    const code = `${'const filler = 1;'.repeat(50000)}\nconst src = 'icon-1999.png'`;
    expect(findAssetsNamedInCode([code], many)).toEqual(['assets/scene/ui/icon-1999.png']);
  });

  it('should find a file with no extension', () => {
    expect(findAssetsNamedInCode(["read('assets/LICENSE')"], ['assets/LICENSE'])).toEqual([
      'assets/LICENSE',
    ]);
  });

  it('should find nothing when there is no code', () => {
    expect(findAssetsNamedInCode([], paths)).toEqual([]);
  });
});

describe('scanForUnusedAssets', () => {
  const files = [
    { path: 'assets/scene/ui/overlay.png', size: 10 },
    { path: 'assets/scene/ui/spare.png', size: 20 },
  ];
  const emptySdk = sdkWith(new Map(), {});
  const code = new TextEncoder().encode("const src = 'assets/scene/ui/overlay.png'");

  function dataLayerWith(overrides: Record<string, unknown>) {
    return {
      getFilesSizes: async ({ path }: { path: string }) => ({
        files: path === 'src' ? [{ path: 'src/ui.tsx', size: code.length }] : files,
      }),
      getFilesList: async ({ paths }: { paths: string[] }) => ({
        files: paths.map(path => ({ path, content: code, success: true })),
      }),
      ...overrides,
    } as unknown as IDataLayer;
  }

  const unused = (assets: { path: string; unused: boolean }[]) =>
    assets.filter(asset => asset.unused).map(asset => asset.path);

  describe('when the code is read', () => {
    it('should keep the file the code names and report the code as scanned', async () => {
      const result = await scanForUnusedAssets(emptySdk, files, dataLayerWith({}));
      expect(result.codeScanned).toBe(true);
      expect(unused(result.assets)).toEqual(['assets/scene/ui/spare.png']);
      expect(result.assets.find(asset => !asset.unused)?.usedByCode).toBe(true);
    });
  });

  describe('when a code file cannot be read', () => {
    it('should not report the code as scanned', async () => {
      const getFilesList = async ({ paths }: { paths: string[] }) => ({
        files: paths.map(path => ({ path, content: new Uint8Array(0), success: false })),
      });
      const result = await scanForUnusedAssets(emptySdk, files, dataLayerWith({ getFilesList }));
      expect(result.codeScanned).toBe(false);
    });
  });

  describe('when listing the code fails', () => {
    it('should still list the assets and not report the code as scanned', async () => {
      const getFilesSizes = async () => {
        throw new Error('listing failed');
      };
      const result = await scanForUnusedAssets(emptySdk, files, dataLayerWith({ getFilesSizes }));
      expect(result.codeScanned).toBe(false);
      expect(result.assets).toHaveLength(2);
    });
  });

  describe('when the scene has no code', () => {
    it('should not report the code as scanned', async () => {
      const getFilesSizes = async () => ({ files });
      const result = await scanForUnusedAssets(emptySdk, files, dataLayerWith({ getFilesSizes }));
      expect(result.codeScanned).toBe(false);
    });
  });
});
