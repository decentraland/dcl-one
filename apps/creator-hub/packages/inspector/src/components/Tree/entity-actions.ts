import type { Entity } from '@dcl/ecs';

import { Event } from '../../lib/logic/analytics';
import { trackEntityAction } from '../../lib/logic/entity-analytics';
import type { SdkContextValue } from '../../lib/sdk/context';
import type { GizmoType } from '../../lib/utils/gizmo';

// The tree is generic: file trees hold strings and the UI Designer tree holds node objects, so only
// a numeric value is a scene entity worth tracking.
const isSceneEntity = (value: unknown): value is Entity => typeof value === 'number';

export function removeFromTree<T>(
  sdk: SdkContextValue | null,
  value: T,
  onRemove: (value: T) => void,
) {
  if (typeof value === 'string' || !sdk) {
    onRemove(value);
    return;
  }
  const selectedEntities = sdk.operations.getSelectedEntities();
  const removed =
    selectedEntities.length > 1
      ? (selectedEntities.filter(entity => typeof entity === typeof value) as T[])
      : [value];
  removed.forEach(item => onRemove(item));
  if (isSceneEntity(value)) {
    trackEntityAction(Event.DELETE_ENTITY, removed as Entity[], 'context_menu');
  }
}

export function duplicateInTree<T>(
  sdk: SdkContextValue | null,
  value: T,
  onDuplicate: (value: T, preferredGizmo?: GizmoType) => void,
) {
  if (typeof value === 'string' || !sdk) {
    onDuplicate(value);
    return;
  }
  const selectedEntities = sdk.operations.getSelectedEntities();
  const preferredGizmo =
    selectedEntities.length > 0
      ? sdk.components.Selection.getOrNull(selectedEntities[0])?.gizmo
      : undefined;
  sdk.operations.removeSelectedEntities();
  let duplicated: Entity[];
  if (selectedEntities.length > 1) {
    duplicated = selectedEntities.filter(entity => typeof entity === typeof value);
    let insertAfter = selectedEntities[selectedEntities.length - 1];
    duplicated.forEach(entity => {
      insertAfter = sdk.operations.duplicateEntity(entity, preferredGizmo, insertAfter);
    });
    void sdk.operations.dispatch();
  } else {
    duplicated = isSceneEntity(value) ? [value] : [];
    onDuplicate(value, preferredGizmo);
  }
  if (isSceneEntity(value)) {
    trackEntityAction(Event.DUPLICATE_ENTITY, duplicated, 'context_menu');
  }
}
