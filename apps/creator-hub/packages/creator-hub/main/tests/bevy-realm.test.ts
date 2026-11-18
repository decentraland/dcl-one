import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { BEVY_REALM_SUPERVISION_EVENT } from '/shared/types/ipc';

import type { Child } from '../src/modules/bin';
import type * as RealmServerLogs from '../src/modules/realm-server-logs';

const mocks = vi.hoisted(() => ({
  child: undefined as unknown,
  send: vi.fn(),
  spawnsMultiplayerServer: vi.fn(async () => false),
  previewRunsServer: vi.fn(() => false),
}));

vi.mock('electron-log/main', () => ({ default: { info: vi.fn(), warn: vi.fn(), error: vi.fn() } }));
vi.mock('../src/modules/bin', () => ({ run: vi.fn(() => mocks.child) }));
vi.mock('../src/modules/port', () => ({ getAvailablePort: vi.fn(async () => 8004) }));
vi.mock('../src/modules/analytics', () => ({
  getProjectId: vi.fn(async () => 'id'),
  track: vi.fn(),
}));
vi.mock('../src/mainWindow', () => ({ MAIN_WINDOW_ID: 'main' }));
vi.mock('../src/modules/window', () => ({
  getWindow: () => ({ isDestroyed: () => false, webContents: { send: mocks.send } }),
}));
vi.mock('../src/modules/realm-server-logs', async importOriginal => ({
  ...(await importOriginal<typeof RealmServerLogs>()),
  spawnsMultiplayerServer: mocks.spawnsMultiplayerServer,
  previewRunsServer: mocks.previewRunsServer,
}));

import {
  MAX_RESTARTS,
  RESTART_WINDOW_MS,
  ensure,
  kill,
  killAllRealms,
  planRestart,
  realmProvidesServer,
  start,
} from '../src/modules/bevy-realm';
import { run } from '../src/modules/bin';
import { getAvailablePort } from '../src/modules/port';
import { isRealmDown } from '../src/modules/realm-status';

type FakeChild = {
  child: Child;
  exit: () => void;
  crash: (err: Error) => void;
  release: () => void;
  /** One output chunk, delivered to every matcher as the real process would. */
  print: (chunk: string) => void;
};

function createFakeChild(opts: { holdReady?: boolean } = {}): FakeChild {
  let alive = true;
  let resolveWait!: (buf: Buffer) => void;
  let rejectWait!: (err: Error) => void;
  const waitPromise = new Promise<Buffer>((resolve, reject) => {
    resolveWait = resolve;
    rejectWait = reject;
  });
  waitPromise.catch(() => {});
  let release: () => void = () => {};
  const ready = opts.holdReady
    ? new Promise<string>(resolve => {
        release = () => resolve('ready');
      })
    : Promise.resolve('ready');
  const exit = () => {
    alive = false;
    resolveWait(Buffer.from(''));
  };
  const handlers: ((data?: string) => void)[] = [];
  const child = {
    on: vi.fn((_pattern: RegExp, handler: (data?: string) => void) => handlers.push(handler) - 1),
    once: vi.fn(() => 1),
    off: vi.fn(),
    wait: () => waitPromise,
    waitFor: vi.fn(() => ready),
    kill: vi.fn(async () => exit()),
    alive: () => alive,
    stdall: () => [],
  } as unknown as Child;
  return {
    child,
    exit,
    crash: err => {
      alive = false;
      rejectWait(err);
    },
    release: () => release(),
    print: chunk => handlers.forEach(handler => handler(chunk)),
  };
}

const flush = () =>
  vi.isFakeTimers()
    ? vi.advanceTimersByTimeAsync(0)
    : new Promise(resolve => setTimeout(resolve, 0));

function supervisionEvents() {
  return mocks.send.mock.calls
    .filter(([channel]) => channel === BEVY_REALM_SUPERVISION_EVENT)
    .map(([, event]) => event);
}

function stoppedEvents() {
  return supervisionEvents().filter(event => event.kind === 'stopped');
}

function spawnedArgs(call = 0): string[] {
  return vi.mocked(run).mock.calls[call][2]!.args!;
}

const SERVER_EXITED =
  '⚠️ WARNING! Multiplayer Server exited with code 1. The preview keeps running without it, but clients wait for state sync.\n';

describe('bevy-realm supervision', () => {
  let path: string;
  let fake: FakeChild;

  beforeEach(() => {
    path = '/scenes/demo';
    fake = createFakeChild();
    mocks.child = fake.child;
  });

  afterEach(async () => {
    await killAllRealms();
    vi.useRealTimers();
    mocks.send.mockClear();
    vi.mocked(run).mockClear();
    mocks.spawnsMultiplayerServer.mockResolvedValue(false);
    mocks.previewRunsServer.mockReturnValue(false);
  });

  describe('when a running realm process exits on its own', () => {
    beforeEach(async () => {
      await start(path);
      fake.exit();
      await flush();
    });

    it('should mark the realm down', () => {
      expect(isRealmDown(path)).toBe(true);
    });

    it('should tell the editor it stopped and restarts after the first backoff', () => {
      expect(supervisionEvents()).toEqual([
        { kind: 'stopped', path, cause: 'realm', message: expect.any(String), restartInMs: 1_000 },
      ]);
    });

    describe('and the editor starts it again', () => {
      let next: FakeChild;
      let restarting: Promise<unknown>;

      beforeEach(async () => {
        next = createFakeChild({ holdReady: true });
        mocks.child = next.child;
        restarting = start(path);
        await flush();
      });

      it('should keep the realm down until the new one is serving', async () => {
        expect(isRealmDown(path)).toBe(true);
        next.release();
        await restarting;
        expect(isRealmDown(path)).toBe(false);
      });
    });

    describe('and the user leaves it with kill', () => {
      beforeEach(async () => {
        await kill(path);
      });

      it('should clear the down state', () => {
        expect(isRealmDown(path)).toBe(false);
      });
    });

    describe('and the app quits', () => {
      beforeEach(async () => {
        await killAllRealms();
      });

      it('should clear the down state', () => {
        expect(isRealmDown(path)).toBe(false);
      });
    });
  });

  describe('when a running realm process fails with a non-zero exit code', () => {
    beforeEach(async () => {
      await start(path);
      fake.crash(new Error('exited with code=1'));
      await flush();
    });

    it('should mark the realm down', () => {
      expect(isRealmDown(path)).toBe(true);
    });

    it('should send the stopped event with the failure in its message', () => {
      expect(stoppedEvents()).toEqual([
        expect.objectContaining({ cause: 'realm', message: expect.stringContaining('code=1') }),
      ]);
    });
  });

  describe('when a realm process exits before it is serving', () => {
    let booting: FakeChild;
    let starting: Promise<unknown>;

    beforeEach(async () => {
      booting = createFakeChild({ holdReady: true });
      mocks.child = booting.child;
      starting = start(path);
      starting.catch(() => {});
      await flush();
      booting.exit();
    });

    it('should reject the start instead of waiting forever', async () => {
      await expect(starting).rejects.toThrow('exited before it was ready');
    });

    it('should not send a stopped event', async () => {
      await starting.catch(() => {});
      expect(supervisionEvents()).toEqual([]);
    });

    describe('and the editor starts it again', () => {
      beforeEach(async () => {
        await starting.catch(() => {});
        mocks.child = createFakeChild().child;
      });

      it('should spawn a fresh realm', async () => {
        await expect(start(path)).resolves.toEqual({
          url: 'http://localhost:8004',
          wsUrl: 'ws://localhost:8004/data-layer',
        });
      });
    });
  });

  describe('when a realm is killed while it is still booting', () => {
    let starting: Promise<unknown>;

    beforeEach(async () => {
      mocks.child = createFakeChild({ holdReady: true }).child;
      starting = start(path);
      starting.catch(() => {});
      await flush();
      await kill(path);
    });

    it('should settle the pending start', async () => {
      await expect(starting).rejects.toThrow();
    });
  });

  describe('when a running realm is killed deliberately', () => {
    beforeEach(async () => {
      await start(path);
      await kill(path);
      await flush();
    });

    it('should not mark the realm down', () => {
      expect(isRealmDown(path)).toBe(false);
    });

    it('should not send a stopped event', () => {
      expect(supervisionEvents()).toEqual([]);
    });
  });

  describe('when a realm exits while the app is quitting', () => {
    beforeEach(async () => {
      const first = createFakeChild();
      const second = createFakeChild();
      vi.mocked(first.child.kill).mockImplementation(async () => {
        second.exit();
        await flush();
        first.exit();
      });
      mocks.child = first.child;
      await start('/scenes/one');
      mocks.child = second.child;
      await start('/scenes/two');
      await killAllRealms();
      await flush();
    });

    it('should not send a stopped event', () => {
      expect(supervisionEvents()).toEqual([]);
    });
  });
});

describe('planRestart', () => {
  const now = 1_000_000;

  describe('when the realm has not been restarted yet', () => {
    it('should restart after the first backoff', () => {
      expect(planRestart([], now)).toEqual({ delayMs: 1_000 });
    });
  });

  describe('when it was restarted twice within the window', () => {
    it('should wait longer before each restart', () => {
      expect(planRestart([now - 5_000], now)).toEqual({ delayMs: 3_000 });
      expect(planRestart([now - 5_000, now - 2_000], now)).toEqual({ delayMs: 9_000 });
    });
  });

  describe(`when it was restarted ${MAX_RESTARTS} times within the window`, () => {
    it('should give up', () => {
      expect(planRestart([now - 30_000, now - 20_000, now - 10_000], now)).toBeNull();
    });
  });

  describe('when the earlier restarts are older than the window', () => {
    it('should start the count over', () => {
      const old = now - RESTART_WINDOW_MS;
      expect(planRestart([old - 2, old - 1, old], now)).toEqual({ delayMs: 1_000 });
    });
  });
});

describe('bevy-realm restarts', () => {
  const path = '/scenes/restart';
  let fake: FakeChild;

  beforeEach(async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'Date'] });
    fake = createFakeChild();
    mocks.child = fake.child;
  });

  afterEach(async () => {
    await killAllRealms();
    vi.useRealTimers();
    mocks.send.mockClear();
    vi.mocked(run).mockClear();
    mocks.spawnsMultiplayerServer.mockResolvedValue(false);
    mocks.previewRunsServer.mockReturnValue(false);
  });

  describe('when a running realm exits unexpectedly', () => {
    let next: FakeChild;

    beforeEach(async () => {
      await start(path);
      next = createFakeChild();
      mocks.child = next.child;
      vi.mocked(getAvailablePort).mockResolvedValueOnce(8005);
      fake.exit();
      await flush();
    });

    it('should not restart it before the backoff', () => {
      expect(run).toHaveBeenCalledTimes(1);
    });

    describe('and the backoff passes', () => {
      beforeEach(async () => {
        await vi.advanceTimersByTimeAsync(1_000);
      });

      it('should start a new realm and hand the editor its URLs', () => {
        expect(run).toHaveBeenCalledTimes(2);
        expect(supervisionEvents().slice(1)).toEqual([
          { kind: 'restarting', path },
          {
            kind: 'restarted',
            path,
            url: 'http://localhost:8005',
            wsUrl: 'ws://localhost:8005/data-layer',
          },
        ]);
      });

      it('should clear the down state', () => {
        expect(isRealmDown(path)).toBe(false);
      });
    });

    describe('and the editor leaves the scene during the backoff', () => {
      beforeEach(async () => {
        await kill(path);
        await vi.advanceTimersByTimeAsync(10_000);
      });

      it('should not restart it', () => {
        expect(run).toHaveBeenCalledTimes(1);
      });
    });

    describe('and the app quits during the backoff', () => {
      beforeEach(async () => {
        await killAllRealms();
        await vi.advanceTimersByTimeAsync(10_000);
      });

      it('should not restart it', () => {
        expect(run).toHaveBeenCalledTimes(1);
      });
    });
  });

  describe(`when the realm keeps exiting ${MAX_RESTARTS + 1} times in a row`, () => {
    beforeEach(async () => {
      await start(path);
      let current = fake;
      for (let i = 0; i < MAX_RESTARTS; i++) {
        const next = createFakeChild();
        mocks.child = next.child;
        current.exit();
        await vi.advanceTimersByTimeAsync(10_000);
        current = next;
      }
      mocks.child = createFakeChild().child;
      current.exit();
      await vi.advanceTimersByTimeAsync(60_000);
    });

    it(`should restart it ${MAX_RESTARTS} times with a growing backoff, then give up`, () => {
      expect(stoppedEvents().map(event => event.restartInMs)).toEqual([1_000, 3_000, 9_000, null]);
    });

    it('should spawn no realm after giving up', () => {
      expect(run).toHaveBeenCalledTimes(MAX_RESTARTS + 1);
    });

    describe('and the editor asks for a realm again', () => {
      beforeEach(async () => {
        mocks.child = createFakeChild().child;
        await ensure(path);
      });

      it('should start one and count the restarts from zero', async () => {
        expect(run).toHaveBeenCalledTimes(MAX_RESTARTS + 2);
      });
    });
  });

  describe('when the scene SDK spawns a Multiplayer Server', () => {
    beforeEach(() => {
      mocks.spawnsMultiplayerServer.mockResolvedValue(true);
    });

    describe('and the realm starts', () => {
      beforeEach(async () => {
        await start(path);
      });

      it('should let the realm spawn it', () => {
        expect(spawnedArgs()).not.toContain('--no-server');
      });

      it('should report that the realm provides the server', () => {
        expect(realmProvidesServer(path)).toBe(true);
      });
    });

    describe('and a preview of the scene already runs its own server', () => {
      beforeEach(async () => {
        mocks.previewRunsServer.mockReturnValue(true);
        await start(path);
      });

      it('should leave the server to the preview', () => {
        expect(spawnedArgs()).toContain('--no-server');
      });

      it('should report that the realm provides no server', () => {
        expect(realmProvidesServer(path)).toBe(false);
      });

      describe('and the realm prints a server exit line', () => {
        beforeEach(async () => {
          fake.print(SERVER_EXITED);
          await vi.advanceTimersByTimeAsync(10_000);
        });

        it('should not restart the realm', () => {
          expect(supervisionEvents()).toEqual([]);
        });
      });
    });

    describe('and only the server stops while the realm keeps serving', () => {
      let next: FakeChild;

      beforeEach(async () => {
        await start(path);
        next = createFakeChild();
        mocks.child = next.child;
        vi.mocked(getAvailablePort).mockResolvedValueOnce(8005);
        // a pipe chunk can end mid-line
        fake.print(SERVER_EXITED.slice(0, 30));
        fake.print(SERVER_EXITED.slice(30));
      });

      it('should tell the editor the server stopped and will be restarted', () => {
        expect(supervisionEvents()).toEqual([
          {
            kind: 'stopped',
            path,
            cause: 'server',
            message: expect.stringContaining('exited with code 1'),
            restartInMs: 1_000,
          },
        ]);
      });

      it('should keep the realm up for the editor', () => {
        expect(fake.child.kill).not.toHaveBeenCalled();
        expect(isRealmDown(path)).toBe(false);
      });

      it('should still count the realm as the server owner while the restart is pending', () => {
        expect(realmProvidesServer(path)).toBe(true);
      });

      describe('and the backoff passes', () => {
        beforeEach(async () => {
          await vi.advanceTimersByTimeAsync(1_000);
        });

        it('should replace the realm with one that spawns a new server', () => {
          expect(fake.child.kill).toHaveBeenCalled();
          expect(run).toHaveBeenCalledTimes(2);
          expect(supervisionEvents().at(-1)).toEqual({
            kind: 'restarted',
            path,
            url: 'http://localhost:8005',
            wsUrl: 'ws://localhost:8005/data-layer',
          });
        });
      });

      describe('and the realm exits right after it', () => {
        beforeEach(async () => {
          fake.exit();
          await flush();
        });

        it('should not report a second stop', () => {
          expect(stoppedEvents()).toHaveLength(1);
        });
      });

      describe('and the editor reloads before the backoff passes', () => {
        beforeEach(async () => {
          await ensure(path);
          await vi.advanceTimersByTimeAsync(10_000);
        });

        it('should replace the realm once', () => {
          expect(run).toHaveBeenCalledTimes(2);
        });
      });
    });

    describe('and the server cannot run on this machine', () => {
      beforeEach(async () => {
        await start(path);
        fake.print(
          '⚠️ WARNING! Multiplayer Server exited with code 78. The preview keeps running\n',
        );
        await vi.advanceTimersByTimeAsync(10_000);
      });

      it('should give up at once', () => {
        expect(stoppedEvents()).toEqual([
          expect.objectContaining({ cause: 'server', restartInMs: null }),
        ]);
        expect(run).toHaveBeenCalledTimes(1);
      });

      it('should report that the realm provides no server', () => {
        expect(realmProvidesServer(path)).toBe(false);
      });
    });

    describe('and the server itself logs a line that mentions an exit', () => {
      beforeEach(async () => {
        await start(path);
        fake.print('[Server] 15:42:18 Multiplayer Server exited with code 1 in a test scene\n');
        await flush();
      });

      it('should not treat it as a stop', () => {
        expect(supervisionEvents()).toEqual([]);
      });
    });

    describe('and the realm is stopped deliberately', () => {
      beforeEach(async () => {
        await start(path);
        await kill(path);
      });

      it('should report that the realm provides no server', () => {
        expect(realmProvidesServer(path)).toBe(false);
      });
    });
  });

  describe('when the editor asks for a realm that is healthy', () => {
    beforeEach(async () => {
      await start(path);
      await ensure(path);
    });

    it('should reuse it without restart events', () => {
      expect(run).toHaveBeenCalledTimes(1);
      expect(supervisionEvents()).toEqual([]);
    });
  });
});
