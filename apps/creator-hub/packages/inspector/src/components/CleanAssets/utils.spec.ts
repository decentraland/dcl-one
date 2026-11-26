import { describe, expect, it } from 'vitest';

import type { SdkContextValue } from '../../lib/sdk/context';
import { collectUsedAssets } from './utils';

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
