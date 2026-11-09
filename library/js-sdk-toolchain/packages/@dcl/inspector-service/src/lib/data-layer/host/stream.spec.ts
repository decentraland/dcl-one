import { AsyncQueue } from '@well-known-components/pushable-channel';
import { Composite } from '@dcl/ecs';
import { describe, expect, it, vi } from 'vitest';

import { createStream } from './stream';
import { createEngineContext } from './utils/engine';
import { dumpEngineToComposite } from './utils/engine-to-composite';

type Context = ReturnType<typeof createEngineContext>;

/** The CRDT frames an editor engine sends after `write` ran against it. */
async function framesFrom(write: (ctx: Context) => void): Promise<Uint8Array[]> {
  const editor = createEngineContext();
  const frames: Uint8Array[] = [];
  editor.engine.addTransport({
    filter: () => true,
    send: async message => {
      frames.push(message);
    },
  });
  write(editor);
  await editor.engine.update(1);
  return frames;
}

describe('createStream', () => {
  describe('when the editor writes the Nodes component on the root entity', () => {
    it('should apply the write and dump it in the composite', async () => {
      const host = createEngineContext();
      const input = new AsyncQueue<{ data: Uint8Array }>(() => {});
      const output = createStream(input, { engine: host.engine });
      const child = 512;
      const value = [
        { entity: 0, children: [child] },
        { entity: child, children: [] },
      ];
      const frames = await framesFrom(({ engine, components }) => {
        components.Nodes.createOrReplace(engine.RootEntity, { value } as any);
      });
      expect(frames.length).toBeGreaterThan(0);
      for (const data of frames) input.enqueue({ data });

      await vi.waitFor(() => {
        expect(host.components.Nodes.getOrNull(host.engine.RootEntity)).not.toBeNull();
      });
      expect(host.components.Nodes.get(host.engine.RootEntity).value).toMatchObject(value);
      const composite = Composite.toJson(dumpEngineToComposite(host.engine, 'json'));
      const nodes = composite.components.find(component => component.name === 'inspector::Nodes');
      expect(nodes?.data['0']).toMatchObject({ json: { value } });

      input.close();
      output.close();
    });
  });
});
