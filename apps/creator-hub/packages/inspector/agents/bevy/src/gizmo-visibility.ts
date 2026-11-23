/**
 * When the editor gizmo (the entity transform gizmo, and the spawn-point handle
 * that rides the same machinery) is drawn and reacts to the pointer.
 *
 * Pure and SDK-free so the inspector's unit tests can import it
 * (`src/lib/renderer/bevy/agent-gizmo-visibility.spec.ts`); this project has no
 * test runner of its own.
 */
export interface GizmoVisibilityState {
  /** Viewport editing (the toolbar "Interact" toggle off). */
  editingEnabled: boolean;
  /** Something the gizmo can anchor to: an entity selection or a spawn point. */
  hasTarget: boolean;
  /** The gizmo mode draws handles (translate, rotate, scale, free). */
  modeDrawsHandles: boolean;
}

/**
 * True while the gizmo is anchored to its target, drawn and hover-highlighted.
 *
 * With editing off (Interact on) the gizmo is not drawn even though the selection
 * and spawn-point state keep updating, so re-enabling editing draws the latest
 * selection in its current mode. The selection outline (`/highlight`) is not
 * governed by this: it is selection feedback, not a handle.
 */
export function isGizmoActive(state: GizmoVisibilityState): boolean {
  return state.editingEnabled && state.hasTarget && state.modeDrawsHandles;
}
