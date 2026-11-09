import { Composite, type Entity } from '@dcl/ecs';
import { describe, expect, it } from 'vitest';

import { renameLegacyComponentName, renameLegacyComposite } from './legacy';

type JsonComposite = { version: number; components: { name: string; data: Record<string, { json: any }> }[] };

function legacyComposite(): JsonComposite {
  return {
    version: 1,
    components: [
      { name: 'asset-packs::Actions', data: { '512': { json: { id: 1, value: [] } } } },
      { name: 'smart-items::States', data: { '512': { json: { id: 2, value: ['a'] } } } },
      { name: 'core::Transform', data: {} },
      {
        name: 'inspector::Config',
        data: {
          '512': {
            json: {
              isBasicViewEnabled: true,
              componentName: 'asset-packs::Actions',
              fields: [
                { name: 'Open', type: 'asset-packs::Actions' },
                { name: 'Pointer', type: 'core::PointerEvents' },
              ],
              sections: [{ id: 'main', items: [{ component: 'asset-packs::VideoScreen', widget: 'text' }] }],
            },
          },
        },
      },
      {
        name: 'core-schema::Sync-Components',
        data: { '512': { json: { value: ['asset-packs::Actions', 'core::Transform'] } } },
      },
    ],
  };
}

const RENAMED = [
  'smart-items::Actions',
  'smart-items::States',
  'core::Transform',
  'inspector::Config',
  'core-schema::Sync-Components',
];

describe('renameLegacyComponentName', () => {
  it('should swap the asset-packs prefix for smart-items and leave other names alone', () => {
    expect(renameLegacyComponentName('asset-packs::Actions')).toBe('smart-items::Actions');
    expect(renameLegacyComponentName('smart-items::Actions')).toBe('smart-items::Actions');
    expect(renameLegacyComponentName('core::Transform')).toBe('core::Transform');
  });
});

describe('renameLegacyComposite', () => {
  describe('when a composite json carries legacy component names', () => {
    it('should rename the component blocks', () => {
      const json = renameLegacyComposite(legacyComposite());
      expect(json.components.map(component => component.name)).toEqual(RENAMED);
    });

    it('should rename the components the basic-view config names', () => {
      const config = renameLegacyComposite(legacyComposite()).components[3].data['512'].json;
      expect(config).toEqual({
        isBasicViewEnabled: true,
        componentName: 'smart-items::Actions',
        fields: [
          { name: 'Open', type: 'smart-items::Actions' },
          { name: 'Pointer', type: 'core::PointerEvents' },
        ],
        sections: [{ id: 'main', items: [{ component: 'smart-items::VideoScreen', widget: 'text' }] }],
      });
    });

    it('should rename the synced component names', () => {
      const sync = renameLegacyComposite(legacyComposite()).components[4].data['512'].json;
      expect(sync.value).toEqual(['smart-items::Actions', 'core::Transform']);
    });
  });

  describe('when the composite is a decoded definition', () => {
    it('should rename the names inside the mapped component data', () => {
      const definition = renameLegacyComposite(Composite.fromJson(legacyComposite()));
      expect(definition.components.map(component => component.name)).toEqual(RENAMED);
      const config = definition.components[3].data.get(512 as Entity)!.data;
      expect(config?.$case === 'json' && config.json.fields[0].type).toBe('smart-items::Actions');
    });
  });
});
