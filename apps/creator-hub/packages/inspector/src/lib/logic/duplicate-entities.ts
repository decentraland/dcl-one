import type { Entity } from '@dcl/ecs';

import type { SdkContextValue } from '../sdk/context';
import type { GizmoType } from '../utils/gizmo';

/** Clears the selection and returns the gizmo its first entity had, for the copies to keep. */
export function takeSelectionGizmo(
  sdk: SdkContextValue,
  selected: readonly Entity[],
): GizmoType | undefined {
  const gizmo =
    selected.length > 0 ? sdk.components.Selection.getOrNull(selected[0])?.gizmo : undefined;
  sdk.operations.removeSelectedEntities();
  return gizmo;
}

/** Copies `entities` in order, each placed after the previous copy, as one change. */
export function duplicateEntities(
  sdk: SdkContextValue,
  entities: readonly Entity[],
  preferredGizmo: GizmoType | undefined,
  insertAfter: Entity | undefined,
) {
  let after = insertAfter;
  for (const entity of entities) {
    after = sdk.operations.duplicateEntity(entity, preferredGizmo, after);
  }
  void sdk.operations.dispatch();
}
