import { Composite, Engine, EntityMappingMode, PutComponentOperation, type Entity } from "@dcl/ecs";
import { Name } from "@dcl/ecs/dist/components";
import { ReadWriteByteBuffer } from "@dcl/ecs/dist/serialization/ByteBuffer";

/** The main.crdt `sdk-commands build` writes for a main.composite: the composite instanced with
    its own entity numbers, every component value as a PUT at timestamp 0. The engine loads it as
    the scene's authored state before the scene's code runs. */
export function compositeToMainCrdt(compositeText: string): Uint8Array {
  const engine = Engine();
  Name(engine);
  const composite = Composite.fromJson(JSON.parse(compositeText));
  // A component neither the SDK nor the composite defines (an editor-only one saved without its
  // schema) cannot be serialized; the engine would ignore it anyway.
  composite.components = composite.components.filter(
    (c) => c.name.startsWith("core::") || c.jsonSchema !== undefined || engine.getComponentOrNull(c.name) !== null,
  );
  Composite.instance(
    engine,
    { src: "main.composite", composite },
    { getCompositeOrNull: () => null },
    { entityMapping: { type: EntityMappingMode.EMM_DIRECT_MAPPING, getCompositeEntity: (e) => e as Entity } },
  );
  const value = new ReadWriteByteBuffer();
  const crdt = new ReadWriteByteBuffer();
  for (const component of engine.componentsIter()) {
    for (const [entity, data] of engine.getEntitiesWith(component)) {
      if (!data) continue;
      value.resetBuffer();
      component.schema.serialize(data, value);
      PutComponentOperation.write(entity, 0, component.componentId, value.toBinary(), crdt);
    }
  }
  return crdt.toBinary();
}
