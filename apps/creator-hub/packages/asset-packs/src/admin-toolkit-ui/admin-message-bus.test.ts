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

function setup(playing = true, options: { position?: number; secondScreen?: boolean } = {}) {
  const engine = Engine();
  createComponents(engine);
  const { VideoPlayer } = getExplorerComponents(engine);
  const { AdminTools } = getComponents(engine);
  const screen = engine.addEntity();
  VideoPlayer.create(screen, {
    src: 'video.mp4',
    playing,
    volume: 0.5,
    position: options.position ?? 0,
  });
  const other = options.secondScreen ? engine.addEntity() : undefined;
  if (other !== undefined) VideoPlayer.create(other, { src: 'other.mp4', playing: true });
  const admin = engine.addEntity();
  const config = AdminTools.schema.create();
  config.videoControl.isEnabled = true;
  config.videoControl.disableVideoPlayersSound = false;
  config.videoControl.videoPlayers = [{ entity: screen, customName: 'Screen' }];
  if (other !== undefined) {
    config.videoControl.videoPlayers.push({ entity: other, customName: 'Other screen' });
  }
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
  return { engine, screen, other, bus, controls, VideoPlayer, sent, tick };
}

beforeEach(() => {
  comms.handlers.clear();
  comms.send.mockClear();
});

describe('Toolkit Restart with the published SDK', () => {
  for (const action of ['pause', 'play', 'volume', 'loop'] as const) {
    it(`preserves an authored start position on the first ${action} command`, async () => {
      const p = setup(true, { position: 30 });
      await p.tick();
      if (action === 'volume') p.controls.setVolumeExact(0.7);
      else if (action === 'loop') p.controls.setLoop(true);
      else p.controls[action]();
      await p.tick();
      expect(p.VideoPlayer.get(p.screen).position).toBe(30);
      expect(p.sent[p.sent.length - 1].position).toBe(30);
    });
  }

  it('preserves a scene position that replaces the Toolkit Restart position', async () => {
    const p = setup();
    p.controls.restart();
    await p.tick();
    await p.tick(0.2);
    p.VideoPlayer.getMutable(p.screen).position = 45;
    p.controls.pause();
    await p.tick();
    expect(p.sent[p.sent.length - 1]).toMatchObject({ playing: false, position: 45 });
  });

  it('preserves an explicit seek on subsequent settings commands', async () => {
    const p = setup();
    p.bus.emitSetVideo(p.screen, { position: 12 });
    await p.tick();
    p.controls.setVolumeExact(0.7);
    await p.tick();
    expect(p.sent[p.sent.length - 1].position).toBe(12);
    expect(p.sent[p.sent.length - 1].volume).toBeCloseTo(0.7);
  });

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
    expect(comms.send).toHaveBeenCalledTimes(1);
    const response = JSON.parse(comms.send.mock.calls[0][0].message);
    expect(response.message).toBe('admin:sync-state');
    expect(response.payload.video).toEqual([
      { entity: p.screen, src: 'video.mp4', playing: true, volume: 0.5 },
    ]);
    // Sending the target state must not apply it locally before the paused stage is delivered.
    expect(p.VideoPlayer.get(p.screen)).toMatchObject({ playing: false, position: -1 });
    await p.tick(0.1);
    expect(p.sent[p.sent.length - 1]).toMatchObject({ playing: false, position: -1 });
    await p.tick(0.1);
    expect(p.sent[p.sent.length - 1]).toMatchObject({ playing: true, position: 0 });
  });

  it('delivers both screens to a late joiner while one Restart is pending', async () => {
    const p = setup(true, { secondScreen: true });
    p.controls.setSource('active.mp4');
    p.bus.emitSetVideo(p.other!, { src: 'active-other.mp4', playing: true });
    p.controls.restart();
    await p.tick();
    const request = comms.handlers.get('admin:request-state')!;
    const viewer = setup(true, { secondScreen: true });
    comms.send.mockClear();
    request({}, 'viewer');
    const response = JSON.parse(comms.send.mock.calls[0][0].message);
    expect(response.payload.video).toHaveLength(2);
    const receive = comms.handlers.get('admin:sync-state')!;
    receive(response.payload, 'attacker');
    expect(viewer.VideoPlayer.get(viewer.screen).src).toBe('video.mp4');
    receive(response.payload, 'admin');
    await viewer.tick(2);
    expect(viewer.VideoPlayer.get(viewer.screen)).toMatchObject({
      src: 'active.mp4',
      playing: true,
    });
    expect(viewer.VideoPlayer.get(viewer.other!)).toMatchObject({
      src: 'active-other.mp4',
      playing: true,
    });
    expect(p.VideoPlayer.get(p.screen)).toMatchObject({ playing: false, position: -1 });
    expect(
      comms.send.mock.calls.filter(
        ([message]) => JSON.parse(message.message).message === 'admin:request-state',
      ),
    ).toHaveLength(0);
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
