import { describe, it, expect } from 'vitest';
import type { IEngine } from '@dcl/ecs';
import type { AssetData } from '@dcl/asset-packs';

import { checkAssetCompatibility } from './compatibility';

const engine = { getComponent: () => ({}) } as unknown as IEngine;

const scriptComposite = (params: Record<string, unknown>): AssetData['composite'] => ({
  version: 1,
  components: [
    {
      name: 'asset-packs::Script',
      data: {
        '0': {
          json: {
            value: [
              {
                path: '{assetPath}/AdminTools.ts',
                priority: 0,
                layout: JSON.stringify({ params }),
              },
            ],
          },
        },
      },
    },
  ],
});

describe('checkAssetCompatibility', () => {
  it('accepts a script whose params this editor can render', () => {
    const composite = scriptComposite({
      authorizedAdminUsers: {
        type: 'object',
        value: { me: true },
        fields: { me: { type: 'boolean', value: false } },
      },
    });
    expect(checkAssetCompatibility(composite, engine)).toEqual({ compatible: true });
  });

  it('rejects a script that declares a param type this editor does not know', () => {
    const composite = scriptComposite({
      cfg: {
        type: 'object',
        value: {},
        fields: { spin: { type: 'quaternion', value: {} } },
      },
    });
    expect(checkAssetCompatibility(composite, engine)).toEqual({
      compatible: false,
      incompatibleComponents: [
        { name: 'AdminTools.ts: cfg.spin', reason: 'unsupported-script-param' },
      ],
    });
  });

  it('ignores a layout that is not valid JSON', () => {
    const composite = scriptComposite({});
    composite.components[0].data['0'].json.value[0].layout = '{not json';
    expect(checkAssetCompatibility(composite, engine)).toEqual({ compatible: true });
  });

  it('treats a malformed Script entry as having no unsupported params instead of throwing', () => {
    const composite = scriptComposite({});
    (composite.components[0].data as Record<string, unknown>)['1'] = null;
    composite.components[0].data['0'].json.value[0].layout = JSON.stringify({
      params: {},
      actions: { not: 'an array' },
    });
    expect(checkAssetCompatibility(composite, engine)).toEqual({ compatible: true });
  });

  describe('when the asset declares the SDK version it needs', () => {
    const composite = scriptComposite({});
    const check = (installed: string | null | undefined) =>
      checkAssetCompatibility(composite, engine, { required: '7.32.0', installed });

    it('rejects it in a scene on an older SDK', () => {
      expect(check('7.31.5')).toEqual({
        compatible: false,
        incompatibleComponents: [{ name: '@dcl/sdk 7.32.0', reason: 'sdk-too-old' }],
      });
    });

    it('accepts it in a scene on that SDK or a newer one', () => {
      expect(check('7.32.0')).toEqual({ compatible: true });
      expect(check('7.33.1')).toEqual({ compatible: true });
      expect(check('8.0.0')).toEqual({ compatible: true });
    });

    it('compares the numbers, not the text', () => {
      expect(check('7.4.0')).toMatchObject({ compatible: false });
      expect(check('7.100.0')).toEqual({ compatible: true });
    });

    it('counts a pre-release of that SDK as that SDK', () => {
      expect(check('7.32.0-123456.commit-abc1234')).toEqual({ compatible: true });
    });

    it('accepts it when the SDK of the scene is unknown', () => {
      expect(check(null)).toEqual({ compatible: true });
      expect(check(undefined)).toEqual({ compatible: true });
      expect(check('next')).toEqual({ compatible: true });
    });
  });

  it('ignores the SDK of the scene for an asset that declares no minimum', () => {
    expect(checkAssetCompatibility(scriptComposite({}), engine, { installed: '7.0.0' })).toEqual({
      compatible: true,
    });
  });
});
