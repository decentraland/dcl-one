import { Engine, PutComponentOperation, Schemas } from "@dcl/ecs";
import * as components from "@dcl/ecs/dist/components";
import { ReadWriteByteBuffer } from "@dcl/ecs/dist/serialization/ByteBuffer";
import { expect, test } from "vitest";

import { compositeToMainCrdt } from "./composite-crdt";
import { buildTemplateCompositeText } from "./template-composites";

function messages(bytes: Uint8Array) {
  const buf = new ReadWriteByteBuffer(bytes);
  const out = [];
  for (let m = PutComponentOperation.read(buf); m; m = PutComponentOperation.read(buf)) out.push(m);
  return out;
}

test("a template composite becomes the main.crdt the engine starts the scene from", () => {
  const crdt = compositeToMainCrdt(buildTemplateCompositeText("tower-defense")!);
  const engine = Engine();
  const Transform = components.Transform(engine);
  const GltfContainer = components.GltfContainer(engine);
  const Name = components.Name(engine);
  const puts = messages(crdt);
  expect(puts.every((m) => m.timestamp === 0)).toBe(true);
  const value = (componentId: number, entity: number) => {
    const m = puts.find((p) => p.componentId === componentId && p.entityId === entity)!;
    return m.data;
  };
  const ground = Transform.schema.deserialize(new ReadWriteByteBuffer(value(Transform.componentId, 513)));
  expect(ground.position).toEqual({ x: 8, y: 0, z: 8 });
  const model = GltfContainer.schema.deserialize(new ReadWriteByteBuffer(value(GltfContainer.componentId, 513)));
  expect(model.src).toBe("assets/imported/template-assets/QmWM8PmLyebMayuY3YkKLX9mZoAAWb9kk69hueAyTwfkvq.glb");
  expect(Name.schema.deserialize(new ReadWriteByteBuffer(value(Name.componentId, 513))).value).toBe("Stone Ground");
});

test("a component saved without a schema the SDK lacks is left out instead of failing the scene", () => {
  const composite = JSON.stringify({
    version: 1,
    components: [
      { name: "editor::Unknown", data: { 512: { json: { anything: true } } } },
      { name: "core::Transform", data: { 512: { json: { position: { x: 1, y: 2, z: 3 }, rotation: { x: 0, y: 0, z: 0, w: 1 }, scale: { x: 1, y: 1, z: 1 }, parent: 0 } } } },
    ],
  });
  const puts = messages(compositeToMainCrdt(composite));
  expect(puts.map((m) => m.entityId)).toEqual([512]);
});

test("a composite saved under the legacy smart-item namespace instances the current components", () => {
  const counterSchema = {
    type: "object",
    properties: { id: { type: "number", serializationType: "float32" }, value: { type: "integer", serializationType: "int32" } },
    serializationType: "map",
  };
  const composite = JSON.stringify({
    version: 1,
    components: [
      { name: "core::Transform", data: { 512: { json: { position: { x: 1, y: 2, z: 3 }, rotation: { x: 0, y: 0, z: 0, w: 1 }, scale: { x: 1, y: 1, z: 1 }, parent: 0 } } } },
      { name: "asset-packs::Counter", jsonSchema: counterSchema, data: { 512: { json: { id: 512, value: 3 } } } },
    ],
  });
  const engine = Engine();
  const Counter = engine.defineComponent("smart-items::Counter", { id: Schemas.Number, value: Schemas.Int });
  const legacyId = engine.defineComponent("asset-packs::Counter", { id: Schemas.Number, value: Schemas.Int }).componentId;
  const puts = messages(compositeToMainCrdt(composite));
  expect(puts.some((m) => m.componentId === legacyId)).toBe(false);
  const put = puts.find((m) => m.componentId === Counter.componentId && m.entityId === 512)!;
  expect(Counter.schema.deserialize(new ReadWriteByteBuffer(put.data))).toEqual({ id: 512, value: 3 });
});
