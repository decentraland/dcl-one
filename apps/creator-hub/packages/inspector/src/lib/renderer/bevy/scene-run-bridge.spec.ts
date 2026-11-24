import { createSceneRunBridge } from './scene-run-bridge';

/**
 * The scene-run bridge posts `set-scene-frozen` to the agent: running=true →
 * frozen:false (run live), running=false → frozen:true (static). No reverse
 * channel — the inspector owns the intent. Driven with a fake channel recorder.
 */
describe('createSceneRunBridge', () => {
  let posted: unknown[];
  let closed: boolean;
  let bridge: ReturnType<typeof createSceneRunBridge>;

  beforeEach(() => {
    posted = [];
    closed = false;
    bridge = createSceneRunBridge({
      channel: {
        postMessage: m => posted.push(m),
        close: () => {
          closed = true;
        },
      },
    });
  });

  afterEach(() => bridge.disconnect());

  it('should post frozen:false when running the scene live', () => {
    bridge.setRunning(true);
    expect(posted).toEqual([{ to: 'scene', msg: { kind: 'set-scene-frozen', frozen: false } }]);
  });

  it('should post frozen:true when freezing the scene', () => {
    bridge.setRunning(false);
    expect(posted).toEqual([{ to: 'scene', msg: { kind: 'set-scene-frozen', frozen: true } }]);
  });

  it('should track the last requested run state via isRunning', () => {
    expect(bridge.isRunning()).toBe(false); // boots frozen
    bridge.setRunning(true);
    expect(bridge.isRunning()).toBe(true);
    bridge.setRunning(false);
    expect(bridge.isRunning()).toBe(false);
  });

  describe('when the scene is played, paused and stopped', () => {
    it('should be stopped only before the first Play and after Stop', () => {
      expect(bridge.isStopped()).toBe(true);
      bridge.setRunning(true);
      expect(bridge.isStopped()).toBe(false);
      bridge.setRunning(false);
      expect(bridge.isStopped()).toBe(false);
      bridge.reset();
      expect(bridge.isStopped()).toBe(true);
    });
  });

  describe('when the engine restarts the scene outside a Stop', () => {
    it('should be stopped again if paused, and not while running', () => {
      bridge.setRunning(true);
      bridge.setRunning(false);
      bridge.markRestarted();
      expect(bridge.isStopped()).toBe(true);

      bridge.setRunning(true);
      bridge.markRestarted();
      expect(bridge.isStopped()).toBe(false);
    });
  });

  describe('when a Play waits on a reset', () => {
    it('should not be stopped once the reset completes', () => {
      bridge.reset();
      bridge.markRunPending();
      expect(bridge.isStopped()).toBe(false);
    });
  });

  it('should post reset-scene when reset (Stop)', () => {
    bridge.reset();
    expect(posted).toEqual([{ to: 'scene', msg: { kind: 'reset-scene' } }]);
  });

  it('should close the channel on disconnect', () => {
    bridge.disconnect();
    expect(closed).toBe(true);
  });

  describe('when onResetComplete is provided', () => {
    let onResetComplete: (ok: boolean) => void;
    let calls: boolean[];
    let channel: {
      postMessage: (m: unknown) => void;
      onmessage: ((ev: { data: unknown }) => void) | null;
      close: () => void;
    };

    beforeEach(() => {
      calls = [];
      onResetComplete = ok => calls.push(ok);
      channel = { postMessage: () => {}, onmessage: null, close: () => {} };
      bridge = createSceneRunBridge({ channel, onResetComplete });
    });

    it('should invoke onResetComplete when the agent posts reset-complete', () => {
      channel.onmessage?.({ data: { to: 'page', msg: { kind: 'reset-complete', ok: true } } });
      channel.onmessage?.({ data: { to: 'page', msg: { kind: 'reset-complete', ok: false } } });
      expect(calls).toEqual([true, false]);
    });

    it('should ignore messages that are not addressed to the page', () => {
      channel.onmessage?.({ data: { to: 'scene', msg: { kind: 'reset-complete', ok: true } } });
      expect(calls).toEqual([]);
    });

    it('should ignore agent messages of other kinds', () => {
      channel.onmessage?.({ data: { to: 'page', msg: { kind: 'editor-ready' } } });
      expect(calls).toEqual([]);
    });

    it('should detach the listener on disconnect', () => {
      bridge.disconnect();
      expect(channel.onmessage).toBeNull();
    });
  });
});
