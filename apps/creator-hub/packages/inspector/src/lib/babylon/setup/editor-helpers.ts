import type { Node, Scene, UtilityLayerRenderer } from '@babylonjs/core';

/** Editor-only nodes, by name: the parcel grid with its north arrow, the spawn points and the ground grid. */
const HELPER_NODE_NAMES = ['layout', 'spawn_points', 'BackgroundPlane'];
const HELPER_MESH_PREFIXES = ['ps-helper-'];
const SELECTION_HIGHLIGHT_LAYER = 'selection_highlight';

const scenesWithHiddenHelpers = new WeakSet<Scene>();

/**
 * True while a capture hides the editor's helpers. Helpers drawn outside the scene graph (the
 * axis gizmo in the viewport corner) read it to skip their own render.
 */
export function areEditorHelpersHidden(scene: Scene): boolean {
  return scenesWithHiddenHelpers.has(scene);
}

/**
 * Hides everything that only exists to edit the scene, so a screenshot shows the scene as
 * players see it: the transform gizmos (drawn by the utility layers), the selection outline,
 * the out-of-bounds boxes, the parcel and ground grids, the spawn points and the particle
 * emitter shapes. Returns a function that puts back exactly what this call hid, leaving alone
 * anything the editor changed itself while the capture ran (a selection change, say).
 */
export function hideEditorHelpers(scene: Scene, utilityLayers: UtilityLayerRenderer[]): () => void {
  const restores: Array<() => void> = [];

  for (const layer of utilityLayers) {
    if (!layer.shouldRender) continue;
    layer.shouldRender = false;
    restores.push(() => {
      if (!layer.shouldRender) layer.shouldRender = true;
    });
  }

  for (const layer of scene.effectLayers) {
    if (layer.name !== SELECTION_HIGHLIGHT_LAYER || !layer.isEnabled) continue;
    layer.isEnabled = false;
    restores.push(() => {
      if (!layer.isEnabled) layer.isEnabled = true;
    });
  }

  const helperNodes: Node[] = [
    ...HELPER_NODE_NAMES.flatMap(name => scene.getNodeByName(name) ?? []),
    ...scene.meshes.filter(mesh =>
      HELPER_MESH_PREFIXES.some(prefix => mesh.name.startsWith(prefix)),
    ),
  ];
  for (const node of helperNodes) {
    if (!node.isEnabled(false)) continue;
    node.setEnabled(false);
    restores.push(() => {
      if (!node.isDisposed() && !node.isEnabled(false)) node.setEnabled(true);
    });
  }

  // The out-of-bounds boxes go off at the renderer, not mesh by mesh: a selection change during
  // the capture sets meshes' `showBoundingBox`, and putting the old values back would undo it.
  const boxRenderer = scene.getBoundingBoxRenderer();
  if (boxRenderer.enabled) {
    boxRenderer.enabled = false;
    restores.push(() => (boxRenderer.enabled = true));
  }

  scenesWithHiddenHelpers.add(scene);
  restores.push(() => scenesWithHiddenHelpers.delete(scene));

  let restored = false;
  return () => {
    if (restored) return;
    restored = true;
    for (const restore of restores) restore();
  };
}
