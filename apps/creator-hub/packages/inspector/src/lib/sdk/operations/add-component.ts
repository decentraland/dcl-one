import type { Entity, IEngine, Vector3Type } from '@dcl/ecs';
import { COMPONENTS_WITH_ID, getNextId } from '@dcl/asset-packs';
import { isLastWriteWinComponent } from '../../../hooks/sdk/useComponentValue';
import { CoreComponents } from '../components/types';
import { getEntityScale, isAreaComponent } from './area-size';

/**
 * What the Add Component menu seeds a component with where the schema defaults alone
 * leave it unusable. "Reset values" returns a component to this same starting point.
 */
export function getInitialValue(componentName: string): Record<string, unknown> | undefined {
  switch (componentName) {
    case CoreComponents.AVATAR_MODIFIER_AREA:
      return { modifiers: [0], excludeIds: [] };
    case CoreComponents.CAMERA_MODE_AREA:
      return { mode: 0 };
    case CoreComponents.TEXT_SHAPE:
      return { font: 0, text: '' };
    default:
      return undefined;
  }
}

export function addComponent(engine: IEngine) {
  return function addComponent(entity: Entity, componentId: number, value?: any) {
    const component = engine.getComponent(componentId);
    if (isLastWriteWinComponent<{ id?: number; area?: Vector3Type }>(component)) {
      component.create(entity, value);
      if (COMPONENTS_WITH_ID.includes(component.componentName)) {
        const value = component.getMutable(entity);
        value.id = getNextId(engine as any);
      }
      // Area components mirror the entity's scale in their `area` field (the
      // runtime ignores the Transform scale for the region size), so a freshly
      // added component starts with the region the entity's scale describes.
      if (isAreaComponent(component.componentName)) {
        const mutable = component.getMutable(entity);
        if (!mutable.area) {
          mutable.area = getEntityScale(engine, entity);
        }
      }
    } else {
      throw new Error('Cannot add component: it must be an LWW component');
    }
  };
}

export default addComponent;
