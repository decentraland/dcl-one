import type { BusEnvelope, PageToSceneMessage } from "./bus-protocol";

const MUTATING_MESSAGES = new Set<PageToSceneMessage["type"]>([
  "add-entity", "set-component", "add-component", "delete-component",
  "entity-deleted", "component-written", "load-scene",
]);
const MUTATING_RPCS = new Set([
  "moveHierarchy", "pasteEntities", "pasteComponent", "writeComponents",
  "writeMaterials", "removeComponent", "removeEntities", "addEntity", "addComponent",
]);

/** Pessimistically protect edits while their acknowledgement is in flight.
 * Hydration, export, camera and playback traffic must not dirty a project. */
export function editorBusEditsProject(envelope: BusEnvelope): boolean {
  const { to, msg } = envelope;
  if (!msg || typeof msg !== "object") return false;
  if (to === "page") return msg.type === "drag-end";
  if (to !== "scene") return false;
  return MUTATING_MESSAGES.has(msg.type as PageToSceneMessage["type"]) ||
    (msg.type === "rpc" && MUTATING_RPCS.has(msg.method));
}
