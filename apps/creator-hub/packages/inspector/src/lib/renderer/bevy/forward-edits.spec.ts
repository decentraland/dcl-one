import * as components from '@dcl/ecs/dist/components';

import type { EngineWindow } from './console';
import { BevySceneContext } from './BevySceneContext';
import { createForwardEditBridge } from './forward-edits';

/**
 * The forward edit bridge translates the inspector's ECS changes into engine
 * console commands (the only way to mutate the running scene). Driven through a
 * real BevySceneContext + engine so the change stream is genuine; the console
 * `send` is a recorder (no wasm).
 */
describe('createForwardEditBridge', () => {
  let ctx: BevySceneContext;
  let sent: Array<{ cmd: string; args: string[] }>;
  let bridge: ReturnType<typeof createForwardEditBridge>;

  const IDENTITY = { rotation: { x: 0, y: 0, z: 0, w: 1 }, scale: { x: 1, y: 1, z: 1 } };
  const engineWindow = {} as EngineWindow;

  beforeEach(() => {
    ctx = new BevySceneContext();
    sent = [];
    bridge = createForwardEditBridge({
      context: ctx,
      engineWindow,
      // Forward immediately in tests (skip the initial-load arm delay).
      shouldForward: () => true,
      send: async (cmd, args) => {
        sent.push({ cmd, args });
        return '';
      },
    });
  });

  afterEach(() => {
    bridge.disconnect();
    ctx.dispose();
  });

  describe('when a Transform is written', () => {
    it('should send set_component with the short engine name and JSON value', async () => {
      const entity = ctx.engine.addEntity();
      ctx.Transform.create(entity, {
        ...IDENTITY,
        position: { x: 1, y: 2, z: 3 },
        parent: ctx.engine.RootEntity,
      });
      await ctx.engine.update(1);

      const write = sent.find(s => s.cmd === 'set_component');
      expect(write).toBeDefined();
      expect(write!.args[0]).toBe(String(entity));
      expect(write!.args[1]).toBe('Transform'); // core:: stripped
      const value = JSON.parse(write!.args[2]);
      expect(value.position).toEqual({ x: 1, y: 2, z: 3 });
    });
  });

  describe('when a GltfContainer is written (a newly added/dropped model)', () => {
    it('should refresh the content map (scene_content) BEFORE writing GltfContainer', async () => {
      const GltfContainer = components.GltfContainer(ctx.engine);
      const entity = ctx.engine.addEntity();
      GltfContainer.create(entity, { src: 'assets/thing.glb', visibleMeshesCollisionMask: 3 });
      await ctx.engine.update(1);
      // Let the sequenced async fire (scene_content → set_component) settle.
      await new Promise(r => setTimeout(r, 0));

      const refreshIdx = sent.findIndex(s => s.cmd === 'scene_content');
      const setIdx = sent.findIndex(
        s => s.cmd === 'set_component' && s.args[1] === 'GltfContainer',
      );
      expect(refreshIdx).toBeGreaterThanOrEqual(0);
      expect(setIdx).toBeGreaterThanOrEqual(0);
      // Content refresh must precede the GltfContainer set so the src resolves and
      // the model renders instead of a placeholder cube.
      expect(refreshIdx).toBeLessThan(setIdx);

      const write = sent[setIdx];
      expect(write.args[0]).toBe(String(entity));
      expect(JSON.parse(write.args[2]).src).toBe('assets/thing.glb');
    });
  });

  describe('when a NEW entity is added (Name + engine components, like a dropped asset)', () => {
    it('should instantiate it via new_entity before forwarding its engine components', async () => {
      const GltfContainer = components.GltfContainer(ctx.engine);
      const entity = ctx.engine.addEntity();
      // Mirror add-asset's write order: engine components first, then Name (the
      // custom component that anchors instantiation) — the case that failed live.
      ctx.Transform.create(entity, {
        ...IDENTITY,
        position: { x: 1, y: 0, z: 2 },
        parent: ctx.engine.RootEntity,
      });
      GltfContainer.create(entity, { src: 'assets/car.glb', visibleMeshesCollisionMask: 3 });
      ctx.Name.create(entity, { value: 'Car' });
      await ctx.engine.update(1);
      // Let the per-entity async queue (new_entity → set_component…) drain.
      await new Promise(r => setTimeout(r, 0));
      await new Promise(r => setTimeout(r, 0));

      const newEntityIdx = sent.findIndex(s => s.cmd === 'new_entity');
      expect(newEntityIdx).toBeGreaterThanOrEqual(0);
      // Instantiate at the exact inspector id, anchored by the Name component.
      const args = sent[newEntityIdx].args;
      expect(args[0]).toBe(String(ctx.Name.componentId));
      expect(args).toContain('--ids');
      expect(args).toContain(String(entity));

      // The engine components (Transform, GltfContainer) are (re)sent AFTER the
      // instantiation, so they land on the now-existing entity.
      const gltfIdx = sent.findIndex(
        s => s.cmd === 'set_component' && s.args[1] === 'GltfContainer',
      );
      const tfIdx = sent.findIndex(s => s.cmd === 'set_component' && s.args[1] === 'Transform');
      expect(gltfIdx).toBeGreaterThan(newEntityIdx);
      expect(tfIdx).toBeGreaterThan(newEntityIdx);
    });

    it('should NOT instantiate when only editing an existing entity (no Name write)', async () => {
      const entity = ctx.engine.addEntity();
      ctx.Transform.create(entity, { ...IDENTITY, position: { x: 0, y: 0, z: 0 } });
      await ctx.engine.update(1);
      sent.length = 0;

      // A gizmo-style move: Transform changes, no Name — must not trigger new_entity.
      ctx.Transform.getMutable(entity).position = { x: 5, y: 0, z: 5 };
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));

      expect(sent.some(s => s.cmd === 'new_entity')).toBe(false);
      expect(sent.some(s => s.cmd === 'set_component' && s.args[1] === 'Transform')).toBe(true);
    });
  });

  describe('when an entity is deleted', () => {
    it('should send delete_entity', async () => {
      const entity = ctx.engine.addEntity();
      ctx.Transform.create(entity, { ...IDENTITY, position: { x: 0, y: 0, z: 0 } });
      await ctx.engine.update(1);
      sent.length = 0;

      ctx.engine.removeEntity(entity);
      await ctx.engine.update(1);

      expect(sent).toContainEqual({ cmd: 'delete_entity', args: [String(entity)] });
    });
  });

  describe('when a Name component is deleted (undo of an add, without a DELETE_ENTITY)', () => {
    it('should send delete_entity so a mirrored engine drops the whole entity (#1460)', async () => {
      const GltfContainer = components.GltfContainer(ctx.engine);
      const entity = ctx.engine.addEntity();
      ctx.Transform.create(entity, {
        ...IDENTITY,
        position: { x: 0, y: 0, z: 0 },
        parent: ctx.engine.RootEntity,
      });
      GltfContainer.create(entity, { src: 'assets/car.glb', visibleMeshesCollisionMask: 3 });
      ctx.Name.create(entity, { value: 'Car' });
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));
      sent.length = 0;

      // Undo reverts an add by deleting each component one-by-one — including Name —
      // and never calls removeEntity, so no DELETE_ENTITY is emitted. The Name delete
      // must still tear the entity down in the mirrored engine.
      ctx.Transform.deleteFrom(entity);
      GltfContainer.deleteFrom(entity);
      ctx.Name.deleteFrom(entity);
      await ctx.engine.update(1);

      expect(sent).toContainEqual({ cmd: 'delete_entity', args: [String(entity)] });
    });
  });

  describe('when an unsupported (custom/schema) component changes', () => {
    it('should not send anything (deferred, not mis-addressed)', async () => {
      // PlayerIdentityData etc. aren't in the engine-name map; use a component
      // the context does not forward. Name component is core-schema, not core::.
      const entity = ctx.engine.addEntity();
      // Only Transform is bound on the context; create it, then confirm the only
      // writes are Transform ones (no stray sends for internal editor components).
      ctx.Transform.create(entity, { ...IDENTITY, position: { x: 0, y: 0, z: 0 } });
      await ctx.engine.update(1);
      expect(sent.every(s => s.args[1] === 'Transform' || s.cmd === 'delete_entity')).toBe(true);
    });
  });

  describe('after disconnect', () => {
    it('should stop forwarding changes', async () => {
      bridge.disconnect();
      const entity = ctx.engine.addEntity();
      ctx.Transform.create(entity, { ...IDENTITY, position: { x: 9, y: 9, z: 9 } });
      await ctx.engine.update(1);
      expect(sent).toEqual([]);
    });
  });

  describe('when the forward gate is closed (initial-load suppression)', () => {
    it('should not forward changes while shouldForward is false', async () => {
      const gatedSent: Array<{ cmd: string; args: string[] }> = [];
      const gatedCtx = new BevySceneContext();
      const off = createForwardEditBridge({
        context: gatedCtx,
        engineWindow,
        shouldForward: () => false, // gate closed (simulates the arm delay)
        send: async (cmd, args) => {
          gatedSent.push({ cmd, args });
          return '';
        },
      });

      const entity = gatedCtx.engine.addEntity();
      gatedCtx.Transform.create(entity, { ...IDENTITY, position: { x: 1, y: 1, z: 1 } });
      await gatedCtx.engine.update(1);

      expect(gatedSent).toEqual([]);
      off.disconnect();
      gatedCtx.dispose();
    });
  });

  describe('when a GltfContainer is forwarded (#1373: editor pointer-pickable)', () => {
    it('should OR the pointer bit into visibleMeshesCollisionMask', async () => {
      const GltfContainer = components.GltfContainer(ctx.engine);
      const entity = ctx.engine.addEntity();
      // A model authored with NO pointer collision (mask 0) — unpickable as-is.
      GltfContainer.create(entity, { src: 'assets/thing.glb', visibleMeshesCollisionMask: 0 });
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));

      const write = sent.find(s => s.cmd === 'set_component' && s.args[1] === 'GltfContainer');
      expect(write).toBeDefined();
      const value = JSON.parse(write!.args[2]);
      // CL_POINTER (bit 1) forced on so the editor pick ray hits it.
      expect(value.visibleMeshesCollisionMask & 1).toBe(1);
    });

    it('should preserve other collision bits already set', async () => {
      const GltfContainer = components.GltfContainer(ctx.engine);
      const entity = ctx.engine.addEntity();
      // Authored with CL_PHYSICS (bit 2) only.
      GltfContainer.create(entity, { src: 'assets/thing.glb', visibleMeshesCollisionMask: 2 });
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));

      const write = sent.find(s => s.cmd === 'set_component' && s.args[1] === 'GltfContainer');
      const mask = JSON.parse(write!.args[2]).visibleMeshesCollisionMask;
      expect(mask & 1).toBe(1); // pointer added
      expect(mask & 2).toBe(2); // physics preserved
    });
  });

  describe('when a dropped asset file lags the content map (#1459)', () => {
    it('should retry scene_content until the src appears, then set GltfContainer', async () => {
      const raceSent: Array<{ cmd: string; args: string[] }> = [];
      const raceCtx = new BevySceneContext();
      // The dev server's content map is empty on the first refresh (the asset file
      // hasn't propagated yet — the Windows race) and includes it from the 2nd.
      let refreshes = 0;
      const bridge2 = createForwardEditBridge({
        context: raceCtx,
        engineWindow,
        shouldForward: () => true,
        send: async (cmd, args) => {
          raceSent.push({ cmd, args });
          if (cmd === 'scene_content') {
            refreshes += 1;
            return refreshes < 2 ? JSON.stringify([]) : JSON.stringify(['assets/late.glb']);
          }
          return '';
        },
      });

      const GltfContainer = components.GltfContainer(raceCtx.engine);
      const entity = raceCtx.engine.addEntity();
      GltfContainer.create(entity, { src: 'assets/late.glb', visibleMeshesCollisionMask: 3 });
      await raceCtx.engine.update(1);
      // Allow the bounded retry (one ~120ms delay) to elapse before asserting.
      await new Promise(r => setTimeout(r, 300));

      // It refreshed more than once (waited for the file), then set the model.
      expect(refreshes).toBeGreaterThanOrEqual(2);
      const refreshIdx = raceSent.findIndex(s => s.cmd === 'scene_content');
      const setIdx = raceSent.findIndex(
        s => s.cmd === 'set_component' && s.args[1] === 'GltfContainer',
      );
      expect(setIdx).toBeGreaterThanOrEqual(0);
      expect(refreshIdx).toBeLessThan(setIdx);

      bridge2.disconnect();
      raceCtx.dispose();
    });
  });

  describe('when an editor Placeholder is written (#1372)', () => {
    it('should forward it as a GltfContainer pointed at the placeholder src', async () => {
      const entity = ctx.engine.addEntity();
      ctx.Name.create(entity, { value: 'Trigger Area' });
      ctx.editorComponents.Placeholder.create(entity, { src: 'assets/placeholder.glb' });
      await ctx.engine.update(1);
      // let the per-entity enqueue chain (ensureInstantiated → set) drain
      await new Promise(r => setTimeout(r, 0));
      await new Promise(r => setTimeout(r, 0));

      const write = sent.find(s => s.cmd === 'set_component' && s.args[1] === 'GltfContainer');
      expect(write).toBeDefined();
      const value = JSON.parse(write!.args[2]);
      expect(value.src).toBe('assets/placeholder.glb');
      // forwardSet also makes it pointer-pickable (#1373) so the placeholder can
      // be clicked to select the item.
      expect(value.visibleMeshesCollisionMask & 1).toBe(1);
    });

    describe('and the entity also has an authored GltfContainer (an invisible wall)', () => {
      const drain = async () => {
        await new Promise(r => setTimeout(r, 0));
        await new Promise(r => setTimeout(r, 0));
      };
      const gltfSrcs = () =>
        sent
          .filter(s => s.cmd === 'set_component' && s.args[1] === 'GltfContainer')
          .map(s => JSON.parse(s.args[2]).src as string);

      it('should forward the placeholder, never the authored model, while frozen', async () => {
        const GltfContainer = components.GltfContainer(ctx.engine);
        const entity = ctx.engine.addEntity();
        ctx.Name.create(entity, { value: 'Invisible Wall' });
        GltfContainer.create(entity, {
          src: 'assets/wall_collider.glb',
          invisibleMeshesCollisionMask: 2,
        });
        ctx.editorComponents.Placeholder.create(entity, { src: 'assets/wall.glb' });
        await ctx.engine.update(1);
        await drain();

        expect(gltfSrcs().length).toBeGreaterThan(0);
        expect(new Set(gltfSrcs())).toEqual(new Set(['assets/wall.glb']));
      });

      it('should keep the placeholder when the authored model is edited while frozen', async () => {
        const GltfContainer = components.GltfContainer(ctx.engine);
        const entity = ctx.engine.addEntity();
        ctx.Name.create(entity, { value: 'Invisible Wall' });
        GltfContainer.create(entity, { src: 'assets/wall_collider.glb' });
        ctx.editorComponents.Placeholder.create(entity, { src: 'assets/wall.glb' });
        await ctx.engine.update(1);
        await drain();
        sent.length = 0;

        GltfContainer.getMutable(entity).invisibleMeshesCollisionMask = 3;
        await ctx.engine.update(1);
        await drain();

        expect(gltfSrcs()).toEqual([]);
      });

      it('should restore the authored model when the placeholder is removed', async () => {
        const GltfContainer = components.GltfContainer(ctx.engine);
        const entity = ctx.engine.addEntity();
        ctx.Name.create(entity, { value: 'Invisible Wall' });
        GltfContainer.create(entity, { src: 'assets/wall_collider.glb' });
        ctx.editorComponents.Placeholder.create(entity, { src: 'assets/wall.glb' });
        await ctx.engine.update(1);
        await drain();
        sent.length = 0;

        ctx.editorComponents.Placeholder.deleteFrom(entity);
        await ctx.engine.update(1);
        await drain();

        expect(gltfSrcs()).toEqual(['assets/wall_collider.glb']);
        expect(sent.some(s => s.cmd === 'delete_component')).toBe(false);
      });

      it('should replay only the placeholder after a scene reload', async () => {
        const GltfContainer = components.GltfContainer(ctx.engine);
        const entity = ctx.engine.addEntity();
        ctx.Name.create(entity, { value: 'Invisible Wall' });
        GltfContainer.create(entity, { src: 'assets/wall_collider.glb' });
        ctx.editorComponents.Placeholder.create(entity, { src: 'assets/wall.glb' });
        await ctx.engine.update(1);
        await drain();
        sent.length = 0;

        bridge.reconcileAfterReload();
        await drain();

        expect(gltfSrcs()).toEqual(['assets/wall.glb']);
      });
    });

    it('should delete the engine GltfContainer when the placeholder is removed', async () => {
      const entity = ctx.engine.addEntity();
      ctx.Name.create(entity, { value: 'Trigger Area' });
      ctx.editorComponents.Placeholder.create(entity, { src: 'assets/placeholder.glb' });
      await ctx.engine.update(1);
      await Promise.resolve();
      await Promise.resolve();
      sent.length = 0;

      ctx.editorComponents.Placeholder.deleteFrom(entity);
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));
      await new Promise(r => setTimeout(r, 0));

      const del = sent.find(s => s.cmd === 'delete_component' && s.args[1] === 'GltfContainer');
      expect(del).toBeDefined();
    });
  });

  describe('when the scene runs (Play) with an editor Placeholder', () => {
    const drain = async () => {
      await new Promise(r => setTimeout(r, 0));
      await new Promise(r => setTimeout(r, 0));
    };

    it('should remove the placeholder GLTF on Play and restore it when frozen again', async () => {
      const entity = ctx.engine.addEntity();
      ctx.Name.create(entity, { value: 'Trigger Area' });
      ctx.editorComponents.Placeholder.create(entity, { src: 'assets/placeholder.glb' });
      await ctx.engine.update(1);
      await drain();
      sent.length = 0;

      bridge.setSceneFrozen(false);
      await drain();
      expect(
        sent.some(
          s =>
            s.cmd === 'delete_component' &&
            s.args[0] === String(entity) &&
            s.args[1] === 'GltfContainer',
        ),
      ).toBe(true);

      sent.length = 0;
      bridge.setSceneFrozen(true);
      await drain();
      const write = sent.find(s => s.cmd === 'set_component' && s.args[1] === 'GltfContainer');
      expect(write).toBeDefined();
      expect(JSON.parse(write!.args[2]).src).toBe('assets/placeholder.glb');
    });

    it('should swap an authored GltfContainer back in on Play and the placeholder on freeze', async () => {
      const GltfContainer = components.GltfContainer(ctx.engine);
      const entity = ctx.engine.addEntity();
      ctx.Name.create(entity, { value: 'Invisible Wall' });
      GltfContainer.create(entity, {
        src: 'assets/wall_collider.glb',
        invisibleMeshesCollisionMask: 2,
      });
      ctx.editorComponents.Placeholder.create(entity, { src: 'assets/wall.glb' });
      await ctx.engine.update(1);
      await drain();
      sent.length = 0;

      bridge.setSceneFrozen(false);
      await drain();
      expect(sent.some(s => s.cmd === 'delete_component' && s.args[1] === 'GltfContainer')).toBe(
        false,
      );
      const played = sent.filter(s => s.cmd === 'set_component' && s.args[1] === 'GltfContainer');
      expect(played).toHaveLength(1);
      expect(JSON.parse(played[0].args[2])).toMatchObject({
        src: 'assets/wall_collider.glb',
        invisibleMeshesCollisionMask: 2,
      });

      sent.length = 0;
      bridge.setSceneFrozen(true);
      await drain();
      const frozen = sent.filter(s => s.cmd === 'set_component' && s.args[1] === 'GltfContainer');
      expect(frozen.map(s => JSON.parse(s.args[2]).src)).toEqual(['assets/wall.glb']);
    });

    it('should not forward a Placeholder written while the scene runs', async () => {
      bridge.disconnect();
      bridge = createForwardEditBridge({
        context: ctx,
        engineWindow,
        shouldForward: () => true,
        isFrozen: () => false,
        send: async (cmd, args) => {
          sent.push({ cmd, args });
          return '';
        },
      });
      const entity = ctx.engine.addEntity();
      ctx.Name.create(entity, { value: 'Trigger Area' });
      ctx.editorComponents.Placeholder.create(entity, { src: 'assets/placeholder.glb' });
      await ctx.engine.update(1);
      await drain();

      expect(
        sent.some(
          s =>
            s.cmd === 'set_component' &&
            s.args[1] === 'GltfContainer' &&
            JSON.parse(s.args[2]).src === 'assets/placeholder.glb',
        ),
      ).toBe(false);
    });
  });

  describe('when entities are attached to the camera', () => {
    let hide: boolean;

    const drain = async () => {
      await new Promise(r => setTimeout(r, 0));
      await new Promise(r => setTimeout(r, 0));
    };
    const lastVisibility = (entity: number): boolean | undefined => {
      const writes = sent.filter(
        s =>
          s.cmd === 'set_component' &&
          s.args[0] === String(entity) &&
          s.args[1] === 'VisibilityComponent',
      );
      const last = writes[writes.length - 1];
      return last ? JSON.parse(last.args[2]).visible : undefined;
    };
    const addEntity = (name: string, parent: number) => {
      const entity = ctx.engine.addEntity();
      ctx.Name.create(entity, { value: name });
      ctx.Transform.create(entity, {
        ...IDENTITY,
        position: { x: 0, y: 0, z: 1 },
        parent: parent as never,
      });
      return entity;
    };

    beforeEach(() => {
      hide = true;
      bridge.disconnect();
      bridge = createForwardEditBridge({
        context: ctx,
        engineWindow,
        shouldForward: () => true,
        hideCameraAttachments: () => hide,
        send: async (cmd, args) => {
          sent.push({ cmd, args });
          return '';
        },
      });
    });

    it('should hide the camera children and their descendants while editing with the free camera', async () => {
      const child = addEntity('Gun', ctx.engine.CameraEntity);
      const grandchild = addEntity('Muzzle', child);
      const other = addEntity('Tree', ctx.engine.RootEntity);
      await ctx.engine.update(1);
      await drain();

      expect(lastVisibility(child)).toBe(false);
      expect(lastVisibility(grandchild)).toBe(false);
      expect(lastVisibility(other)).toBe(true);
    });

    it('should show them again once the camera is no longer the free editing camera', async () => {
      const child = addEntity('Gun', ctx.engine.CameraEntity);
      await ctx.engine.update(1);
      await drain();
      expect(lastVisibility(child)).toBe(false);

      hide = false;
      bridge.refreshCameraAttachments();
      await drain();
      expect(lastVisibility(child)).toBe(true);
    });

    it('should keep an editor-hidden attachment hidden when it is shown again', async () => {
      const child = addEntity('Gun', ctx.engine.CameraEntity);
      ctx.editorComponents.Hide.create(child, { value: true });
      await ctx.engine.update(1);
      await drain();

      hide = false;
      bridge.refreshCameraAttachments();
      await drain();
      expect(lastVisibility(child)).toBe(false);
    });

    it('should follow a reparent onto and off the camera', async () => {
      const entity = addEntity('Lamp', ctx.engine.RootEntity);
      await ctx.engine.update(1);
      await drain();
      expect(lastVisibility(entity)).toBe(true);

      ctx.Transform.getMutable(entity).parent = ctx.engine.CameraEntity;
      await ctx.engine.update(1);
      await drain();
      expect(lastVisibility(entity)).toBe(false);

      ctx.Transform.getMutable(entity).parent = ctx.engine.RootEntity;
      await ctx.engine.update(1);
      await drain();
      expect(lastVisibility(entity)).toBe(true);
    });

    it('should not rescan the hierarchy when a Transform write keeps its parent', async () => {
      const child = addEntity('Gun', ctx.engine.CameraEntity);
      await ctx.engine.update(1);
      await drain();
      const scan = vi.spyOn(ctx.engine, 'getEntitiesWith');

      ctx.Transform.getMutable(child).position = { x: 0, y: 1, z: 1 };
      await ctx.engine.update(1);
      await drain();

      expect(scan.mock.calls.some(([component]) => component === ctx.Transform)).toBe(false);
      scan.mockRestore();
    });

    it('should hide them again when overrides are replayed after a reload', async () => {
      const child = addEntity('Gun', ctx.engine.CameraEntity);
      await ctx.engine.update(1);
      await drain();
      sent.length = 0;

      bridge.reconcileAfterReload();
      await drain();
      expect(lastVisibility(child)).toBe(false);
    });
  });

  describe('setSceneFrozen (#1382)', () => {
    it('should forward Animator with all states playing:false when frozen', async () => {
      const Animator = components.Animator(ctx.engine);
      const entity = ctx.engine.addEntity();
      ctx.Name.create(entity, { value: 'Zombie' });
      Animator.create(entity, {
        states: [
          { clip: 'walk', playing: true, loop: true },
          { clip: 'idle', playing: false },
        ],
      });
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));
      sent.length = 0;

      bridge.setSceneFrozen(true);
      await new Promise(r => setTimeout(r, 0));
      await new Promise(r => setTimeout(r, 0));

      const write = sent.find(s => s.cmd === 'set_component' && s.args[1] === 'Animator');
      expect(write).toBeDefined();
      const value = JSON.parse(write!.args[2]);
      expect(value.states.every((s: { playing: boolean }) => s.playing === false)).toBe(true);
    });

    it('should restore the authored Animator when unfrozen', async () => {
      const Animator = components.Animator(ctx.engine);
      const entity = ctx.engine.addEntity();
      ctx.Name.create(entity, { value: 'Zombie' });
      Animator.create(entity, { states: [{ clip: 'walk', playing: true, loop: true }] });
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));
      sent.length = 0;

      bridge.setSceneFrozen(false);
      await new Promise(r => setTimeout(r, 0));
      await new Promise(r => setTimeout(r, 0));

      const write = sent.find(s => s.cmd === 'set_component' && s.args[1] === 'Animator');
      expect(write).toBeDefined();
      const value = JSON.parse(write!.args[2]);
      // The AUTHORED value (walk still playing) is restored.
      expect(value.states[0].playing).toBe(true);
    });

    it('should force an Animator PUT to playing:false while frozen (#1421)', async () => {
      // The default bridge is frozen (isFrozen defaults to true). A reloaded GLTF
      // re-PUTs its authored Animator (playing:true) — while frozen that must be
      // forwarded paused, or the clip resumes for a few frames after Stop.
      const Animator = components.Animator(ctx.engine);
      const entity = ctx.engine.addEntity();
      ctx.Name.create(entity, { value: 'Zombie' });
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));
      sent.length = 0;

      // The Animator arrives (as after a reload) with the clip playing.
      Animator.create(entity, { states: [{ clip: 'walk', playing: true, loop: true }] });
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));
      await new Promise(r => setTimeout(r, 0));

      const write = sent.find(s => s.cmd === 'set_component' && s.args[1] === 'Animator');
      expect(write).toBeDefined();
      const value = JSON.parse(write!.args[2]);
      expect(value.states.every((s: { playing: boolean }) => s.playing === false)).toBe(true);
    });

    it('should forward an Animator PUT as-is when NOT frozen', async () => {
      // Replace the default (frozen) bridge with a running one so only its forwards
      // land in `sent`.
      bridge.disconnect();
      const running = createForwardEditBridge({
        context: ctx,
        engineWindow,
        shouldForward: () => true,
        isFrozen: () => false,
        send: async (cmd, args) => {
          sent.push({ cmd, args });
          return '';
        },
      });
      const Animator = components.Animator(ctx.engine);
      const entity = ctx.engine.addEntity();
      ctx.Name.create(entity, { value: 'Zombie' });
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));
      sent.length = 0;

      Animator.create(entity, { states: [{ clip: 'walk', playing: true, loop: true }] });
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));
      await new Promise(r => setTimeout(r, 0));

      const write = sent.find(s => s.cmd === 'set_component' && s.args[1] === 'Animator');
      expect(write).toBeDefined();
      const value = JSON.parse(write!.args[2]);
      expect(value.states[0].playing).toBe(true);
      running.disconnect();
    });
  });

  describe('VideoPlayer freeze (#1469)', () => {
    it('should forward VideoPlayer with playing:false when frozen', async () => {
      const VideoPlayer = components.VideoPlayer(ctx.engine);
      const entity = ctx.engine.addEntity();
      ctx.Name.create(entity, { value: 'Screen' });
      VideoPlayer.create(entity, { src: 'video.mp4', playing: true });
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));
      sent.length = 0;

      bridge.setSceneFrozen(true);
      await new Promise(r => setTimeout(r, 0));
      await new Promise(r => setTimeout(r, 0));

      const write = sent.find(s => s.cmd === 'set_component' && s.args[1] === 'VideoPlayer');
      expect(write).toBeDefined();
      expect(JSON.parse(write!.args[2]).playing).toBe(false);
    });

    it('should restore the authored VideoPlayer (playing) when unfrozen', async () => {
      const VideoPlayer = components.VideoPlayer(ctx.engine);
      const entity = ctx.engine.addEntity();
      ctx.Name.create(entity, { value: 'Screen' });
      VideoPlayer.create(entity, { src: 'video.mp4', playing: true });
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));
      sent.length = 0;

      bridge.setSceneFrozen(false);
      await new Promise(r => setTimeout(r, 0));
      await new Promise(r => setTimeout(r, 0));

      const write = sent.find(s => s.cmd === 'set_component' && s.args[1] === 'VideoPlayer');
      expect(write).toBeDefined();
      expect(JSON.parse(write!.args[2]).playing).toBe(true);
    });

    it('should force a VideoPlayer PUT to playing:false while frozen', async () => {
      // The default bridge is frozen. A video added/loaded while paused must not
      // start playing in the editor (#1469).
      const VideoPlayer = components.VideoPlayer(ctx.engine);
      const entity = ctx.engine.addEntity();
      ctx.Name.create(entity, { value: 'Screen' });
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));
      sent.length = 0;

      VideoPlayer.create(entity, { src: 'video.mp4', playing: true });
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));
      await new Promise(r => setTimeout(r, 0));

      const write = sent.find(s => s.cmd === 'set_component' && s.args[1] === 'VideoPlayer');
      expect(write).toBeDefined();
      expect(JSON.parse(write!.args[2]).playing).toBe(false);
    });
  });

  describe('MeshRenderer oneof JSON shape (#1466)', () => {
    it('should forward the engine serde shape (mesh.box), not the $case discriminator', async () => {
      const MeshRenderer = components.MeshRenderer(ctx.engine);
      const entity = ctx.engine.addEntity();
      MeshRenderer.create(entity, { mesh: { $case: 'box', box: { uvs: [] } } });
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));

      const write = sent.find(s => s.cmd === 'set_component' && s.args[1] === 'MeshRenderer');
      expect(write).toBeDefined();
      const value = JSON.parse(write!.args[2]);
      // Engine (serde externally-tagged oneof, camelCase): { mesh: { box: {...} } }.
      expect(value.mesh).toEqual({ box: { uvs: [] } });
      expect(value.mesh.$case).toBeUndefined();
    });

    it('should carry the variant name for a shape change (box → sphere)', async () => {
      const MeshRenderer = components.MeshRenderer(ctx.engine);
      const entity = ctx.engine.addEntity();
      MeshRenderer.create(entity, { mesh: { $case: 'box', box: { uvs: [] } } });
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));
      sent.length = 0;

      // The user switches the shape to a sphere in the inspector.
      MeshRenderer.createOrReplace(entity, { mesh: { $case: 'sphere', sphere: { uvs: [] } } });
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));

      const write = sent.find(s => s.cmd === 'set_component' && s.args[1] === 'MeshRenderer');
      expect(write).toBeDefined();
      const value = JSON.parse(write!.args[2]);
      expect(value.mesh).toEqual({ sphere: { uvs: [] } });
    });
  });

  describe('MeshRenderer viewport pickability (editor pointer collider)', () => {
    it('should forward an editor pointer MeshCollider matching the shape when none is authored', async () => {
      const MeshRenderer = components.MeshRenderer(ctx.engine);
      const entity = ctx.engine.addEntity();
      ctx.Name.create(entity, { value: 'Cube' });
      MeshRenderer.create(entity, { mesh: { $case: 'box', box: { uvs: [] } } });
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));
      await new Promise(r => setTimeout(r, 0));

      const write = sent.find(s => s.cmd === 'set_component' && s.args[1] === 'MeshCollider');
      expect(write).toBeDefined();
      const value = JSON.parse(write!.args[2]);
      expect(value.mesh).toEqual({ box: { uvs: [] } });
      expect(value.collisionMask & 1).toBe(1); // CL_POINTER → clickable
    });

    it('should OR CL_POINTER into an authored MeshCollider (preserving its bits)', async () => {
      const MeshRenderer = components.MeshRenderer(ctx.engine);
      const MeshCollider = components.MeshCollider(ctx.engine);
      const entity = ctx.engine.addEntity();
      ctx.Name.create(entity, { value: 'Cube' });
      MeshRenderer.create(entity, { mesh: { $case: 'box', box: { uvs: [] } } });
      MeshCollider.create(entity, { mesh: { $case: 'box', box: {} }, collisionMask: 2 }); // physics
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));
      await new Promise(r => setTimeout(r, 0));

      const colliders = sent.filter(s => s.cmd === 'set_component' && s.args[1] === 'MeshCollider');
      expect(colliders.length).toBeGreaterThanOrEqual(1);
      for (const w of colliders) {
        const v = JSON.parse(w.args[2]);
        expect(v.collisionMask & 1).toBe(1); // pointer added
        expect(v.collisionMask & 2).toBe(2); // physics preserved
      }
    });
  });

  describe('when a LightSource is written', () => {
    it('should forward it live with the light-type oneof collapsed for serde', async () => {
      const LightSource = components.LightSource(ctx.engine);
      const entity = ctx.engine.addEntity();
      ctx.Name.create(entity, { value: 'Lamp' });
      LightSource.create(entity, {
        active: true,
        intensity: 800,
        color: { r: 1, g: 0.5, b: 0 },
        type: { $case: 'spot', spot: { innerAngle: 20, outerAngle: 45 } },
      });
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));
      await new Promise(r => setTimeout(r, 0));

      const write = sent.find(s => s.cmd === 'set_component' && s.args[1] === 'LightSource');
      expect(write).toBeDefined();
      expect(write!.args[0]).toBe(String(entity));
      const value = JSON.parse(write!.args[2]);
      expect(value.intensity).toBe(800);
      expect(value.color).toEqual({ r: 1, g: 0.5, b: 0 });
      expect(value.type).toEqual({ spot: { innerAngle: 20, outerAngle: 45 } });
    });
  });

  describe('when a static engine component without a live path before is written', () => {
    it.each([
      [
        'VirtualCamera',
        () => components.VirtualCamera(ctx.engine),
        { defaultTransition: { transitionMode: { $case: 'time', time: 2 } } },
        (v: any) => expect(v.defaultTransition.transitionMode).toEqual({ time: 2 }),
      ],
      [
        'AvatarAttach',
        () => components.AvatarAttach(ctx.engine),
        { anchorPointId: 3 },
        (v: any) => expect(v.anchorPointId).toBe(3),
      ],
      [
        'CameraModeArea',
        () => components.CameraModeArea(ctx.engine),
        { area: { x: 4, y: 2, z: 4 }, mode: 1 },
        (v: any) => expect(v.area).toEqual({ x: 4, y: 2, z: 4 }),
      ],
      [
        'NftShape',
        () => components.NftShape(ctx.engine),
        { urn: 'urn:decentraland:ethereum:erc721:0xabc:1' },
        (v: any) => expect(v.urn).toContain('erc721'),
      ],
    ])('should forward %s as-is', async (engineName, getComponent, value, check) => {
      const Component = getComponent() as unknown as { create: (e: unknown, v: unknown) => void };
      const entity = ctx.engine.addEntity();
      ctx.Name.create(entity, { value: engineName });
      Component.create(entity, value);
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));
      await new Promise(r => setTimeout(r, 0));

      const write = sent.find(s => s.cmd === 'set_component' && s.args[1] === engineName);
      expect(write).toBeDefined();
      expect(write!.args[0]).toBe(String(entity));
      check(JSON.parse(write!.args[2]));
    });
  });

  describe('when an AudioSource is written', () => {
    it('should force playing:false while frozen and restore the authored value on unfreeze', async () => {
      const AudioSource = components.AudioSource(ctx.engine);
      const entity = ctx.engine.addEntity();
      ctx.Name.create(entity, { value: 'Speaker' });
      AudioSource.create(entity, { audioClipUrl: 'sounds/loop.mp3', playing: true, volume: 0.5 });
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));
      await new Promise(r => setTimeout(r, 0));

      // The default bridge is frozen: the edit lands paused, other fields intact.
      const frozenWrite = sent.find(s => s.cmd === 'set_component' && s.args[1] === 'AudioSource');
      expect(frozenWrite).toBeDefined();
      expect(JSON.parse(frozenWrite!.args[2])).toMatchObject({
        audioClipUrl: 'sounds/loop.mp3',
        playing: false,
        volume: 0.5,
      });

      sent.length = 0;
      bridge.setSceneFrozen(false);
      await new Promise(r => setTimeout(r, 0));
      await new Promise(r => setTimeout(r, 0));

      const resumed = sent.find(s => s.cmd === 'set_component' && s.args[1] === 'AudioSource');
      expect(resumed).toBeDefined();
      expect(JSON.parse(resumed!.args[2]).playing).toBe(true);
    });
  });

  describe('when an AudioStream is written', () => {
    it('should force playing:false while frozen', async () => {
      const AudioStream = components.AudioStream(ctx.engine);
      const entity = ctx.engine.addEntity();
      ctx.Name.create(entity, { value: 'Radio' });
      AudioStream.create(entity, { url: 'https://stream.example/live', playing: true });
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));
      await new Promise(r => setTimeout(r, 0));

      const write = sent.find(s => s.cmd === 'set_component' && s.args[1] === 'AudioStream');
      expect(write).toBeDefined();
      expect(JSON.parse(write!.args[2])).toMatchObject({
        url: 'https://stream.example/live',
        playing: false,
      });
    });
  });

  describe('ParticleSystem (#1467)', () => {
    const getPS = () => ctx.getForwardableComponent('core::ParticleSystem') as never;

    it('should forward a ParticleSystem live, collapsing the shape oneof', async () => {
      // Running (not frozen) bridge so the authored value is forwarded as-is.
      bridge.disconnect();
      const running = createForwardEditBridge({
        context: ctx,
        engineWindow,
        shouldForward: () => true,
        isFrozen: () => false,
        send: async (cmd, args) => {
          sent.push({ cmd, args });
          return '';
        },
      });
      const PS = getPS() as { create: (e: unknown, v: unknown) => void };
      const entity = ctx.engine.addEntity();
      ctx.Name.create(entity, { value: 'Sparks' });
      PS.create(entity, { rate: 20, shape: { $case: 'sphere', sphere: { radius: 2 } } });
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));
      await new Promise(r => setTimeout(r, 0));

      const write = sent.find(s => s.cmd === 'set_component' && s.args[1] === 'ParticleSystem');
      expect(write).toBeDefined();
      const value = JSON.parse(write!.args[2]);
      expect(value.shape).toEqual({ sphere: { radius: 2 } }); // $case collapsed for serde
      running.disconnect();
    });

    it('should force playbackState PS_PAUSED (1) while frozen', async () => {
      const PS = getPS() as { create: (e: unknown, v: unknown) => void };
      const entity = ctx.engine.addEntity();
      ctx.Name.create(entity, { value: 'Sparks' });
      PS.create(entity, { rate: 20, playbackState: 0 }); // authored PS_PLAYING
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));
      await new Promise(r => setTimeout(r, 0));

      const write = sent.find(s => s.cmd === 'set_component' && s.args[1] === 'ParticleSystem');
      expect(write).toBeDefined();
      expect(JSON.parse(write!.args[2]).playbackState).toBe(1); // paused in a frozen editor
    });

    it('should restore the authored playbackState on unfreeze', async () => {
      const PS = getPS() as { create: (e: unknown, v: unknown) => void };
      const entity = ctx.engine.addEntity();
      ctx.Name.create(entity, { value: 'Sparks' });
      PS.create(entity, { rate: 20, playbackState: 0 });
      await ctx.engine.update(1);
      await new Promise(r => setTimeout(r, 0));
      sent.length = 0;

      bridge.setSceneFrozen(false);
      await new Promise(r => setTimeout(r, 0));
      await new Promise(r => setTimeout(r, 0));

      const write = sent.find(s => s.cmd === 'set_component' && s.args[1] === 'ParticleSystem');
      expect(write).toBeDefined();
      expect(JSON.parse(write!.args[2]).playbackState).toBe(0); // resumes as authored
    });
  });

  describe('when the engine copy of the scene has state its code made', () => {
    // The scene's main() made a flashlight (600) parented to the camera, a sound
    // (601) parented to the player, a lamp (602) on the ground, and a night sky
    // with a disabled root light on the root entity.
    const NIGHT = {
      '0': {
        Skybox: { stars: { brightness: 6 } },
        SkyboxTime: { fixedTime: 5400 },
        LightSource: { active: false },
      },
      '600': {
        Transform: { ...IDENTITY, position: { x: 0, y: 0, z: 0.5 }, parent: 2 },
        LightSource: { active: true, intensity: 150000 },
      },
      '601': { Transform: { ...IDENTITY, position: { x: 0, y: 0, z: 0 }, parent: 1 } },
      '602': {
        Transform: { ...IDENTITY, position: { x: 4, y: 1, z: 4 }, parent: 0 },
        LightSource: { active: true },
      },
    };
    let frozen: boolean;
    let stopped: boolean;
    let hide: boolean;
    let snapshot: unknown;

    const drain = async () => {
      for (let i = 0; i < 4; i++) await new Promise(r => setTimeout(r, 0));
    };
    const lastVisibility = (entity: number): boolean | undefined => {
      const writes = sent.filter(
        s =>
          s.cmd === 'set_component' &&
          s.args[0] === String(entity) &&
          s.args[1] === 'VisibilityComponent',
      );
      const last = writes[writes.length - 1];
      return last ? JSON.parse(last.args[2]).visible : undefined;
    };
    const rootDeletes = () =>
      sent
        .filter(s => s.cmd === 'delete_component' && s.args[0] === '0')
        .map(s => s.args[1])
        .sort();
    const rootSets = () =>
      Object.fromEntries(
        sent
          .filter(s => s.cmd === 'set_component' && s.args[0] === '0')
          .map(s => [s.args[1], JSON.parse(s.args[2])]),
      );
    const clock = () => sent.filter(s => s.cmd === 'time').map(s => s.args.join(' '));
    // `hide` follows the editor's rule: stopped and the free camera.
    const play = () => {
      frozen = false;
      stopped = false;
      hide = false;
      bridge.setSceneFrozen(false);
      bridge.refreshCameraAttachments();
    };
    const pause = () => {
      frozen = true;
      bridge.setSceneFrozen(true);
      bridge.refreshCameraAttachments();
    };
    const stop = () => {
      frozen = true;
      stopped = true;
      hide = true;
      bridge.reconcileAfterReload();
    };

    beforeEach(() => {
      frozen = true;
      stopped = true;
      hide = true;
      snapshot = NIGHT;
      bridge.disconnect();
      bridge = createForwardEditBridge({
        context: ctx,
        engineWindow,
        shouldForward: () => true,
        isFrozen: () => frozen,
        isStopped: () => stopped,
        hideCameraAttachments: () => hide,
        send: async (cmd, args) => {
          sent.push({ cmd, args });
          if (cmd === 'crdt_snapshot') return JSON.stringify(snapshot);
          if (cmd === 'time' && args.length === 0) {
            return 'time 1:30 -> 1:30, speed 12 (elapsed: 5400)';
          }
          return '';
        },
      });
    });

    describe('and the scene is stopped with the free camera', () => {
      it('should hide the entities its code parented to the camera or the player, not the others', async () => {
        bridge.refreshEngineScene();
        await drain();

        expect(lastVisibility(600)).toBe(false);
        expect(lastVisibility(601)).toBe(false);
        expect(lastVisibility(602)).toBeUndefined();
      });

      it('should never drop the attachment or touch the attached LightSource', async () => {
        bridge.refreshEngineScene();
        await drain();

        expect(
          sent.some(
            s =>
              s.args[0] === '600' && (s.args[1] === 'AvatarAttach' || s.args[1] === 'LightSource'),
          ),
        ).toBe(false);
      });
    });

    describe('and the editor drives the player camera while stopped', () => {
      it('should forward the attachments as the scene made them', async () => {
        hide = false;
        bridge.refreshEngineScene();
        await drain();

        expect(lastVisibility(600)).toBeUndefined();
      });
    });

    describe('and the camera mode changes while stopped', () => {
      it('should hide the attachments in the free camera and show them in the player camera', async () => {
        hide = false;
        bridge.refreshEngineScene();
        await drain();

        hide = true;
        bridge.refreshCameraAttachments();
        await drain();
        expect(lastVisibility(600)).toBe(false);

        hide = false;
        bridge.refreshCameraAttachments();
        await drain();
        expect(lastVisibility(600)).toBe(true);
      });

      it('should give back the visibility the scene code set when it shows them again', async () => {
        snapshot = {
          ...NIGHT,
          '600': { ...NIGHT['600'], VisibilityComponent: { visible: false } },
        };
        bridge.refreshEngineScene();
        await drain();

        hide = false;
        bridge.refreshCameraAttachments();
        await drain();
        expect(lastVisibility(600)).toBe(false);
      });
    });

    describe('and the scene runs', () => {
      it('should show the attachments again and not read the engine copy', async () => {
        bridge.refreshEngineScene();
        await drain();
        sent.length = 0;

        play();
        bridge.refreshEngineScene();
        await drain();

        expect(lastVisibility(600)).toBe(true);
        expect(sent.some(s => s.cmd === 'crdt_snapshot')).toBe(false);
      });
    });

    describe('and an authored entity carries an AvatarAttach', () => {
      it('should hide it only when it follows the local player', async () => {
        const AvatarAttach = components.AvatarAttach(ctx.engine);
        const local = ctx.engine.addEntity();
        AvatarAttach.create(local, { anchorPointId: 0 });
        const other = ctx.engine.addEntity();
        AvatarAttach.create(other, {
          anchorPointId: 0,
          avatarId: '0x0000000000000000000000000000000000000001',
        });
        await ctx.engine.update(1);
        await drain();

        expect(lastVisibility(local)).toBe(false);
        expect(lastVisibility(other)).toBeUndefined();
      });
    });

    describe('and the scene is stopped', () => {
      it('should remove only the root lighting the scene set and pin the editor clock', async () => {
        bridge.refreshEngineScene();
        await drain();

        expect(rootDeletes()).toEqual(['LightSource', 'Skybox', 'SkyboxTime']);
        expect(clock()).toEqual(['12 0']);
      });
    });

    describe('and the scene starts to run (Play)', () => {
      it('should put back the scene values as they were, a disabled root light included, and run the clock', async () => {
        bridge.refreshEngineScene();
        await drain();
        sent.length = 0;

        play();
        await drain();

        expect(rootSets()).toEqual({
          LightSource: { active: false },
          Skybox: { stars: { brightness: 6 } },
          SkyboxTime: { fixedTime: 5400 },
        });
        expect(rootDeletes()).toEqual([]);
        expect(clock()).toEqual(['--resume']);
      });

      it('should not send a component the scene never set', async () => {
        snapshot = { '0': { SkyboxTime: { fixedTime: 5400 } } };
        bridge.refreshEngineScene();
        await drain();
        sent.length = 0;

        play();
        await drain();

        expect(Object.keys(rootSets())).toEqual(['SkyboxTime']);
      });

      it('should not apply the editor lighting when a read started before Play lands after it', async () => {
        bridge.refreshEngineScene();
        play();
        await drain();

        expect(rootDeletes()).toEqual([]);
        expect(clock()).toEqual(['--resume']);
      });
    });

    describe('and the scene is paused after running', () => {
      it('should keep the scene lighting and hold the clock where the sky is', async () => {
        bridge.refreshEngineScene();
        await drain();
        play();
        await drain();
        sent.length = 0;

        pause();
        bridge.refreshEngineScene();
        await drain();

        expect(sent.some(s => s.cmd === 'crdt_snapshot')).toBe(false);
        expect(rootDeletes()).toEqual([]);
        expect(rootSets()).toEqual({});
        expect(clock()).toEqual(['', '1.5 0']);
      });

      it('should keep the attachments shown', async () => {
        bridge.refreshEngineScene();
        await drain();
        play();
        await drain();

        pause();
        await drain();

        expect(lastVisibility(600)).toBe(true);
      });

      it('should only run the clock again on Play, leaving the lighting the scene has now', async () => {
        bridge.refreshEngineScene();
        await drain();
        play();
        await drain();
        pause();
        await drain();
        sent.length = 0;

        play();
        await drain();

        expect(rootSets()).toEqual({});
        expect(clock()).toEqual(['--resume']);
      });

      it('should show a root LightSource edit at once', async () => {
        bridge.refreshEngineScene();
        await drain();
        play();
        await drain();
        pause();
        await drain();
        sent.length = 0;

        const LightSource = components.LightSource(ctx.engine);
        LightSource.create(ctx.engine.RootEntity, { active: true, intensity: 3 });
        await ctx.engine.update(1);
        await drain();

        expect(rootSets().LightSource).toMatchObject({ active: true, intensity: 3 });
      });
    });

    describe('and an invisible item pairs a collider model with a placeholder', () => {
      let wall: number;
      const gltfWrites = () =>
        sent.filter(
          s =>
            (s.cmd === 'set_component' || s.cmd === 'delete_component') &&
            s.args[0] === String(wall) &&
            s.args[1] === 'GltfContainer',
        );

      beforeEach(async () => {
        const GltfContainer = components.GltfContainer(ctx.engine);
        const entity = ctx.engine.addEntity();
        wall = entity;
        ctx.Name.create(entity, { value: 'Invisible Wall' });
        GltfContainer.create(entity, {
          src: 'assets/wall_collider.glb',
          invisibleMeshesCollisionMask: 2,
        });
        ctx.editorComponents.Placeholder.create(entity, { src: 'assets/wall.glb' });
        await ctx.engine.update(1);
        await drain();
        play();
        await drain();
        sent.length = 0;
      });

      describe('and the scene is paused after running', () => {
        beforeEach(async () => {
          pause();
          await drain();
        });

        it('should leave the collider model in place, not the placeholder', () => {
          expect(gltfWrites()).toEqual([]);
        });

        describe('and it plays again', () => {
          beforeEach(async () => {
            play();
            await drain();
          });

          it('should not reload the model', () => {
            expect(gltfWrites()).toEqual([]);
          });
        });

        describe('and the scene is stopped', () => {
          beforeEach(async () => {
            stop();
            await drain();
          });

          it('should show the placeholder again', () => {
            expect(gltfWrites().map(s => JSON.parse(s.args[2]).src)).toEqual(['assets/wall.glb']);
          });
        });
      });
    });

    describe('and the editor agent finishes booting', () => {
      describe('and the scene is running', () => {
        beforeEach(async () => {
          play();
          await drain();
          sent.length = 0;
          bridge.syncSceneClock();
          await drain();
        });

        it('should run the day clock again', () => {
          expect(clock()).toEqual(['--resume']);
        });
      });

      describe('and the scene is stopped', () => {
        beforeEach(async () => {
          bridge.syncSceneClock();
          await drain();
        });

        it('should leave the clock to the editor lighting', () => {
          expect(clock()).toEqual([]);
        });
      });
    });

    describe('and the scene code parents an authored entity to the camera', () => {
      let torch: number;

      beforeEach(async () => {
        const entity = ctx.engine.addEntity();
        torch = entity;
        ctx.Name.create(entity, { value: 'Torch' });
        ctx.Transform.create(entity, {
          ...IDENTITY,
          position: { x: 4, y: 1, z: 4 },
          parent: ctx.engine.RootEntity,
        });
        await ctx.engine.update(1);
        snapshot = {
          ...NIGHT,
          [String(entity)]: {
            Transform: { ...IDENTITY, position: { x: 0, y: 0, z: 0 }, parent: 2 },
          },
        };
        stop();
        await drain();
      });

      it('should hide it while stopped with the free camera', () => {
        expect(lastVisibility(torch)).toBe(false);
      });

      describe('and the entity is then reparented in the inspector', () => {
        beforeEach(async () => {
          const other = ctx.engine.addEntity();
          ctx.Transform.getMutable(torch as never).parent = other;
          await ctx.engine.update(1);
          await drain();
        });

        it('should show it where the inspector put it', () => {
          expect(lastVisibility(torch)).toBe(true);
        });
      });
    });

    describe('and the scene is stopped and reset (Stop)', () => {
      it('should apply the editor lighting and hide the attachments again on the reloaded scene', async () => {
        bridge.refreshEngineScene();
        await drain();
        play();
        await drain();
        pause();
        await drain();
        sent.length = 0;

        stop();
        await drain();

        expect(rootDeletes()).toEqual(['LightSource', 'Skybox', 'SkyboxTime']);
        expect(clock()).toEqual(['12 0']);
        expect(lastVisibility(600)).toBe(false);
      });
    });

    describe('and the inspector edits the root LightSource', () => {
      it('should save it but show it only on Play, with the edited value', async () => {
        bridge.refreshEngineScene();
        await drain();
        const LightSource = components.LightSource(ctx.engine);
        LightSource.create(ctx.engine.RootEntity, { active: true, intensity: 3 });
        await ctx.engine.update(1);
        await drain();
        expect(rootSets()).toEqual({});

        play();
        await drain();
        expect(rootSets().LightSource).toMatchObject({ active: true, intensity: 3 });
      });

      describe('and the scene is then stopped, and its code sets its own root light', () => {
        it('should show the value the scene code set on the next Play', async () => {
          bridge.refreshEngineScene();
          await drain();
          const LightSource = components.LightSource(ctx.engine);
          LightSource.create(ctx.engine.RootEntity, { active: true, intensity: 3 });
          await ctx.engine.update(1);
          await drain();
          play();
          await drain();

          stop();
          snapshot = { '0': { LightSource: { active: true, intensity: 7 } } };
          bridge.refreshEngineScene();
          await drain();
          sent.length = 0;
          play();
          await drain();

          expect(rootSets().LightSource).toMatchObject({ active: true, intensity: 7 });
        });
      });
    });
  });

  describe('when forwarding arms after the initial load', () => {
    afterEach(() => {
      vi.useRealTimers();
    });

    it('should apply the same editor lighting as a Stop, without waiting for an edit', async () => {
      vi.useFakeTimers();
      const armSent: Array<{ cmd: string; args: string[] }> = [];
      const armed = createForwardEditBridge({
        context: ctx,
        engineWindow,
        send: async (cmd, args) => {
          armSent.push({ cmd, args });
          return cmd === 'crdt_snapshot'
            ? JSON.stringify({ '0': { EnvironmentFog: { density: 0.4 } } })
            : '';
        },
      });
      expect(armSent).toEqual([]);

      await vi.advanceTimersByTimeAsync(3000);

      const snapshotAt = armSent.findIndex(s => s.cmd === 'crdt_snapshot');
      const deleteAt = armSent.findIndex(
        s => s.cmd === 'delete_component' && s.args.join(' ') === '0 EnvironmentFog',
      );
      expect(snapshotAt).toBeGreaterThanOrEqual(0);
      expect(deleteAt).toBeGreaterThan(snapshotAt);
      expect(armSent.some(s => s.cmd === 'time' && s.args.join(' ') === '12 0')).toBe(true);
      armed.disconnect();
    });
  });

  describe('when a console command fails', () => {
    it('should report via onError and not throw into the change loop', async () => {
      const errors: string[] = [];
      const failCtx = new BevySceneContext();
      const off = createForwardEditBridge({
        context: failCtx,
        engineWindow,
        shouldForward: () => true,
        send: async () => {
          throw new Error('engine console gone');
        },
        onError: label => errors.push(label),
      });

      const entity = failCtx.engine.addEntity();
      failCtx.Transform.create(entity, { ...IDENTITY, position: { x: 1, y: 1, z: 1 } });
      await failCtx.engine.update(1);
      // let the fire-and-forget rejection settle
      await Promise.resolve();
      await Promise.resolve();

      expect(errors.length).toBeGreaterThanOrEqual(1);
      off.disconnect();
      failCtx.dispose();
    });
  });
});
