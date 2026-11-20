import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import * as components from '@dcl/ecs/dist/components';

import { BevySceneContext } from './BevySceneContext';
import { createAvatarHitboxesBridge } from './avatar-hitboxes-bridge';

describe('createAvatarHitboxesBridge', () => {
  let ctx: BevySceneContext;
  let posted: Array<{ to?: string; msg?: { kind?: string; hitboxes?: unknown } }>;
  let onmessage: ((ev: { data: unknown }) => void) | null;
  let running: boolean;
  let bridge: ReturnType<typeof createAvatarHitboxesBridge>;

  const ROTATION = { x: 0, y: 0.7071, z: 0, w: 0.7071 };
  const SCALE = { x: 2, y: 2, z: 2 };

  const lastHitboxes = () =>
    posted.filter(p => p.msg?.kind === 'set-avatar-hitboxes').at(-1)?.msg?.hitboxes as
      | { entity: number }[]
      | undefined;

  beforeEach(() => {
    ctx = new BevySceneContext();
    posted = [];
    onmessage = null;
    running = false;
    bridge = createAvatarHitboxesBridge({
      context: ctx,
      isRunning: () => running,
      channel: {
        postMessage: m => posted.push(m as never),
        set onmessage(fn: ((ev: { data: unknown }) => void) | null) {
          onmessage = fn;
        },
        get onmessage() {
          return onmessage;
        },
        close: () => {},
      },
    });
  });

  afterEach(() => {
    bridge.disconnect();
    ctx.dispose();
  });

  const addNpc = async (position = { x: 4, y: 0, z: 6 }) => {
    const entity = ctx.engine.addEntity();
    ctx.Transform.create(entity, {
      position,
      rotation: ROTATION,
      scale: SCALE,
      parent: ctx.engine.RootEntity,
    });
    components.AvatarShape(ctx.engine).create(entity, { id: '', wearables: [], emotes: [] });
    await ctx.engine.update(1);
    return entity;
  };

  it('should post the pose of each NPC', async () => {
    const entity = await addNpc();

    expect(lastHitboxes()).toEqual([
      { entity, position: { x: 4, y: 0, z: 6 }, rotation: ROTATION, scale: SCALE },
    ]);
  });

  it('should follow an NPC that moves', async () => {
    const entity = await addNpc();
    ctx.Transform.getMutable(entity).position = { x: 1, y: 2, z: 3 };
    await ctx.engine.update(1);

    expect(lastHitboxes()).toEqual([expect.objectContaining({ position: { x: 1, y: 2, z: 3 } })]);
  });

  it('should leave out a hidden NPC', async () => {
    const entity = await addNpc();
    ctx.editorComponents.Hide.create(entity, { value: true });
    await ctx.engine.update(1);

    expect(lastHitboxes()).toEqual([]);
  });

  it('should leave out the Player, whose AvatarShape changes the player avatar', async () => {
    components.AvatarShape(ctx.engine).create(ctx.engine.PlayerEntity, {
      id: '',
      wearables: [],
      emotes: [],
    });
    await ctx.engine.update(1);

    expect(lastHitboxes() ?? []).toEqual([]);
  });

  it('should clear the hitboxes while the scene runs and restore them after', async () => {
    await addNpc();
    running = true;
    bridge.refresh();
    expect(lastHitboxes()).toEqual([]);

    running = false;
    bridge.refresh();
    expect(lastHitboxes()).toHaveLength(1);
  });

  it('should resend the hitboxes when the agent reboots', async () => {
    await addNpc();
    const before = posted.length;
    onmessage?.({ data: { to: 'page', msg: { kind: 'editor-ready' } } });

    expect(posted.length).toBe(before + 1);
  });
});
