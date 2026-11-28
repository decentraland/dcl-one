import { ComponentType } from '@dcl/ecs';
import type { ComponentOperation } from '../component-operations';
import { setGizmoManager, unsetGizmoManager } from './selection';

export const putHideComponent: ComponentOperation = (entity, component) => {
  // The AvatarShape mannequin is built synchronously, so it needs no wait for an asset.
  if (entity.avatarShape && component.componentType === ComponentType.LastWriteWinElementSet) {
    const { value: isHidden } =
      (component.getOrNull(entity.entityId) as { value: boolean } | null) ?? {};
    entity.avatarShape.setEnabled(!isHidden);
  }
  // Ensure asset is loaded before applying hide.
  entity.onAssetLoaded().then(() => {
    const container = entity.gltfContainer ?? entity.meshRenderer;
    if (!container) return;

    if (component.componentType === ComponentType.LastWriteWinElementSet) {
      const context = entity.context.deref()!;
      const { value: isHidden } =
        (component.getOrNull(entity.entityId) as { value: boolean } | null) ?? {};
      container.setEnabled(!isHidden);
      if (isHidden) {
        unsetGizmoManager(entity);
      } else {
        const selectionValue = context.editorComponents.Selection.getOrNull(entity.entityId);
        if (selectionValue) setGizmoManager(entity, selectionValue);
      }
    }
  });
};
