import type { Entity, IEngine } from '@dcl/ecs';
import { Engine } from '@dcl/ecs';

import type { EditorComponents, SdkComponents } from '../components';
import { createComponents, createEditorComponents } from '../components';
import { addComponent } from './add-component';
import { resetComponent } from './reset-component';

describe('resetComponent', () => {
  let engine: IEngine;
  let components: SdkComponents & EditorComponents;
  let entity: Entity;

  beforeEach(() => {
    engine = Engine();
    components = { ...createComponents(engine), ...createEditorComponents(engine) };
    entity = engine.addEntity();
  });

  describe('when the component was edited after being added', () => {
    let fresh: unknown;

    beforeEach(() => {
      addComponent(engine)(entity, components.Billboard.componentId);
      fresh = structuredClone(components.Billboard.get(entity));
      components.Billboard.getMutable(entity).billboardMode = 2;
      resetComponent(engine)(entity, components.Billboard);
    });

    it('should hold the values a freshly added component has', () => {
      expect(components.Billboard.get(entity)).toEqual(fresh);
    });
  });

  describe('when the entity does not have the component', () => {
    beforeEach(() => {
      resetComponent(engine)(entity, components.Billboard);
    });

    it('should not add it', () => {
      expect(components.Billboard.has(entity)).toBe(false);
    });
  });

  describe('when the component is a Transform under a parent', () => {
    let parent: Entity;

    beforeEach(() => {
      parent = engine.addEntity();
      components.Transform.create(entity, {
        parent,
        position: { x: 4, y: 5, z: 6 },
        scale: { x: 2, y: 3, z: 4 },
      });
      components.CameraModeArea.create(entity, { mode: 0, area: { x: 2, y: 3, z: 4 } });
      resetComponent(engine)(entity, components.Transform);
    });

    it('should return position and scale to their defaults', () => {
      const transform = components.Transform.get(entity);
      expect(transform.position).toEqual({ x: 0, y: 0, z: 0 });
      expect(transform.scale).toEqual({ x: 1, y: 1, z: 1 });
    });

    it('should keep the parent', () => {
      expect(components.Transform.get(entity).parent).toBe(parent);
    });

    it('should mirror the reset scale into the area components', () => {
      expect(components.CameraModeArea.get(entity).area).toEqual({ x: 1, y: 1, z: 1 });
    });
  });

  describe('when the component is an area component', () => {
    beforeEach(() => {
      components.Transform.create(entity, { scale: { x: 2, y: 3, z: 4 } });
      components.AvatarModifierArea.create(entity, {
        modifiers: [0, 1],
        excludeIds: ['0x1'],
        area: { x: 9, y: 9, z: 9 },
      });
      resetComponent(engine)(entity, components.AvatarModifierArea);
    });

    it('should hold the values the Add Component menu seeds it with', () => {
      const value = components.AvatarModifierArea.get(entity);
      expect(value.modifiers).toEqual([0]);
      expect(value.excludeIds).toEqual([]);
    });

    it('should size its area from the entity scale', () => {
      expect(components.AvatarModifierArea.get(entity).area).toEqual({ x: 2, y: 3, z: 4 });
    });
  });

  describe('when the component carries an id other items reference', () => {
    let id: number;

    beforeEach(() => {
      addComponent(engine)(entity, components.Counter.componentId);
      id = components.Counter.get(entity).id;
      components.Counter.getMutable(entity).value = 7;
      resetComponent(engine)(entity, components.Counter);
    });

    it('should reset the value', () => {
      expect(components.Counter.get(entity).value).toBe(0);
    });

    it('should keep the id', () => {
      expect(components.Counter.get(entity).id).toBe(id);
    });
  });
});
