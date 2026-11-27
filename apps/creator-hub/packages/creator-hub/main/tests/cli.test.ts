import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { PREVIEW_CLIENT, type PreviewOptions } from '/shared/types/settings';
import type { Child } from '../src/modules/bin';

const mocks = vi.hoisted(() => ({
  run: vi.fn(),
  dclDeepLink: vi.fn(),
  install: vi.fn(),
  readFile: vi.fn(),
  stat: vi.fn(),
  openExternal: vi.fn(),
  startMobileDebugServer: vi.fn(),
  ensureWorkerPackage: vi.fn(async () => undefined),
  realmProvidesServer: vi.fn(),
  ensureEditorFilesNotDeployed: vi.fn(),
  addDependencyOverrides: vi.fn(),
}));

vi.mock('electron', () => ({
  app: { getPath: vi.fn(() => '/fake/exe') },
  shell: { openExternal: mocks.openExternal },
}));
vi.mock('electron-log/main', () => ({
  default: { info: vi.fn(), warn: vi.fn(), error: vi.fn() },
}));
vi.mock('fs/promises', () => ({ default: { readFile: mocks.readFile, stat: mocks.stat } }));
vi.mock('../src/modules/bin', () => ({ run: mocks.run, dclDeepLink: mocks.dclDeepLink }));
vi.mock('../src/modules/port', () => ({ getAvailablePort: vi.fn(async () => 4000) }));
vi.mock('../src/modules/analytics', () => ({
  getProjectId: vi.fn(async () => 'project-id'),
  track: vi.fn(),
}));
vi.mock('../src/modules/npm', () => ({ install: mocks.install }));
vi.mock('../src/modules/download-github-folder', () => ({ downloadGithubRepo: vi.fn() }));
vi.mock('../src/modules/mobile-debug-server', () => ({
  startMobileDebugServer: mocks.startMobileDebugServer,
}));
vi.mock('../src/modules/bevy-realm', () => ({
  realmProvidesServer: mocks.realmProvidesServer,
}));
vi.mock('../src/modules/network', () => ({ getLanIp: vi.fn(() => '192.168.0.10') }));
vi.mock('../src/modules/downloaded-tools', () => ({
  ensureWorkerPackage: mocks.ensureWorkerPackage,
}));
vi.mock('../src/modules/project-config-files', () => ({
  ensureEditorFilesNotDeployed: mocks.ensureEditorFilesNotDeployed,
  addDependencyOverrides: mocks.addDependencyOverrides,
}));

import {
  getMobilePreview,
  getPreview,
  killAllPreviews,
  start,
  stopPreview,
  getDeployCommandSupport,
  type DeployCommandSupport,
  deploy,
  init,
} from '../src/modules/cli';
import { previewRunsServer } from '../src/modules/realm-server-logs';
import {
  formatDeployRunnerFailure,
  formatDeployRunnerReady,
} from '../src/modules/deploy-runner-protocol';

const BASE_OPTS: PreviewOptions = {
  debugger: false,
  skipAuthScreen: true,
  enableLandscapeTerrains: false,
  openNewInstance: false,
  multiInstance: false,
  showWarnings: false,
  loadRawGltfs: false,
  mcp: false,
  client: PREVIEW_CLIENT.DESKTOP,
};

// Fake bin.Child whose output, failure and death the test controls. Output is delivered
// per chunk like the real stdout stream: a waitFor() settles only on a chunk its pattern
// matches, so a test can reproduce sdk-commands printing two lines in separate chunks.
// Chunks printed before start() registers its waitFor (it awaits a scene.json read first)
// are replayed to it, as tests print the ready line right after calling start().
function createFakeChild() {
  let alive = true;
  let settleWait!: () => void;
  const waitPromise = new Promise<Buffer>(resolve => {
    settleWait = () => resolve(Buffer.from(''));
  });
  // start() awaits waitFor/wait through a race; an unconsumed rejection is fine
  waitPromise.catch(() => {});
  const printed: string[] = [];
  const pendingWaits: { pattern: RegExp; resolve: (chunk: string) => void }[] = [];
  const matches = (pattern: RegExp, chunk: string) => {
    pattern.lastIndex = 0;
    return pattern.test(chunk);
  };

  const child = {
    pkg: '@dcl/sdk-commands',
    bin: 'sdk-commands',
    args: [],
    cwd: '',
    process: {} as never,
    on: vi.fn(() => 1),
    once: vi.fn(() => 1),
    off: vi.fn(),
    wait: () => waitPromise,
    waitFor: vi.fn(
      (pattern: RegExp) =>
        new Promise<string>(resolve => {
          const earlier = printed.find(chunk => matches(pattern, chunk));
          if (earlier !== undefined) resolve(earlier);
          else pendingWaits.push({ pattern, resolve });
        }),
    ),
    kill: vi.fn(async () => {
      alive = false;
      settleWait();
    }),
    alive: () => alive,
    stdall: () => [...printed],
  } as unknown as Child;

  const print = (chunk: string) => {
    printed.push(chunk);
    for (const wait of pendingWaits.splice(0)) {
      if (matches(wait.pattern, chunk)) wait.resolve(chunk);
      else pendingWaits.push(wait);
    }
  };

  return {
    child,
    // one stdout chunk, as the real process would deliver it
    print,
    // the line sdk-commands prints once the preview is ready (the sdk also self-opens then)
    printDeeplink: (params: string) => print(`Desktop client: decentraland://${params}`),
    // process death before any deeplink (crash, or an external kill)
    die: () => {
      alive = false;
      settleWait();
    },
  };
}

const BEVY_WEB_URL =
  'https://decentraland.org/bevy-web/?preview=true&realm=http://127.0.0.1:8000&position=0,0';

// Drives a Bevy web start to its ready state the way sdk-commands prints it: the
// server-ready line and the browser URL arrive in separate stdout chunks.
async function startBevyWeb(path: string, fake: ReturnType<typeof createFakeChild>) {
  const promise = start(path, { ...BASE_OPTS, client: PREVIEW_CLIENT.BEVY_WEB });
  fake.print('Preview server is now running!\n');
  await flush();
  fake.print(`Available on:\n    ${BEVY_WEB_URL}\n`);
  await promise;
}

function setSceneJsonMissing() {
  // missing scene.json keeps the default landscapeTerrain behavior
  mocks.readFile.mockRejectedValue(new Error('ENOENT'));
}

function spawnedArgs(call = 0): string[] {
  return mocks.run.mock.calls[call][2].args;
}

const flush = () => new Promise(resolve => setTimeout(resolve, 0));

let sceneCount = 0;

describe('cli preview start', () => {
  let path: string;

  beforeEach(() => {
    vi.clearAllMocks();
    // a unique path per test isolates the module-level preview cache between tests
    path = `/scenes/scene-${++sceneCount}`;
    setSceneJsonMissing();
    mocks.install.mockResolvedValue(undefined);
  });

  describe('when starting a fresh preview', () => {
    it('should not fire the deeplink itself (sdk-commands self-opens the client)', async () => {
      const fake = createFakeChild();
      mocks.run.mockReturnValue(fake.child);

      const promise = start(path, BASE_OPTS);
      fake.printDeeplink('realm=http://127.0.0.1:8000&position=0,0');
      await promise;

      expect(mocks.run).toHaveBeenCalledTimes(1);
      expect(mocks.dclDeepLink).not.toHaveBeenCalled();
    });

    it('should cache the captured deeplink for later re-focus', async () => {
      const fake = createFakeChild();
      mocks.run.mockReturnValue(fake.child);

      const promise = start(path, BASE_OPTS);
      fake.printDeeplink('realm=http://127.0.0.1:8000&position=0,0');
      await promise;

      expect(getPreview(path)?.url).toContain('realm=http');
    });

    it('should pass --mcp and the chosen --mcp-port (the Explorer gateway launch path)', async () => {
      const fake = createFakeChild();
      mocks.run.mockReturnValue(fake.child);

      const promise = start(path, { ...BASE_OPTS, mcp: true, mcpPort: 4321 });
      fake.printDeeplink('realm=http://127.0.0.1:8000&mcp=true');
      await promise;

      const args = spawnedArgs();
      expect(args).toContain('--mcp');
      expect(args.slice(args.indexOf('--mcp-port'), args.indexOf('--mcp-port') + 2)).toEqual([
        '--mcp-port',
        '4321',
      ]);
    });

    it('should not pass --mcp-port when no port is chosen', async () => {
      const fake = createFakeChild();
      mocks.run.mockReturnValue(fake.child);

      const promise = start(path, { ...BASE_OPTS, mcp: true });
      fake.printDeeplink('realm=http://127.0.0.1:8000&mcp=true');
      await promise;

      const args = spawnedArgs();
      expect(args).toContain('--mcp');
      expect(args).not.toContain('--mcp-port');
    });

    describe('and raw GLTFs are requested', () => {
      it('should end the args with the explorer passthrough for lsd-raw-gltf', async () => {
        const fake = createFakeChild();
        mocks.run.mockReturnValue(fake.child);

        const promise = start(path, { ...BASE_OPTS, loadRawGltfs: true, mcp: true, mcpPort: 4321 });
        fake.printDeeplink('realm=http://127.0.0.1:8000&lsd-raw-gltf=true');
        await promise;

        expect(spawnedArgs().slice(-2)).toEqual(['--', '--lsd-raw-gltf']);
      });
    });

    describe('and raw GLTFs are not requested', () => {
      it('should not pass the explorer passthrough at all', async () => {
        const fake = createFakeChild();
        mocks.run.mockReturnValue(fake.child);

        const promise = start(path, BASE_OPTS);
        fake.printDeeplink('realm=http://127.0.0.1:8000&position=0,0');
        await promise;

        expect(spawnedArgs()).not.toContain('--');
        expect(spawnedArgs()).not.toContain('--lsd-raw-gltf');
      });
    });
  });

  describe('when the Bevy editor realm is already running for the scene', () => {
    // what the scene's sdk-commands start command declares, read from its dist
    function setStartCommand(source: string) {
      mocks.readFile.mockImplementation(async (file: string) => {
        if (file.includes('commands/start/index.js')) return source;
        throw new Error('ENOENT');
      });
    }

    beforeEach(() => {
      mocks.realmProvidesServer.mockReturnValue(true);
    });

    afterEach(() => {
      mocks.realmProvidesServer.mockReset();
    });

    describe('and the scene sdk-commands declares --no-server', () => {
      beforeEach(() => {
        setStartCommand("'--no-client': Boolean,\n'--no-server': Boolean,");
      });

      it('should pass --no-server so the preview reuses the realm Multiplayer Server', async () => {
        const fake = createFakeChild();
        mocks.run.mockReturnValue(fake.child);

        const promise = start(path, BASE_OPTS);
        fake.printDeeplink('realm=http://127.0.0.1:8000&position=0,0');
        await promise;

        expect(spawnedArgs()).toContain('--no-server');
      });

      it('should keep the explorer passthrough last', async () => {
        const fake = createFakeChild();
        mocks.run.mockReturnValue(fake.child);

        const promise = start(path, { ...BASE_OPTS, loadRawGltfs: true });
        fake.printDeeplink('realm=http://127.0.0.1:8000&lsd-raw-gltf=true');
        await promise;

        expect(spawnedArgs()).toContain('--no-server');
        expect(spawnedArgs().slice(-2)).toEqual(['--', '--lsd-raw-gltf']);
      });

      it('should pass it to a Bevy web preview too', async () => {
        const fake = createFakeChild();
        mocks.run.mockReturnValue(fake.child);

        await startBevyWeb(path, fake);

        expect(spawnedArgs()).toEqual(['start', '--bevy-web', '--no-server']);
      });

      it('should pass it to a mobile-QR preview too', async () => {
        const fake = createFakeChild();
        mocks.run.mockReturnValue(fake.child);

        const promise = start(path, { ...BASE_OPTS, mobile: true });
        fake.printDeeplink('open?preview=http://192.168.0.10:8000&position=0,0');
        await promise;

        expect(spawnedArgs()).toEqual(['start', '--mobile', '--no-server']);
      });
    });

    describe('and the scene sdk-commands only mentions --no-server outside its arg spec', () => {
      it('should not pass the flag (arg would reject it)', async () => {
        setStartCommand("'--no-client': Boolean,\n// TODO: add --no-server\n");
        const fake = createFakeChild();
        mocks.run.mockReturnValue(fake.child);

        const promise = start(path, BASE_OPTS);
        fake.printDeeplink('realm=http://127.0.0.1:8000&position=0,0');
        await promise;

        expect(spawnedArgs()).not.toContain('--no-server');
      });
    });

    describe('and the scene sdk-commands does not declare --no-server', () => {
      it('should not pass the flag (arg rejects unknown flags)', async () => {
        setStartCommand("'--no-client': Boolean,");
        const fake = createFakeChild();
        mocks.run.mockReturnValue(fake.child);

        const promise = start(path, BASE_OPTS);
        fake.printDeeplink('realm=http://127.0.0.1:8000&position=0,0');
        await promise;

        expect(spawnedArgs()).not.toContain('--no-server');
      });
    });
  });

  describe('when no Bevy editor realm is running for the scene', () => {
    it('should let the preview spawn its own Multiplayer Server', async () => {
      mocks.readFile.mockResolvedValue("'--no-server': Boolean,");
      const fake = createFakeChild();
      mocks.run.mockReturnValue(fake.child);

      const promise = start(path, BASE_OPTS);
      fake.printDeeplink('realm=http://127.0.0.1:8000&position=0,0');
      await promise;

      expect(spawnedArgs()).not.toContain('--no-server');
    });
  });

  describe('when a preview that reuses the realm Multiplayer Server is running', () => {
    let fake: ReturnType<typeof createFakeChild>;

    beforeEach(async () => {
      mocks.readFile.mockImplementation(async (file: string) => {
        if (file.includes('commands/start/index.js')) return "'--no-server': Boolean,";
        throw new Error('ENOENT');
      });
      mocks.realmProvidesServer.mockReturnValue(true);
      fake = createFakeChild();
      mocks.run.mockImplementation((_pkg, _bin, { args }) => {
        (fake.child as { args: string[] }).args = args;
        return fake.child;
      });
      const promise = start(path, BASE_OPTS);
      fake.printDeeplink('realm=http://127.0.0.1:8000&position=0,0');
      await promise;
      mocks.run.mockReset();
    });

    afterEach(() => {
      mocks.realmProvidesServer.mockReset();
    });

    describe('and the realm still provides it, or is restarting', () => {
      it('should re-focus the client instead of respawning', async () => {
        await start(path, BASE_OPTS);

        expect(mocks.run).not.toHaveBeenCalled();
        expect(mocks.dclDeepLink).toHaveBeenCalledTimes(1);
      });
    });

    describe('and the realm no longer provides it', () => {
      let restarted: ReturnType<typeof createFakeChild>;

      beforeEach(async () => {
        mocks.realmProvidesServer.mockReturnValue(false);
        restarted = createFakeChild();
        mocks.run.mockReturnValue(restarted.child);
        const promise = start(path, BASE_OPTS);
        restarted.printDeeplink('realm=http://127.0.0.1:8001&position=0,0');
        await promise;
      });

      it('should restart the preview with its own Multiplayer Server', () => {
        expect(fake.child.kill).toHaveBeenCalled();
        expect(mocks.run).toHaveBeenCalledTimes(1);
        expect(spawnedArgs()).not.toContain('--no-server');
      });

      it('should record the preview as the server owner for the editor realm', () => {
        expect(previewRunsServer(path)).toBe(true);
      });

      describe('and the preview is stopped', () => {
        beforeEach(async () => {
          await stopPreview(path);
        });

        it('should no longer count it as the server owner', () => {
          expect(previewRunsServer(path)).toBe(false);
        });
      });
    });

    describe('and a mobile-QR start arrives after the realm lost it', () => {
      it('should still reuse the running preview', async () => {
        mocks.realmProvidesServer.mockReturnValue(false);

        await start(path, { ...BASE_OPTS, mobile: true });

        expect(mocks.run).not.toHaveBeenCalled();
      });
    });
  });

  describe('when starting a mobile-QR preview', () => {
    it('should spawn a bare `start --mobile`: it opens no client, so no client option applies', async () => {
      const fake = createFakeChild();
      mocks.run.mockReturnValue(fake.child);

      const promise = start(path, {
        ...BASE_OPTS,
        mobile: true,
        enableLandscapeTerrains: true,
        multiInstance: true,
        mcp: true,
        mcpPort: 4321,
        loadRawGltfs: true,
      });
      fake.printDeeplink('open?preview=http://192.168.0.10:8000&position=0,0');
      await promise;

      expect(spawnedArgs()).toEqual(['start', '--mobile']);
    });

    it('should not pass --mobile for a regular desktop preview', async () => {
      const fake = createFakeChild();
      mocks.run.mockReturnValue(fake.child);

      const promise = start(path, BASE_OPTS);
      fake.printDeeplink('realm=http://127.0.0.1:8000&position=0,0');
      await promise;

      expect(spawnedArgs()).not.toContain('--mobile');
    });

    describe('and the Bevy web client is the selected preview client', () => {
      it('should still take the --mobile path instead of opening the Bevy web tab', async () => {
        const fake = createFakeChild();
        mocks.run.mockReturnValue(fake.child);

        const promise = start(path, {
          ...BASE_OPTS,
          client: PREVIEW_CLIENT.BEVY_WEB,
          mobile: true,
        });
        fake.printDeeplink('open?preview=http://192.168.0.10:8000&position=0,0');
        await promise;

        expect(spawnedArgs()).toEqual(['start', '--mobile']);
        expect(mocks.openExternal).not.toHaveBeenCalled();
        expect(getPreview(path)?.url).toContain('open?preview=http://192.168.0.10:8000');
      });
    });

    describe('and a desktop preview is already running', () => {
      it('should reuse its server without restarting or re-firing the deeplink', async () => {
        const fake = createFakeChild();
        mocks.run.mockReturnValue(fake.child);
        const first = start(path, BASE_OPTS);
        fake.printDeeplink('realm=http://127.0.0.1:8000&position=0,0');
        await first;
        mocks.run.mockClear();

        await start(path, { ...BASE_OPTS, mobile: true });

        expect(fake.child.kill).not.toHaveBeenCalled();
        expect(mocks.run).not.toHaveBeenCalled();
        expect(mocks.dclDeepLink).not.toHaveBeenCalled();
        expect(getPreview(path)?.url).toContain('realm=http://127.0.0.1:8000');
      });
    });

    describe('and a Bevy web preview is already running', () => {
      it('should reuse its server without restarting or reopening the browser tab', async () => {
        const fake = createFakeChild();
        mocks.run.mockReturnValue(fake.child);
        await startBevyWeb(path, fake);
        mocks.run.mockClear();

        await start(path, { ...BASE_OPTS, client: PREVIEW_CLIENT.BEVY_WEB, mobile: true });

        expect(fake.child.kill).not.toHaveBeenCalled();
        expect(mocks.run).not.toHaveBeenCalled();
        expect(mocks.openExternal).not.toHaveBeenCalled();
      });
    });
  });

  describe('when starting a Bevy web preview', () => {
    it('should cache the browser URL sdk-commands prints after the server-ready line', async () => {
      const fake = createFakeChild();
      mocks.run.mockReturnValue(fake.child);

      await startBevyWeb(path, fake);

      expect(spawnedArgs()).toContain('--bevy-web');
      expect(getPreview(path)?.url).toBe(BEVY_WEB_URL);
    });

    it('should spawn a bare `start --bevy-web`: the preview options only parameterize the desktop client', async () => {
      const fake = createFakeChild();
      mocks.run.mockReturnValue(fake.child);

      const promise = start(path, {
        ...BASE_OPTS,
        client: PREVIEW_CLIENT.BEVY_WEB,
        enableLandscapeTerrains: true,
        multiInstance: true,
        mcp: true,
        mcpPort: 4321,
        loadRawGltfs: true,
      });
      fake.print(`Available on:\n    ${BEVY_WEB_URL}\n`);
      await promise;

      expect(spawnedArgs()).toEqual(['start', '--bevy-web']);
    });

    it('should not settle on the server-ready line alone', async () => {
      const fake = createFakeChild();
      mocks.run.mockReturnValue(fake.child);
      let settled = false;

      const promise = start(path, { ...BASE_OPTS, client: PREVIEW_CLIENT.BEVY_WEB }).then(() => {
        settled = true;
      });
      fake.print('Preview server is now running!\n');
      await flush();

      expect(settled).toBe(false);
      fake.print(`Available on:\n    ${BEVY_WEB_URL}\n`);
      await promise;
      expect(settled).toBe(true);
    });

    it('should reopen the cached browser URL on a repeat Preview press instead of respawning', async () => {
      const fake = createFakeChild();
      mocks.run.mockReturnValue(fake.child);
      await startBevyWeb(path, fake);
      mocks.run.mockClear();

      await start(path, { ...BASE_OPTS, client: PREVIEW_CLIENT.BEVY_WEB });

      expect(mocks.run).not.toHaveBeenCalled();
      expect(mocks.openExternal).toHaveBeenCalledWith(BEVY_WEB_URL);
    });

    it('should reinstall and retry when the spawn dies before printing the URL', async () => {
      const crashed = createFakeChild();
      const retried = createFakeChild();
      mocks.run
        .mockImplementationOnce(() => crashed.child)
        .mockImplementationOnce(() => retried.child);

      const promise = start(path, { ...BASE_OPTS, client: PREVIEW_CLIENT.BEVY_WEB });
      await flush();
      crashed.die();
      await vi.waitFor(() => expect(mocks.run).toHaveBeenCalledTimes(2));
      retried.print(`Available on:\n    ${BEVY_WEB_URL}\n`);
      await promise;

      expect(mocks.install).toHaveBeenCalledTimes(1);
      expect(getPreview(path)?.url).toBe(BEVY_WEB_URL);
    });
  });

  describe('when a mobile-QR preview is already running and a desktop Preview is requested', () => {
    it('should restart instead of reusing so the desktop client actually opens', async () => {
      const fake = createFakeChild();
      mocks.run.mockReturnValue(fake.child);
      const first = start(path, { ...BASE_OPTS, mobile: true });
      fake.printDeeplink('open?preview=http://192.168.0.10:8000&position=0,0');
      await first;
      mocks.run.mockClear();

      const restarted = createFakeChild();
      mocks.run.mockReturnValue(restarted.child);
      const second = start(path, BASE_OPTS);
      restarted.printDeeplink('realm=http://127.0.0.1:8000&position=0,0');
      await second;

      expect(fake.child.kill).toHaveBeenCalled();
      expect(mocks.run).toHaveBeenCalledTimes(1);
      expect(spawnedArgs()).not.toContain('--mobile');
    });
  });

  describe('when a preview is already running', () => {
    let fake: ReturnType<typeof createFakeChild>;

    beforeEach(async () => {
      fake = createFakeChild();
      mocks.run.mockReturnValue(fake.child);
      const promise = start(path, BASE_OPTS);
      fake.printDeeplink('realm=http://127.0.0.1:8000&position=0,0');
      await promise;
      mocks.run.mockClear();
    });

    it('should re-focus the client by firing the adjusted deeplink instead of respawning', async () => {
      await start(path, BASE_OPTS);

      expect(mocks.run).not.toHaveBeenCalled();
      expect(mocks.dclDeepLink).toHaveBeenCalledTimes(1);
    });

    it('should leave lsd-raw-gltf out of the re-focus deeplink', async () => {
      await start(path, BASE_OPTS);

      const fired = mocks.dclDeepLink.mock.calls[0][0];
      expect(new URLSearchParams(fired).has('lsd-raw-gltf')).toBe(false);
    });

    describe('and raw GLTFs are now requested', () => {
      it('should kill the preview and respawn it with the passthrough (the explorer reads it only at startup)', async () => {
        const restarted = createFakeChild();
        mocks.run.mockReturnValue(restarted.child);

        const promise = start(path, { ...BASE_OPTS, loadRawGltfs: true });
        restarted.printDeeplink('realm=http://127.0.0.1:8001&lsd-raw-gltf=true');
        await promise;

        expect(fake.child.kill).toHaveBeenCalled();
        expect(mocks.run).toHaveBeenCalledTimes(1);
        expect(mocks.dclDeepLink).not.toHaveBeenCalled();
        expect(spawnedArgs().slice(-2)).toEqual(['--', '--lsd-raw-gltf']);
      });
    });
  });

  describe('when a preview loading raw GLTFs is already running', () => {
    let fake: ReturnType<typeof createFakeChild>;

    beforeEach(async () => {
      fake = createFakeChild();
      mocks.run.mockReturnValue(fake.child);
      const promise = start(path, { ...BASE_OPTS, loadRawGltfs: true });
      fake.printDeeplink('realm=http://127.0.0.1:8000&lsd-raw-gltf=true');
      await promise;
      mocks.run.mockClear();
    });

    it('should re-focus keeping lsd-raw-gltf when the setting stays on', async () => {
      await start(path, { ...BASE_OPTS, loadRawGltfs: true });

      expect(mocks.run).not.toHaveBeenCalled();
      expect(mocks.dclDeepLink).toHaveBeenCalledTimes(1);
      const fired = mocks.dclDeepLink.mock.calls[0][0];
      expect(new URLSearchParams(fired).get('lsd-raw-gltf')).toBe('true');
    });

    it('should kill the preview and respawn it without the passthrough when the setting goes off', async () => {
      const restarted = createFakeChild();
      mocks.run.mockReturnValue(restarted.child);

      const promise = start(path, BASE_OPTS);
      restarted.printDeeplink('realm=http://127.0.0.1:8001&position=0,0');
      await promise;

      expect(fake.child.kill).toHaveBeenCalled();
      expect(mocks.run).toHaveBeenCalledTimes(1);
      expect(spawnedArgs()).not.toContain('--lsd-raw-gltf');
    });
  });

  describe('when a preview spawned with raw GLTFs by an sdk-commands without the passthrough is running', () => {
    let fake: ReturnType<typeof createFakeChild>;

    beforeEach(async () => {
      fake = createFakeChild();
      mocks.run.mockReturnValue(fake.child);
      const promise = start(path, { ...BASE_OPTS, loadRawGltfs: true });
      // the older sdk-commands ignored everything after `--`, so the param never reached the deeplink
      fake.printDeeplink('realm=http://127.0.0.1:8000&position=0,0');
      await promise;
      mocks.run.mockClear();
    });

    it('should re-focus instead of restarting the preview on every press', async () => {
      await start(path, { ...BASE_OPTS, loadRawGltfs: true });

      expect(fake.child.kill).not.toHaveBeenCalled();
      expect(mocks.run).not.toHaveBeenCalled();
      expect(mocks.dclDeepLink).toHaveBeenCalledTimes(1);
    });
  });

  describe('when a second start arrives while a spawn is still booting', () => {
    it('should ride the in-flight spawn instead of racing a second one', async () => {
      const fake = createFakeChild();
      mocks.run.mockReturnValue(fake.child);

      const first = start(path, BASE_OPTS);
      await flush();
      const second = start(path, BASE_OPTS);
      await flush();

      fake.printDeeplink('realm=http://127.0.0.1:8000&position=0,0');
      await Promise.all([first, second]);

      expect(mocks.run).toHaveBeenCalledTimes(1);
      expect(mocks.dclDeepLink).not.toHaveBeenCalled();
    });

    it('should serialize two starts issued in the same tick (registration happens before any await)', async () => {
      const fake = createFakeChild();
      mocks.run.mockReturnValue(fake.child);

      // no flush between them: both calls race the awaits inside start()
      const first = start(path, BASE_OPTS);
      const second = start(path, BASE_OPTS);

      fake.printDeeplink('realm=http://127.0.0.1:8000&position=0,0');
      await Promise.all([first, second]);

      expect(mocks.run).toHaveBeenCalledTimes(1);
      expect(mocks.dclDeepLink).not.toHaveBeenCalled();
    });
  });

  describe('when a second start arrives during the reinstall-retry', () => {
    it('should ride the whole operation instead of racing the retry spawn', async () => {
      const crashed = createFakeChild();
      const retried = createFakeChild();
      mocks.run
        .mockImplementationOnce(() => crashed.child)
        .mockImplementationOnce(() => retried.child);
      let resolveInstall!: () => void;
      mocks.install.mockImplementation(
        () =>
          new Promise<void>(resolve => {
            resolveInstall = resolve;
          }),
      );

      const first = start(path, BASE_OPTS);
      await flush();
      crashed.die();
      await vi.waitFor(() => expect(mocks.install).toHaveBeenCalledTimes(1));

      // lands while the reinstall is still underway (no spawn alive at this moment)
      const second = start(path, BASE_OPTS);
      resolveInstall();
      await vi.waitFor(() => expect(mocks.run).toHaveBeenCalledTimes(2));
      retried.printDeeplink('realm=http://127.0.0.1:8000&position=0,0');
      await Promise.all([first, second]);

      // crashed spawn + its retry only: the second start never spawned a third process
      expect(mocks.run).toHaveBeenCalledTimes(2);
      expect(getPreview(path)?.url).toContain('realm=http');
    });
  });

  describe('when the spawn dies before producing a deeplink', () => {
    it('should reinstall and retry once', async () => {
      const crashed = createFakeChild();
      const retried = createFakeChild();
      mocks.run
        .mockImplementationOnce(() => crashed.child)
        .mockImplementationOnce(() => retried.child);

      const promise = start(path, BASE_OPTS);
      await flush();
      crashed.die();
      await vi.waitFor(() => expect(mocks.run).toHaveBeenCalledTimes(2));
      retried.printDeeplink('realm=http://127.0.0.1:8000&position=0,0');
      await promise;

      expect(mocks.install).toHaveBeenCalledTimes(1);
    });

    it('should reject when the retry dies too', async () => {
      const crashed = createFakeChild();
      const retried = createFakeChild();
      mocks.run
        .mockImplementationOnce(() => crashed.child)
        .mockImplementationOnce(() => retried.child);

      const promise = start(path, BASE_OPTS);
      await flush();
      crashed.die();
      await vi.waitFor(() => expect(mocks.run).toHaveBeenCalledTimes(2));
      retried.die();

      await expect(promise).rejects.toThrow('Preview process exited before producing a deeplink');
      expect(mocks.install).toHaveBeenCalledTimes(1);
    });
  });
});

describe('getMobilePreview', () => {
  let path: string;
  const fetchMock = vi.fn();
  const MOBILE_DEEPLINK = 'decentraland://open?preview=http://192.168.0.10:8000&position=0,0';

  beforeEach(() => {
    vi.clearAllMocks();
    path = `/scenes/scene-${++sceneCount}`;
    setSceneJsonMissing();
    vi.stubGlobal('fetch', fetchMock);
    fetchMock.mockResolvedValue({
      ok: true,
      json: async () => ({
        ok: true,
        data: { url: MOBILE_DEEPLINK, qr: 'data:image/png;base64,qr' },
      }),
    });
    mocks.startMobileDebugServer.mockResolvedValue(9123);
  });

  it('should answer null when nothing is running for the path', async () => {
    expect(await getMobilePreview(path)).toBeNull();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('should ask the running Bevy web preview server for the QR (the URL carries the realm)', async () => {
    const fake = createFakeChild();
    mocks.run.mockReturnValue(fake.child);
    await startBevyWeb(path, fake);

    const result = await getMobilePreview(path);

    expect(fetchMock).toHaveBeenCalledWith('http://127.0.0.1:8000/mobile-preview');
    expect(result?.url).toContain('preview=http://192.168.0.10:8000');
    expect(result?.url).toContain('scene-inspector=');
    expect(result?.qr).toMatch(/^data:image\/svg\+xml;base64,/);
  });

  it('should ask the running desktop preview server for the QR (the deeplink carries the realm)', async () => {
    const fake = createFakeChild();
    mocks.run.mockReturnValue(fake.child);
    const promise = start(path, BASE_OPTS);
    fake.printDeeplink('realm=http://127.0.0.1:8000&position=0,0');
    await promise;

    await getMobilePreview(path);

    expect(fetchMock).toHaveBeenCalledWith('http://127.0.0.1:8000/mobile-preview');
  });

  it('should ask the running mobile-QR preview server for the QR (the deeplink carries the LAN url)', async () => {
    const fake = createFakeChild();
    mocks.run.mockReturnValue(fake.child);
    const promise = start(path, { ...BASE_OPTS, mobile: true });
    fake.printDeeplink('open?preview=http://192.168.0.10:8000&position=0,0');
    await promise;

    await getMobilePreview(path);

    expect(fetchMock).toHaveBeenCalledWith('http://192.168.0.10:8000/mobile-preview');
  });

  it('should answer null when the server has no LAN address to hand the phone', async () => {
    const fake = createFakeChild();
    mocks.run.mockReturnValue(fake.child);
    await startBevyWeb(path, fake);
    fetchMock.mockResolvedValue({ ok: false, status: 404, json: async () => ({ ok: false }) });

    expect(await getMobilePreview(path)).toBeNull();
  });
});

describe('cli preview stop', () => {
  let path: string;

  beforeEach(() => {
    vi.clearAllMocks();
    path = `/scenes/scene-${++sceneCount}`;
    mocks.install.mockResolvedValue(undefined);
  });

  describe('when the user leaves the scene while its preview is still starting', () => {
    it('should kill the spawn and settle without reinstalling or spawning again', async () => {
      const fake = createFakeChild();
      mocks.run.mockReturnValue(fake.child);

      const promise = start(path, BASE_OPTS);
      await vi.waitFor(() => expect(mocks.run).toHaveBeenCalledTimes(1));

      await stopPreview(path);
      await expect(promise).resolves.toBe(path);

      expect(fake.child.kill).toHaveBeenCalled();
      expect(mocks.install).not.toHaveBeenCalled();
      expect(mocks.run).toHaveBeenCalledTimes(1);
      expect(getPreview(path)).toBeUndefined();
    });
  });

  describe('when the user leaves the scene while a raw-GLTF toggle is restarting its preview', () => {
    it('should kill the restart and settle without reinstalling or spawning again', async () => {
      const running = createFakeChild();
      mocks.run.mockReturnValue(running.child);
      const first = start(path, BASE_OPTS);
      running.printDeeplink('realm=http://127.0.0.1:8000&position=0,0');
      await first;

      const restarting = createFakeChild();
      mocks.run.mockReturnValue(restarting.child);
      const restart = start(path, { ...BASE_OPTS, loadRawGltfs: true });
      await vi.waitFor(() => expect(mocks.run).toHaveBeenCalledTimes(2));

      await stopPreview(path);
      await expect(restart).resolves.toBe(path);

      expect(running.child.kill).toHaveBeenCalled();
      expect(restarting.child.kill).toHaveBeenCalled();
      expect(mocks.install).not.toHaveBeenCalled();
      expect(mocks.run).toHaveBeenCalledTimes(2);
      expect(getPreview(path)).toBeUndefined();
    });
  });

  describe('when the user leaves the scene before the spawn happens', () => {
    it('should never spawn', async () => {
      const promise = start(path, BASE_OPTS);
      await stopPreview(path);

      await expect(promise).resolves.toBe(path);
      expect(mocks.run).not.toHaveBeenCalled();
    });
  });

  describe('when the user leaves the scene during the reinstall-retry', () => {
    let resolveInstall: () => void;

    beforeEach(() => {
      mocks.run.mockImplementation(() => createFakeChild().child);
      mocks.install.mockImplementation(
        () =>
          new Promise<void>(resolve => {
            resolveInstall = resolve;
          }),
      );
    });

    it('should not respawn once the reinstall finishes', async () => {
      const promise = start(path, BASE_OPTS);
      await vi.waitFor(() => expect(mocks.run).toHaveBeenCalledTimes(1));
      await getPreview(path)!.child.kill();
      await vi.waitFor(() => expect(mocks.install).toHaveBeenCalledTimes(1));

      await stopPreview(path);
      resolveInstall();

      await expect(promise).resolves.toBe(path);
      expect(mocks.run).toHaveBeenCalledTimes(1);
    });

    it('should spawn a fresh preview for a start issued after the stop', async () => {
      const first = start(path, BASE_OPTS);
      await vi.waitFor(() => expect(mocks.run).toHaveBeenCalledTimes(1));
      await getPreview(path)!.child.kill();
      await vi.waitFor(() => expect(mocks.install).toHaveBeenCalledTimes(1));

      await stopPreview(path);
      // the user came back to the scene and pressed Preview before the stopped start settled
      const fresh = createFakeChild();
      mocks.run.mockImplementation(() => fresh.child);
      const second = start(path, BASE_OPTS);
      resolveInstall();

      await expect(first).resolves.toBe(path);
      await vi.waitFor(() => expect(mocks.run).toHaveBeenCalledTimes(2));
      fresh.printDeeplink('realm=http://127.0.0.1:8000&position=0,0');
      await second;

      expect(getPreview(path)?.child).toBe(fresh.child);
    });
  });

  describe('when the user leaves the scene while a Bevy web preview is starting', () => {
    it('should settle instead of waiting for a URL that never comes', async () => {
      const fake = createFakeChild();
      mocks.run.mockReturnValue(fake.child);

      const promise = start(path, { ...BASE_OPTS, client: PREVIEW_CLIENT.BEVY_WEB });
      await vi.waitFor(() => expect(mocks.run).toHaveBeenCalledTimes(1));
      await stopPreview(path);

      await expect(promise).resolves.toBe(path);
      expect(mocks.run).toHaveBeenCalledTimes(1);
    });
  });

  describe('when the preview is stopped twice (Back, then the editor unmounting)', () => {
    it('should resolve both stops', async () => {
      const fake = createFakeChild();
      mocks.run.mockReturnValue(fake.child);
      const promise = start(path, BASE_OPTS);
      fake.printDeeplink('realm=http://127.0.0.1:8000&position=0,0');
      await promise;

      await expect(Promise.all([stopPreview(path), stopPreview(path)])).resolves.toBeDefined();
      expect(getPreview(path)).toBeUndefined();
    });
  });

  describe('when the app quits while a preview is still starting', () => {
    it('should kill it without reinstalling or spawning again', async () => {
      const fake = createFakeChild();
      mocks.run.mockReturnValue(fake.child);

      const promise = start(path, BASE_OPTS);
      await vi.waitFor(() => expect(mocks.run).toHaveBeenCalledTimes(1));

      await killAllPreviews();
      await expect(promise).resolves.toBe(path);

      expect(fake.child.kill).toHaveBeenCalled();
      expect(mocks.install).not.toHaveBeenCalled();
      expect(mocks.run).toHaveBeenCalledTimes(1);
    });
  });
});

describe('getDeployCommandSupport', () => {
  let files: Record<string, string>;
  let result: DeployCommandSupport;

  beforeEach(() => {
    vi.clearAllMocks();
    files = {};
    const find = (file: string) => {
      const name = Object.keys(files).find(suffix => file.endsWith(suffix));
      if (name === undefined) throw new Error('ENOENT');
      return name;
    };
    mocks.readFile.mockImplementation(async (file: string) => files[find(file)]);
    mocks.stat.mockImplementation(async (file: string) => {
      find(file);
      return { mtimeMs: 1 };
    });
  });

  const read = async () => {
    result = await getDeployCommandSupport(`/scenes/scene-${++sceneCount}`);
  };

  describe('when the deploy command predates programmatic deploys', () => {
    beforeEach(async () => {
      files['deploy/index.js'] = 'deploy command';
      await read();
    });

    it('should report it as legacy, without any newer capability', () => {
      expect(result).toEqual({
        legacy: true,
        multiScene: false,
        deployResource: false,
        deployToken: false,
      });
    });
  });

  describe('when the deploy command ships its routes module and declares every flag', () => {
    beforeEach(async () => {
      files['deploy/index.js'] =
        "const args = { '--programmatic': Boolean, '--multi-scene': Boolean, '--deploy-token': String }";
      files['deploy/routes.js'] = "router.delete('/api/deploy')";
      await read();
    });

    it('should report every capability', () => {
      expect(result).toEqual({
        legacy: false,
        multiScene: true,
        deployResource: true,
        deployToken: true,
      });
    });

    it('should read the deploy command once', () => {
      expect(mocks.readFile).toHaveBeenCalledTimes(1);
    });
  });

  describe('when the deploy command only mentions respond-async', () => {
    beforeEach(async () => {
      files['deploy/index.js'] = "'--programmatic' --help: Prefer: respond-async";
      await read();
    });

    it('should report the deploy resource without a deploy token', () => {
      expect({ deployResource: result.deployResource, deployToken: result.deployToken }).toEqual({
        deployResource: true,
        deployToken: false,
      });
    });
  });

  describe('when the deploy command declares the token flag with double quotes', () => {
    beforeEach(async () => {
      files['deploy/index.js'] =
        'const args = { "--programmatic": Boolean, "--deploy-token": String }';
      await read();
    });

    it('should still report the token flag', () => {
      expect(result.deployToken).toBe(true);
    });
  });

  describe('when the sdk is not installed', () => {
    let error: unknown;

    beforeEach(async () => {
      error = await read().catch(e => e);
    });

    it('should fail, as there is no deploy command to run', () => {
      expect(error).toBeInstanceOf(Error);
    });
  });
});

// Fake deploy runner (src/deploy-runner.ts): prints the given stdout chunks once cli.ts listens,
// then exits with `exitCode` when `exitAfter` is set. Without it, it runs until killed.
function createFakeRunner(chunks: string[], opts: { exitAfter?: boolean; exitCode?: number } = {}) {
  let alive = true;
  let settle!: (error?: Error) => void;
  const waitPromise = new Promise<Buffer>((resolve, reject) => {
    settle = error => (error ? reject(error) : resolve(Buffer.from('')));
  });
  waitPromise.catch(() => {});
  const exit = (code = 0) => {
    if (!alive) return;
    alive = false;
    settle(code === 0 ? undefined : new Error(`exited with code=${code}`));
  };
  const child = {
    on: vi.fn((_pattern: RegExp, handler: (data?: string) => void) => {
      void Promise.resolve().then(() => {
        for (const chunk of chunks) handler(chunk);
        if (opts.exitAfter) exit(opts.exitCode ?? 1);
      });
      return 0;
    }),
    wait: () => waitPromise,
    kill: vi.fn(async () => exit(0)),
    alive: () => alive,
  } as unknown as Child;
  return child;
}

const RUNNER_READY = formatDeployRunnerReady();

describe('deploy', () => {
  let files: Record<string, string>;
  let result: Awaited<ReturnType<typeof deploy>>;
  let args: string[];
  let path: string;

  beforeEach(() => {
    vi.clearAllMocks();
    files = { 'deploy/index.js': 'deploy command with --programmatic' };
    const find = (file: string) => {
      const name = Object.keys(files).find(suffix => file.endsWith(suffix));
      if (name === undefined) throw new Error('ENOENT');
      return name;
    };
    mocks.readFile.mockImplementation(async (file: string) => files[find(file)]);
    mocks.stat.mockImplementation(async (file: string) => {
      find(file);
      return { mtimeMs: 1 };
    });
    mocks.run.mockImplementation(() => createFakeRunner([RUNNER_READY]));
  });

  const run = async () => {
    path = `/scenes/scene-${++sceneCount}`;
    result = await deploy({ path, chainId: 1, wallet: '0xabc' });
    args = mocks.run.mock.calls[0][2].args;
  };

  describe('when the CLI serves the deploy resource and takes a deploy token', () => {
    beforeEach(async () => {
      files['deploy/index.js'] =
        "const args = { '--programmatic': Boolean, '--deploy-token': String }";
      files['deploy/routes.js'] = "router.delete('/api/deploy')";
      await run();
    });

    it('should report the deploy resource as supported', () => {
      expect(result.supportsDeployResource).toBe(true);
    });

    it('should hand out a token for the deployment', () => {
      expect(result.deployToken).toMatch(/^[0-9a-f-]{36}$/);
    });

    it('should pass the token to the CLI', () => {
      expect(args.slice(args.indexOf('--deploy-token'))).toEqual([
        '--deploy-token',
        result.deployToken,
      ]);
    });

    it('should return the port the CLI serves', () => {
      expect(result.port).toBe(4000);
    });

    it('should read the deploy command once', () => {
      expect(mocks.readFile).toHaveBeenCalledTimes(1);
    });

    it('should keep the editor files out of the scene before deploying it', () => {
      expect(mocks.ensureEditorFilesNotDeployed).toHaveBeenCalledWith(
        args[args.indexOf('--dir') + 1],
      );
    });
  });

  describe('when the deploy starts', () => {
    beforeEach(async () => {
      await run();
    });

    it('should run the deploy runner in a process of its own, in the scene folder', () => {
      expect(mocks.run).toHaveBeenCalledWith(
        '@dcl-creator-hub/deploy-runner',
        'deploy-runner',
        expect.objectContaining({ cwd: path }),
      );
    });

    it('should copy the runner bundle where the process can run it', () => {
      expect(mocks.ensureWorkerPackage).toHaveBeenCalledWith(
        expect.objectContaining({
          pkg: '@dcl-creator-hub/deploy-runner',
          bundle: 'deploy-runner.js',
        }),
      );
    });

    it('should run the CLI in programmatic mode', () => {
      expect(args).toContain('--programmatic');
    });
  });

  describe('when the scene is published twice', () => {
    beforeEach(async () => {
      await run();
      await run();
    });

    it('should start a new process for each publish, so each one reads node_modules as it is now', () => {
      expect(mocks.run).toHaveBeenCalledTimes(2);
    });
  });

  describe('when the ready line arrives split across two chunks', () => {
    beforeEach(async () => {
      mocks.run.mockImplementation(() =>
        createFakeRunner(['building…\n[deploy-run', 'ner] ready\n']),
      );
      await run();
    });

    it('should resolve once the line is complete', () => {
      expect(result.port).toBe(4000);
    });
  });

  describe('when the CLI fails before its server listens', () => {
    let runner: Child;
    let error: Error;

    beforeEach(async () => {
      runner = createFakeRunner(
        [
          formatDeployRunnerFailure(
            Object.assign(new Error('Could not rebuild bundle'), { name: 'BUNDLE_REBUILD_FAILED' }),
          ),
        ],
        { exitAfter: true },
      );
      mocks.run.mockReturnValue(runner);
      error = await deploy({ path: `/scenes/scene-${++sceneCount}`, chainId: 1, wallet: '0xabc' })
        .then(() => {
          throw new Error('deploy resolved');
        })
        .catch(e => e);
    });

    it('should reject with the CLI error name and message', () => {
      expect({ name: error.name, message: error.message }).toEqual({
        name: 'BUNDLE_REBUILD_FAILED',
        message: 'Could not rebuild bundle',
      });
    });
  });

  describe('when the runner exits without saying why', () => {
    let error: Error;

    beforeEach(async () => {
      mocks.run.mockImplementation(() => createFakeRunner([], { exitAfter: true, exitCode: 0 }));
      error = await deploy({ path: `/scenes/scene-${++sceneCount}`, chainId: 1, wallet: '0xabc' })
        .then(() => {
          throw new Error('deploy resolved');
        })
        .catch(e => e);
    });

    it('should reject instead of waiting forever', () => {
      expect(error.message).toBe('The deploy stopped before it was ready.');
    });
  });

  describe('when the CLI serves the deploy resource but takes no deploy token', () => {
    beforeEach(async () => {
      files['deploy/routes.js'] = "router.delete('/api/deploy')";
      await run();
    });

    it('should report the deploy resource as supported without a token', () => {
      expect(result).toEqual({ port: 4000, supportsDeployResource: true, deployToken: undefined });
    });

    it('should not pass a token to the CLI', () => {
      expect(args).not.toContain('--deploy-token');
    });
  });

  describe('when a deploy starts while the previous one is still starting', () => {
    let first: Child;
    let second: Child;
    let releaseFirst: () => void;

    beforeEach(async () => {
      let handler: (data?: string) => void = () => undefined;
      first = createFakeRunner([]);
      vi.mocked(first.on).mockImplementation((_pattern, onData) => {
        handler = onData;
        return 0;
      });
      releaseFirst = () => handler(RUNNER_READY);
      second = createFakeRunner([RUNNER_READY]);
      mocks.run.mockReturnValueOnce(first).mockReturnValueOnce(second);
      const options = { path: `/scenes/scene-${++sceneCount}`, chainId: 1, wallet: '0xabc' };
      const firstDeploy = deploy(options);
      const secondDeploy = deploy(options);
      await vi.waitFor(() => expect(mocks.run).toHaveBeenCalledTimes(1));
      releaseFirst();
      await Promise.all([firstDeploy, secondDeploy]);
    });

    it('should stop the first CLI before starting the second, leaving none untracked', () => {
      expect({
        firstStops: vi.mocked(first.kill).mock.calls.length,
        secondStops: vi.mocked(second.kill).mock.calls.length,
        starts: mocks.run.mock.calls.length,
      }).toEqual({ firstStops: 1, secondStops: 0, starts: 2 });
    });
  });

  describe('when the CLI predates the deploy resource', () => {
    beforeEach(async () => {
      await run();
    });

    it('should not hand out a token', () => {
      expect(result).toEqual({ port: 4000, supportsDeployResource: false, deployToken: undefined });
    });

    it('should not pass a token to the CLI', () => {
      expect(args).not.toContain('--deploy-token');
    });
  });
});

describe('init', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.ensureEditorFilesNotDeployed.mockReset();
    mocks.addDependencyOverrides.mockReset();
  });

  describe('when the template is downloaded', () => {
    beforeEach(async () => {
      await init('/scenes/new-scene', 'https://github.com/decentraland/sdk-empty-scene-template');
    });

    it('should keep the template’s editor files out of the scene’s deploys', () => {
      expect(mocks.ensureEditorFilesNotDeployed).toHaveBeenCalledWith('/scenes/new-scene');
    });

    it('should add the dependency overrides before anything installs', () => {
      expect(mocks.addDependencyOverrides).toHaveBeenCalledWith('/scenes/new-scene');
    });
  });

  describe('when the .dclignore and package.json cannot be written', () => {
    beforeEach(() => {
      const denied = () => {
        throw new Error('EACCES');
      };
      mocks.ensureEditorFilesNotDeployed.mockImplementation(denied);
      mocks.addDependencyOverrides.mockImplementation(denied);
    });

    it('should still create the scene', async () => {
      await expect(
        init('/scenes/new-scene', 'https://github.com/decentraland/sdk-empty-scene-template'),
      ).resolves.toBeUndefined();
    });
  });
});
