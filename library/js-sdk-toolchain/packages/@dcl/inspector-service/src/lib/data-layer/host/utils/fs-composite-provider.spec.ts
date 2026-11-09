import { Composite, Schemas } from '@dcl/ecs';
import { componentNumberFromName } from '@dcl/ecs/dist/components/component-number';
import { ReadWriteByteBuffer } from '@dcl/ecs/dist/serialization/ByteBuffer';
import { describe, expect, it } from 'vitest';

import { createFileSystemInterface } from '../../../logic/file-system-interface';
import { createInMemoryStorage } from '../../../logic/storage/in-memory';
import { createFsCompositeProvider } from './fs-composite-provider';

const legacyComposite = {
  version: 1,
  components: [
    { name: 'asset-packs::Actions', data: { '512': { json: { id: 1, value: [] } } } },
    { name: 'core::Transform', data: { '512': { json: { position: { x: 1, y: 2, z: 3 } } } } },
  ],
};

async function load(files: Record<string, Buffer>, path: string) {
  const provider = await createFsCompositeProvider(createFileSystemInterface(createInMemoryStorage(files)));
  return provider.getCompositeOrNull(path)!.composite;
}

async function loadedNames(files: Record<string, Buffer>, path: string) {
  return (await load(files, path)).components.map(component => component.name);
}

const SYNC_SCHEMA = Schemas.Map({ componentIds: Schemas.Array(Schemas.Int64) });

function syncBytes(componentIds: number[]): Uint8Array {
  const buffer = new ReadWriteByteBuffer();
  SYNC_SCHEMA.serialize({ componentIds }, buffer);
  return buffer.toBinary();
}

describe('createFsCompositeProvider', () => {
  describe('when a composite on disk carries legacy component names', () => {
    it('should rename them in the loaded json composite', async () => {
      const path = 'assets/scene/main.composite';
      const names = await loadedNames({ [path]: Buffer.from(JSON.stringify(legacyComposite)) }, path);
      expect(names).toEqual(['smart-items::Actions', 'core::Transform']);
    });

    it('should rename them in the loaded binary composite', async () => {
      const path = 'assets/scene/main.composite.bin';
      const bytes = Composite.toBinary(Composite.fromJson(legacyComposite));
      const names = await loadedNames({ [path]: Buffer.from(bytes) }, path);
      expect(names).toEqual(['smart-items::Actions', 'core::Transform']);
    });
  });

  describe('when a binary composite on disk syncs legacy component numbers', () => {
    it('should remap them inside the binary Sync-Components entry', async () => {
      const path = 'assets/scene/main.composite.bin';
      const legacyActions = componentNumberFromName('asset-packs::Actions');
      const actions = componentNumberFromName('smart-items::Actions');
      const definition = {
        version: 1,
        components: [
          {
            name: 'core-schema::Sync-Components',
            jsonSchema: undefined,
            data: new Map([[512, { data: { $case: 'binary', binary: syncBytes([legacyActions, 7]) } }]]),
          },
        ],
      } as unknown as Composite.Definition;

      const loaded = await load({ [path]: Buffer.from(Composite.toBinary(definition)) }, path);

      const entry = loaded.components[0].data.get(512 as any)!.data;
      expect(entry.$case).toBe('binary');
      const { componentIds } = SYNC_SCHEMA.deserialize(new ReadWriteByteBuffer((entry as any).binary));
      expect(componentIds).toEqual([actions, 7]);
    });
  });
});
