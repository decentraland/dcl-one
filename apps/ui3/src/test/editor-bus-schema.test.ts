import { describe, expect, test } from "vitest";
import { SceneToPageMessageSchema } from "../generated/editor-bus-schemas";
import { SCENE_BRIDGE_VERSION } from "../generated/editor-bus";

const sceneReady = {
  type: "scene-ready",
  bridge: SCENE_BRIDGE_VERSION,
  scene: {
    hash: "bafk",
    title: "Genesis Plaza",
    parcels: [{ x: 0, y: 0 }],
    isPortable: false,
    isBroken: false,
    isBlocked: false,
    isSuper: false,
    sdkVersion: "7",
  },
  frozen: false,
  tool: "select",
  orientGlobal: false,
  pivotEach: false,
  selected: [],
  active: null,
};

describe("editor bus scene-to-page validation", () => {
  test("accepts valid messages and unknown extra keys; rejects drifted shapes", () => {
    const cases: [string, unknown, boolean][] = [
      ["valid scene-ready", sceneReady, true],
      ["valid rpc-reply", { type: "rpc-reply", id: "t-1", ok: true, result: 3 }, true],
      ["scene-ready with an unknown extra key", { ...sceneReady, futureField: 1 }, true],
      ["scene-ready missing sdkVersion", { ...sceneReady, scene: { ...sceneReady.scene, sdkVersion: undefined } }, false],
      ["valid authored checkpoint", { type: "authoring-checkpoint", composite: '{"version":1,"components":[]}', runtime: "b64-project:42" }, true],
      ["checkpoint missing runtime", { type: "authoring-checkpoint", composite: "{}" }, false],
      ["checkpoint object instead of serialized composite", { type: "authoring-checkpoint", composite: {}, runtime: "b64-project:42" }, false],
      ["scene-ready parcels as tuples instead of {x,y}", { ...sceneReady, scene: { ...sceneReady.scene, parcels: [[0, 0]] } }, false],
      ["rpc-reply with ok renamed to success", { type: "rpc-reply", id: "t-1", success: true }, false],
      ["unknown message type", { type: "brand-new", payload: 1 }, false],
      ["selection with selected as a string", { type: "selection", selected: "a", active: null }, false],
    ];
    const verdicts = cases.map(([name, value]) => [name, SceneToPageMessageSchema.safeParse(value).success]);
    expect(verdicts).toEqual(cases.map(([name, , shouldPass]) => [name, shouldPass]));
  });
});
