import { describe, expect, it, vi } from 'vitest';
import { Engine, type Entity } from '@dcl/ecs';
import { createComponents } from '../src/definitions';
import { getExplorerComponents } from '../src/components';
import { initAdminMessageBus } from '../src/admin-toolkit-ui/admin-message-bus';

const state = vi.hoisted(() => ({ screens: [] as { entity: Entity }[] }));
vi.mock('@dcl/sdk/message-bus', () => ({
  MessageBus: class {
    on() {}
  },
}));
vi.mock('~system/CommunicationsController', () => ({ send: async () => ({}) }));
vi.mock('../src/admin-toolkit-ui/fetch-utils', () => ({ isPreview: () => false }));
vi.mock('../src/admin-toolkit-ui/VideoControl/utils', () => ({
  getVideoPlayers: () => state.screens,
}));
vi.mock('../src/admin-toolkit-ui/utils', () => ({ setInterval: vi.fn(), clearInterval: vi.fn() }));

function setup() {
  const engine = Engine();
  createComponents(engine);
  const { VideoPlayer } = getExplorerComponents(engine);
  const screen = engine.addEntity();
  state.screens = [{ entity: screen }];
  VideoPlayer.create(screen, {
    src: 'https://example.com/video.mp4',
    playing: true,
    playbackRate: 1.5,
  });
  const bus = initAdminMessageBus(engine, [], engine.addEntity());
  const updates = () => Array.from(engine.getComponent(VideoPlayer.componentId).getCrdtUpdates());
  updates();
  return { screen, bus, VideoPlayer, updates };
}

describe('admin video commands', () => {
  it('sends repeated identical restart commands through the SDK serialization boundary', () => {
    const { bus, screen, updates } = setup();
    bus.emitSetVideo(screen, { playing: true, position: 0 });
    expect(updates()).toHaveLength(1);
    bus.emitSetVideo(screen, { playing: true, position: 0 });
    expect(updates()).toHaveLength(1);
  });

  it('does not repeat a seek on volume, pause or resume and preserves unrelated video fields', () => {
    const { bus, screen, VideoPlayer } = setup();
    bus.emitSetVideo(screen, { playing: true, position: 0 });
    expect(VideoPlayer.get(screen).position).toBe(0);
    for (const command of [{ volume: 0.5 }, { playing: false }, { playing: true }]) {
      bus.emitSetVideo(screen, command);
      expect(VideoPlayer.get(screen).position).toBeUndefined();
      expect(VideoPlayer.get(screen).playbackRate).toBe(1.5);
    }
  });
});
