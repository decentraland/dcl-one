import type {
  Entity,
  IEngine,
  LastWriteWinElementSetComponentDefinition,
  Vector3Type,
} from '@dcl/ecs';
import { COMPONENTS_WITH_ID } from '@dcl/asset-packs';
import { CoreComponents } from '../components/types';
import { getInitialValue } from './add-component';
import { getEntityScale, isAreaComponent, syncAreaWithScale } from './area-size';

type Resettable = { id?: number; area?: Vector3Type; parent?: Entity; scale?: Vector3Type };

/**
 * Returns a component to the values it has right after "Add Component". What ties the
 * entity to the rest of the scene stays: the id other items reference it by and, for a
 * Transform, its parent.
 */
export function resetComponent(engine: IEngine) {
  return function resetComponent<T>(
    entity: Entity,
    component: LastWriteWinElementSetComponentDefinition<T>,
  ) {
    const lww = component as unknown as LastWriteWinElementSetComponentDefinition<Resettable>;
    const current = lww.getOrNull(entity);
    if (current === null) return;
    const { id, parent } = current;

    const value = lww.createOrReplace(entity, getInitialValue(lww.componentName) as Resettable);

    if (COMPONENTS_WITH_ID.includes(lww.componentName)) {
      value.id = id;
    }
    if (isAreaComponent(lww.componentName) && !value.area) {
      value.area = getEntityScale(engine, entity);
    }
    if (lww.componentName === CoreComponents.TRANSFORM) {
      value.parent = parent;
      // Area components mirror the scale, as on every other Transform write.
      if (value.scale) syncAreaWithScale(engine, entity, value.scale);
    }
  };
}

export default resetComponent;
