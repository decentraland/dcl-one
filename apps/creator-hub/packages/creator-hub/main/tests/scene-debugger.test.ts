import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { WebContents } from 'electron';

import type { Child, StreamType } from '../src/modules/bin';

const mocks = vi.hoisted(() => ({
  getPreview: vi.fn(),
}));

vi.mock('electron', () => ({ app: { getAppPath: vi.fn(() => '/fake/app') } }));
vi.mock('electron-log', () => ({ default: { info: vi.fn(), warn: vi.fn(), error: vi.fn() } }));
vi.mock('http-server', () => ({ createServer: vi.fn() }));
vi.mock('../src/modules/port', () => ({ getAvailablePort: vi.fn(async () => 4000) }));
vi.mock('../src/modules/cache', () => ({ getPreview: mocks.getPreview }));

import { attachSceneDebugger, detachSceneDebugger } from '../src/modules/inspector';
import {
  REALM_SERVER_STOPPED,
  captureRealmServerLogs,
  realmServerStopped,
} from '../src/modules/realm-server-logs';

type Listener = { handler: (data?: string) => void; type: StreamType };

// Fake bin.Child that delivers each printed chunk to the listeners of its stream.
function createFakeChild(args: string[] = []) {
  const listeners: (Listener | undefined)[] = [];
  const printed: string[] = [];
  const child = {
    args,
    process: { once: vi.fn(), off: vi.fn() },
    on: (_pattern: RegExp, handler: (data?: string) => void, opts?: { type?: StreamType }) =>
      listeners.push({ handler, type: opts?.type ?? 'all' }) - 1,
    off: (index: number) => {
      listeners[index] = undefined;
    },
    alive: () => true,
    stdall: () => [...printed],
  } as unknown as Child;
  return {
    child,
    print: (chunk: string, stream: 'stdout' | 'stderr' = 'stdout') => {
      printed.push(chunk);
      for (const listener of listeners) {
        if (listener && (listener.type === 'all' || listener.type === stream)) {
          listener.handler(chunk);
        }
      }
    },
  };
}

function createSender() {
  const send = vi.fn();
  const sender = { send, once: vi.fn(), isDestroyed: () => false } as unknown as WebContents;
  return { sender, sent: () => send.mock.calls.map(call => call[1] as string) };
}

let sceneCount = 0;

describe('attachSceneDebugger', () => {
  let path: string;

  beforeEach(() => {
    vi.clearAllMocks();
    // a unique path per test isolates the module-level debugger and log maps between tests
    path = `/scenes/debugger-${++sceneCount}`;
  });

  describe('when the preview reuses the editor realm Multiplayer Server', () => {
    let preview: ReturnType<typeof createFakeChild>;
    let realm: ReturnType<typeof createFakeChild>;

    beforeEach(() => {
      preview = createFakeChild(['start', '--explorer-alpha', '--no-server']);
      realm = createFakeChild(['start', '--data-layer']);
      mocks.getPreview.mockReturnValue({ child: preview.child });
      captureRealmServerLogs(path, realm.child);
    });

    it('should forward the realm server lines and leave out its build output', async () => {
      const { sender, sent } = createSender();
      await attachSceneDebugger(sender, path);

      realm.print('File src/index.ts changed, rebuilding...\n[Server] 15:42:18 player joined\n');

      expect(sent()).toEqual(['[Server] 15:42:18 player joined\n']);
    });

    it('should forward a server line that arrives split across two chunks as one line', async () => {
      const { sender, sent } = createSender();
      await attachSceneDebugger(sender, path);

      realm.print('[Server] 15:42:18 player');
      realm.print(' joined\n');

      expect(sent()).toEqual(['[Server] 15:42:18 player joined\n']);
    });

    it('should keep a stdout server line whole when a stderr chunk arrives inside it', async () => {
      const { sender, sent } = createSender();
      await attachSceneDebugger(sender, path);

      realm.print('[Server] 15:42:18 player');
      realm.print('Some warning\n', 'stderr');
      realm.print(' joined\n');

      expect(sent()).toEqual(['[Server] 15:42:18 player joined\n']);
    });

    it('should keep forwarding the preview own output', async () => {
      const { sender, sent } = createSender();
      await attachSceneDebugger(sender, path);

      preview.print('Preview server is now running!\n');

      expect(sent()).toEqual(['Preview server is now running!\n']);
    });

    it('should put the complete server lines so far ahead of the preview backlog', async () => {
      realm.print('Bundle saved bin/index.js\n[Server] 15:41:00 world ready\n[Server] 15:41');
      preview.print('Preview server is now running!\n');
      const { sender } = createSender();

      const { backlog } = await attachSceneDebugger(sender, path);

      expect(backlog).toEqual([
        '[Server] 15:41:00 world ready\n',
        'Preview server is now running!\n',
      ]);
    });

    it('should forward the rest of a server line that was partial at attach time', async () => {
      realm.print('[Server] 15:42:18 player');
      const { sender, sent } = createSender();
      await attachSceneDebugger(sender, path);

      realm.print(' joined\n');

      expect(sent()).toEqual(['[Server] 15:42:18 player joined\n']);
    });

    it('should forward the server lines of a realm that restarted while attached', async () => {
      const { sender, sent } = createSender();
      await attachSceneDebugger(sender, path);

      const restarted = createFakeChild(['start', '--data-layer']);
      captureRealmServerLogs(path, restarted.child);
      restarted.print('[Server] 15:43:00 world ready\n');

      expect(sent()).toEqual(['[Server] 15:43:00 world ready\n']);
    });

    it('should tell the console when the realm server stops', async () => {
      const { sender, sent } = createSender();
      await attachSceneDebugger(sender, path);

      realmServerStopped(path);

      expect(sent()).toEqual([`${REALM_SERVER_STOPPED}\n`]);
    });

    it('should stop forwarding once the last window detaches', async () => {
      const { sender, sent } = createSender();
      await attachSceneDebugger(sender, path);

      detachSceneDebugger(sender, path);
      realm.print('[Server] 15:42:18 player joined\n');

      expect(sent()).toEqual([]);
    });
  });

  describe('when the console opens before the editor realm is up', () => {
    it('should forward the server lines once the realm starts', async () => {
      const preview = createFakeChild(['start', '--explorer-alpha', '--no-server']);
      mocks.getPreview.mockReturnValue({ child: preview.child });
      const { sender, sent } = createSender();
      await attachSceneDebugger(sender, path);

      const realm = createFakeChild(['start', '--data-layer']);
      captureRealmServerLogs(path, realm.child);
      realm.print('[Server] 15:42:18 player joined\n');

      expect(sent()).toEqual(['[Server] 15:42:18 player joined\n']);
    });
  });

  describe('when the preview runs its own Multiplayer Server', () => {
    it('should not forward anything from the editor realm', async () => {
      const preview = createFakeChild(['start', '--explorer-alpha']);
      const realm = createFakeChild(['start', '--data-layer']);
      mocks.getPreview.mockReturnValue({ child: preview.child });
      captureRealmServerLogs(path, realm.child);
      const { sender, sent } = createSender();
      await attachSceneDebugger(sender, path);

      realm.print('[Server] 15:42:18 some other server\n');
      realmServerStopped(path);

      expect(sent()).toEqual([]);
    });
  });
});
