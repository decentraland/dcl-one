import { Event } from '../../lib/logic/analytics';
import { trackEntityAction } from '../../lib/logic/entity-analytics';
import type { SdkContextValue } from '../../lib/sdk/context';

export function deleteSelectedEntities(sdk: SdkContextValue) {
  const selectedEntities = sdk.operations.getSelectedEntities();
  selectedEntities.forEach(entity => sdk.operations.removeEntity(entity));
  void sdk.operations.dispatch();
  trackEntityAction(Event.DELETE_ENTITY, selectedEntities, 'shortcut');
}

export function duplicateSelectedEntities(sdk: SdkContextValue) {
  sdk.renderer.camera.setControlEnabled(false);
  const selectedEntities = sdk.operations.getSelectedEntities();
  const preferredGizmo =
    selectedEntities.length > 0
      ? sdk.components.Selection.getOrNull(selectedEntities[0])?.gizmo
      : undefined;
  sdk.operations.removeSelectedEntities();
  let insertAfter =
    selectedEntities.length > 1 ? selectedEntities[selectedEntities.length - 1] : undefined;
  selectedEntities.forEach(entity => {
    const cloned = sdk.operations.duplicateEntity(entity, preferredGizmo, insertAfter);
    insertAfter = cloned;
  });
  void sdk.operations.dispatch();
  setTimeout(() => {
    sdk.renderer.camera.setControlEnabled(true);
  }, 100);
  trackEntityAction(Event.DUPLICATE_ENTITY, selectedEntities, 'shortcut');
}
