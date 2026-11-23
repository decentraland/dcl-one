import { Event } from '../../lib/logic/analytics';
import { duplicateEntities, takeSelectionGizmo } from '../../lib/logic/duplicate-entities';
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
  const preferredGizmo = takeSelectionGizmo(sdk, selectedEntities);
  const insertAfter =
    selectedEntities.length > 1 ? selectedEntities[selectedEntities.length - 1] : undefined;
  duplicateEntities(sdk, selectedEntities, preferredGizmo, insertAfter);
  setTimeout(() => {
    sdk.renderer.camera.setControlEnabled(true);
  }, 100);
  trackEntityAction(Event.DUPLICATE_ENTITY, selectedEntities, 'shortcut');
}
