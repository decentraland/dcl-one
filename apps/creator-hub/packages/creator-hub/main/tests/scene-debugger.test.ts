import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { WebContents } from 'electron';

import type { Child } from '../src/modules/bin';

const mocks = vi.hoisted(() => ({
  getPreview: vi.fn(),
  getRealm: vi.fn(),
}));

vi.mock('electron', () => ({ app: { getAppPath: vi.fn(() => '/fake/app') } }));
vi.mock('electron-log', () => ({ default: { info: vi.fn(), warn: vi.fn(), error: vi.fn() } }));
vi.mock('http-server', () => ({ createServer: vi.fn() }));
vi.mock('../src/modules/port', () => ({ getAvailablePort: vi.fn(async () => 4000) }));
vi.mock('../src/modules/cache', () => ({ getPreview: mocks.getPreview }));
vi.mock('../src/modules/bevy-realm', () => ({ getRealm: mocks.getRealm }));
vi.mock('../src/modules/cli', () => ({
  usesRealmServer: (preview: { child: Child }) => preview.child.args.includes('--no-server'),
}));

import { attachSceneDebugger, detachSceneDebugger } from '../src/modules/inspector';

// Fake bin.Child that delivers each printed chunk to every enabled listener.
function createFakeChild(args: string[] = []) {
  const listeners: (((data?: string) => void) | undefined)[] = [];
  const printed: string[] = [];
  const child = {
    args,
    process: { once: vi.fn(), off: vi.fn() },
    on: (_pattern: RegExp, handler: (data?: string) => void) => listeners.push(handler) - 1,
    off: (index: number) => {
      listeners[index] = undefined;
    },
    alive: () => true,
    stdall: () => [...printed],
  } as unknown as Child;
  return {
    child,
    print: (chunk: string) => {
      printed.push(chunk);
      listeners.forEach(listener => listener?.(chunk));
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
    vi.resetAllMocks();
    // a unique path per test isolates the module-level debugger map between tests
    path = `/scenes/debugger-${++sceneCount}`;
  });

  describe('when the preview reuses the editor realm Multiplayer Server', () => {
    let preview: ReturnType<typeof createFakeChild>;
    let realm: ReturnType<typeof createFakeChild>;

    beforeEach(() => {
      preview = createFakeChild(['start', '--explorer-alpha', '--no-server']);
      realm = createFakeChild(['start', '--data-layer']);
      mocks.getPreview.mockReturnValue({ child: preview.child });
      mocks.getRealm.mockReturnValue({ child: realm.child });
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

    it('should keep forwarding the preview own output', async () => {
      const { sender, sent } = createSender();
      await attachSceneDebugger(sender, path);

      preview.print('Preview server is now running!\n');

      expect(sent()).toEqual(['Preview server is now running!\n']);
    });

    it('should put the server logs so far ahead of the preview backlog', async () => {
      realm.print('Bundle saved bin/index.js\n[Server] 15:41:00 world ready\n');
      preview.print('Preview server is now running!\n');
      const { sender } = createSender();

      const { backlog } = await attachSceneDebugger(sender, path);

      expect(backlog).toEqual([
        '[Server] 15:41:00 world ready\n',
        'Preview server is now running!\n',
      ]);
    });

    it('should stop forwarding once the last window detaches', async () => {
      const { sender, sent } = createSender();
      await attachSceneDebugger(sender, path);

      detachSceneDebugger(sender, path);
      realm.print('[Server] 15:42:18 player joined\n');

      expect(sent()).toEqual([]);
    });
  });

  describe('when the preview runs its own Multiplayer Server', () => {
    it('should not forward anything from the editor realm', async () => {
      const preview = createFakeChild(['start', '--explorer-alpha']);
      const realm = createFakeChild(['start', '--data-layer']);
      mocks.getPreview.mockReturnValue({ child: preview.child });
      mocks.getRealm.mockReturnValue({ child: realm.child });
      const { sender, sent } = createSender();
      await attachSceneDebugger(sender, path);

      realm.print('[Server] 15:42:18 some other server\n');

      expect(sent()).toEqual([]);
    });
  });
});
