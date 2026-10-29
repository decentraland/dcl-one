import { expect, it } from "vitest";
import { editorBusEditsProject } from "./bus-edits";

it.each(["writeMaterials", "addComponent", "writeComponents", "moveHierarchy", "pasteEntities", "pasteComponent", "removeComponent", "removeEntities", "addEntity"])("protects unsaved %s edits", method => {
  expect(editorBusEditsProject({ to: "scene", msg: { type: "rpc", id: "1", method, args: [] } })).toBe(true);
});
it.each(["restoreComposite", "exportComposite", "registerContent", "setPlayback", "stopPlayback", "stepPlayback", "copyEntities"])("does not mark %s as an authored edit", method => {
  expect(editorBusEditsProject({ to: "scene", msg: { type: "rpc", id: "1", method, args: [] } })).toBe(false);
});
it("protects direct imports and gizmo edits, but not camera changes", () => {
  expect(editorBusEditsProject({ to: "scene", msg: { type: "load-scene", composite: "{}" } })).toBe(true);
  expect(editorBusEditsProject({ to: "page", msg: { type: "drag-end", transforms: {} } })).toBe(true);
  expect(editorBusEditsProject({ to: "scene", msg: { type: "camera-input", orbitYaw: 1 } })).toBe(false);
});
