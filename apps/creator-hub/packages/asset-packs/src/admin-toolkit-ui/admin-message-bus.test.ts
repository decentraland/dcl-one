import { beforeEach, describe, expect, it, vi } from 'vitest';
import { Engine } from '@dcl/ecs';
import { ReadWriteByteBuffer } from '@dcl/ecs/dist/serialization/ByteBuffer';
import type { Entity, PBVideoPlayer } from '@dcl/ecs';
import { createComponents, getComponents } from '../definitions';
import { getExplorerComponents } from '../components';
import { initAdminMessageBus } from './admin-message-bus';
import { createVideoPlayerControls } from './VideoControl/utils';

const comms = vi.hoisted(() => ({
  handlers: new Map<string, (payload: unknown, sender: string) => void>(),
  send: vi.fn(async (_request: { message: string }) => ({})),
}));
vi.mock('@dcl/sdk/message-bus', () => ({
  MessageBus: class {
    on(type: string, handler: (payload: unknown, sender: string) => void) {
      comms.handlers.set(type, handler);
    }
  },
}));
vi.mock('~system/CommunicationsController', () => ({ send: comms.send }));
vi.mock('./fetch-utils', () => ({ isPreview: () => false }));
vi.mock('./VideoControl/index', () => ({ DEFAULT_VOLUME: 1 }));

function setup(playing = true) {
  const engine = Engine();
  createComponents(engine);
  const { VideoPlayer } = getExplorerComponents(engine);
  const { AdminTools } = getComponents(engine);
  const screen = engine.addEntity();
  VideoPlayer.create(screen, { src: 'video.mp4', playing, volume: 0.5, position: 0 });
  const admin = engine.addEntity();
  const config = AdminTools.schema.create();
  config.videoControl.isEnabled = true;
  config.videoControl.disableVideoPlayersSound = false;
  config.videoControl.videoPlayers = [{ entity: screen, customName: 'Screen' }];
  AdminTools.create(admin, config);
  const bus = initAdminMessageBus(engine, [{ address: 'admin' } as never], admin);
  const controls = createVideoPlayerControls(screen, engine);
  // Read component updates through a real SDK transport: identical serialized requests are dropped.
  const sent: Partial<PBVideoPlayer>[] = [];
  engine.addTransport({
    filter: message =>
      'componentId' in message &&
      message.componentId === VideoPlayer.componentId &&
      message.entityId === screen,
    send: async message => {
      const bytes = message as Uint8Array;
      const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
      for (let offset = 0; offset < bytes.length; ) {
        const length = view.getUint32(offset, true);
        if (view.getUint32(offset + 4, true) === 1) {
          sent.push(
            VideoPlayer.schema.deserialize(
              new ReadWriteByteBuffer(bytes.subarray(offset + 24, offset + length)),
            ),
          );
        }
        offset += length;
      }
    },
  });
  const tick = (dt = 0.1) => engine.update(dt);
  return { engine, screen, bus, controls, VideoPlayer, sent, tick };
}

beforeEach(() => {
  comms.handlers.clear();
  comms.send.mockClear();
});

describe('Toolkit Restart with the published SDK', () => {
  for (const playing of [true, false]) {
    it(`serializes pause/-1 before play/0 on every Restart (initial playing=${playing})`, async () => {
      const p = setup(playing);
      await p.tick();
      for (let i = 0; i < 3; i++) {
        p.controls.restart();
        await p.tick(10); // A stalled first update still cannot consume both phases.
        expect(p.sent[p.sent.length - 1]).toMatchObject({ playing: false, position: -1 });
        await p.tick();
        expect(p.VideoPlayer.get(p.screen).playing).toBe(false);
        await p.tick();
        expect(p.sent[p.sent.length - 1]).toMatchObject({ playing: true, position: 0 });
      }
    });
  }

  it('does not retain a seek on Pause, volume or Resume after Restart', async () => {
    const p = setup();
    p.controls.restart();
    await p.tick();
    await p.tick(0.2);
    p.controls.pause();
    await p.tick();
    expect(p.VideoPlayer.get(p.screen)).toMatchObject({ playing: false, position: undefined });
    p.controls.setVolumeExact(0.7);
    p.controls.play();
    await p.tick();
    expect(p.VideoPlayer.get(p.screen)).toMatchObject({
      playing: true,
      position: undefined,
      volume: 0.7,
    });
  });

  for (const action of ['pause', 'play', 'source'] as const) {
    it(`cancels pending Restart when a newer ${action} command arrives`, async () => {
      const p = setup();
      p.controls.restart();
      await p.tick();
      if (action === 'source') p.controls.setSource('other.mp4');
      else p.controls[action]();
      await p.tick(1);
      expect(p.VideoPlayer.get(p.screen)).toMatchObject({
        src: action === 'source' ? 'other.mp4' : 'video.mp4',
        playing: action !== 'pause',
        position: undefined,
      });
    });
  }

  it('keeps volume and loop changes made while Restart is pending', async () => {
    const p = setup();
    p.controls.restart();
    await p.tick();
    p.controls.setVolumeExact(0.8);
    p.controls.setLoop(true);
    await p.tick(0.2);
    expect(p.VideoPlayer.get(p.screen)).toMatchObject({
      playing: true,
      position: 0,
      volume: 0.8,
      loop: true,
    });
  });

  it('applies batched remote Restarts in separate local updates, after sender validation', async () => {
    const p = setup();
    const receive = comms.handlers.get('admin:set-video')!;
    receive({ entity: p.screen, playing: true, position: 0 }, 'attacker');
    expect(p.VideoPlayer.get(p.screen).playing).toBe(true);
    receive({ entity: p.screen, playing: true, position: 0 }, 'admin');
    receive({ entity: p.screen, playing: true, position: 0 }, 'admin');
    await p.tick(10);
    expect(p.sent[p.sent.length - 1]).toMatchObject({ playing: false, position: -1 });
    await p.tick(0.2);
    expect(p.sent[p.sent.length - 1]).toMatchObject({ playing: true, position: 0 });
  });

  it('drops pending work when the screen is removed', async () => {
    const p = setup();
    p.controls.restart();
    await p.tick();
    p.VideoPlayer.deleteFrom(p.screen);
    await p.tick(1);
    expect(p.VideoPlayer.has(p.screen)).toBe(false);
  });

  it('syncs only stable playback state to late joiners, without replaying a seek', async () => {
    const p = setup();
    p.controls.restart();
    await p.tick();
    comms.send.mockClear();
    const request = comms.handlers.get('admin:request-state')!;
    request({}, 'viewer');
    expect(comms.send).not.toHaveBeenCalled();
    await p.tick(0.2);
    request({}, 'viewer');
    const response = JSON.parse(comms.send.mock.calls[0][0].message);
    expect(response.message).toBe('admin:sync-state');
    expect(response.payload.video).toEqual([
      { entity: p.screen, src: 'video.mp4', playing: true, volume: 0.5 },
    ]);
  });

  it('keeps independent Restarts for different screens', async () => {
    const p = setup();
    const other = p.engine.addEntity();
    p.VideoPlayer.create(other, { src: 'other.mp4', playing: true });
    p.controls.restart();
    p.bus.emitSetVideo(other as Entity, { playing: true, position: 0 });
    await p.tick();
    await p.tick(0.2);
    expect(p.VideoPlayer.get(other)).toMatchObject({ playing: true, position: 0 });
    expect(p.VideoPlayer.get(p.screen)).toMatchObject({ playing: true, position: 0 });
  });
});
