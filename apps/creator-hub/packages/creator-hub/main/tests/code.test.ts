import fs from 'fs/promises';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { EditorConfig } from '/shared/types/config';

import { addEditor, addEditorsPathsToConfig, open } from '../src/modules/code';

// `vi.hoisted` is required here: code.ts imports `child_process` and `electron` at its
// top, so the factories below run before plain module-level consts are initialized.
const { launch, spawnSpy, execSpy, openPathSpy, showItemInFolderSpy } = vi.hoisted(() => {
  const succeed = (...args: unknown[]) => {
    const cb = args[args.length - 1] as (err: unknown, res: unknown) => void;
    cb(null, { stdout: '', stderr: '' });
  };
  // A child process that starts, or fails to start with `error`, right after `spawn` returns.
  const launch =
    (error?: Error) =>
    (..._args: unknown[]) => {
      const handlers: Record<string, (arg?: unknown) => void> = {};
      queueMicrotask(() => (error ? handlers.error?.(error) : handlers.spawn?.()));
      return {
        once: (event: string, handler: (arg?: unknown) => void) => {
          handlers[event] = handler;
        },
        unref: vi.fn(),
      };
    };
  return {
    launch,
    spawnSpy: vi.fn(launch()),
    execSpy: vi.fn(succeed),
    openPathSpy: vi.fn(),
    showItemInFolderSpy: vi.fn(),
  };
});

vi.mock('child_process', () => ({ exec: execSpy, spawn: spawnSpy }));

vi.mock('electron', () => ({
  app: { getAppPath: () => '/app' },
  shell: {
    openPath: openPathSpy,
    showItemInFolder: showItemInFolderSpy,
    openExternal: vi.fn(),
  },
}));

// The two PATH entries `setup-node` prepends in production: the directory holding the `node`
// link to the Electron executable, and the one holding the npm scripts that resolve `node`
// through PATH. They are only injected in a packaged app, so they are stated here rather than
// produced by running the real setup.
const SHIM_DIR = '/app/../app.asar.unpacked';
const BUNDLED_NPM_DIR = '/app/node_modules/npm/bin';
const BUNDLED_NODE_DIR = '/resources/node-bin/bin';

vi.mock('../src/modules/setup-node', () => ({
  getInjectedPathEntries: () => [SHIM_DIR, BUNDLED_NPM_DIR],
}));

const { resolveNodeRuntimeSpy } = vi.hoisted(() => ({ resolveNodeRuntimeSpy: vi.fn() }));
vi.mock('../src/modules/node-runtime', () => ({ resolveNodeRuntime: resolveNodeRuntimeSpy }));

vi.mock('electron-log/main', () => ({
  default: { info: vi.fn(), warn: vi.fn(), error: vi.fn() },
}));

vi.mock('../src/modules/analytics', () => ({ track: vi.fn() }));

let editors: EditorConfig[] = [];

const configStorage = {
  get: vi.fn(async () => editors),
  set: vi.fn(async (_key: string, value: typeof editors) => {
    editors = value;
  }),
};

vi.mock('../src/modules/config', () => ({
  getConfigStorage: vi.fn(async () => configStorage),
}));

vi.mock('fs/promises', () => ({
  default: {
    stat: vi.fn(async () => ({ isDirectory: () => true })),
    readdir: vi.fn(async () => []),
    access: vi.fn(async () => undefined),
    constants: { X_OK: 1 },
  },
}));

const EDITOR_PATH = '/Applications/Visual Studio Code.app/Contents/MacOS/Electron';

describe('code.open', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resolveNodeRuntimeSpy.mockReturnValue({ source: 'bundled', binDir: BUNDLED_NODE_DIR });
    editors = [{ name: 'VSCode', path: EDITOR_PATH, isDefault: true }];
  });

  describe('when the path contains shell metacharacters', () => {
    const pathWithShellCharacters = '/projects/scene/a$(x)b`y`;z&w|v';

    it('should pass the whole path as a single argv element', async () => {
      await open(pathWithShellCharacters);

      expect(spawnSpy).toHaveBeenCalledTimes(1);
      const [file, args] = spawnSpy.mock.calls[0];
      expect(file).toBe(EDITOR_PATH);
      expect(args).toEqual([pathWithShellCharacters]);
    });

    it('should not build a shell command string', async () => {
      await open(pathWithShellCharacters);

      expect(execSpy).not.toHaveBeenCalled();
      for (const call of spawnSpy.mock.calls) {
        expect(typeof call[0]).toBe('string');
        expect(call[0]).not.toContain('$(');
        expect(call[0]).not.toContain(pathWithShellCharacters);
      }
    });
  });

  describe('when the editor launches successfully', () => {
    it('should not wait for the editor to exit or keep its output', async () => {
      await open('/projects/scene/main.ts');

      const [, , options] = spawnSpy.mock.calls[0] as [string, string[], Record<string, unknown>];
      expect(options).toMatchObject({ detached: true, stdio: 'ignore' });
      expect(spawnSpy.mock.results[0].value.unref).toHaveBeenCalled();
    });

    it('should not also reveal the path in the file manager', async () => {
      await open('/projects/scene/main.ts');

      expect(spawnSpy).toHaveBeenCalledTimes(1);
      expect(showItemInFolderSpy).not.toHaveBeenCalled();
      expect(openPathSpy).not.toHaveBeenCalled();
    });
  });

  describe('when reading the editor list fails', () => {
    beforeEach(() => {
      configStorage.get.mockRejectedValueOnce(new Error('config unreadable'));
    });

    it('should reveal the path instead of rejecting', async () => {
      await expect(open('/projects/scene/main.ts')).resolves.toBeUndefined();

      expect(showItemInFolderSpy).toHaveBeenCalledWith('/projects/scene/main.ts');
    });
  });

  describe('when the editor process fails', () => {
    beforeEach(() => {
      spawnSpy.mockImplementationOnce(launch(new Error('spawn ENOENT')));
    });

    it('should reveal the path in the file manager', async () => {
      await open('/projects/scene/main.ts');

      expect(showItemInFolderSpy).toHaveBeenCalledWith('/projects/scene/main.ts');
    });

    it('should not hand the path to the OS default handler', async () => {
      await open('/projects/scene/main.ts');

      expect(openPathSpy).not.toHaveBeenCalled();
    });
  });

  describe('when no editor is configured as default', () => {
    beforeEach(() => {
      editors = [{ name: 'VSCode', path: EDITOR_PATH, isDefault: false }];
    });

    it('should reveal the path without launching a process', async () => {
      await open('/projects/scene/main.ts');

      expect(spawnSpy).not.toHaveBeenCalled();
      expect(showItemInFolderSpy).toHaveBeenCalledWith('/projects/scene/main.ts');
    });

    it('should not hand the path to the OS default handler', async () => {
      await open('/projects/scene/main.ts');

      expect(openPathSpy).not.toHaveBeenCalled();
    });
  });

  describe('when the path is a file type the OS runs on open', () => {
    const batFilePath = '/projects/scene/build.bat';

    it('should never reach the OS handler, with or without an editor', async () => {
      await open(batFilePath);

      editors = [{ name: 'VSCode', path: EDITOR_PATH, isDefault: false }];
      await open(batFilePath);

      expect(openPathSpy).not.toHaveBeenCalled();
    });
  });

  describe('when handing the environment to the editor', () => {
    const ORIGINAL_PATH = process.env.PATH;
    const ORIGINAL_RUN_AS_NODE = process.env.ELECTRON_RUN_AS_NODE;

    beforeEach(() => {
      process.env.PATH = [SHIM_DIR, BUNDLED_NPM_DIR, '/usr/bin', '/bin'].join(':');
      process.env.ELECTRON_RUN_AS_NODE = '1';
    });

    afterEach(() => {
      process.env.PATH = ORIGINAL_PATH;
      if (ORIGINAL_RUN_AS_NODE === undefined) delete process.env.ELECTRON_RUN_AS_NODE;
      else process.env.ELECTRON_RUN_AS_NODE = ORIGINAL_RUN_AS_NODE;
    });

    function editorEnv() {
      const [, , options] = spawnSpy.mock.calls[0] as [
        string,
        string[],
        { env: NodeJS.ProcessEnv },
      ];
      return options.env;
    }

    it('should drop the entries whose node and npm are Creator Hub itself', async () => {
      await open('/projects/scene/main.ts');

      const entries = (editorEnv().PATH ?? '').split(':');
      expect(entries).not.toContain(SHIM_DIR);
      expect(entries).not.toContain(BUNDLED_NPM_DIR);
    });

    it('should put the real bundled node first so node and npm still resolve', async () => {
      await open('/projects/scene/main.ts');

      const entries = (editorEnv().PATH ?? '').split(':');
      expect(entries[0]).toBe(BUNDLED_NODE_DIR);
    });

    it('should keep the rest of the inherited PATH in order', async () => {
      await open('/projects/scene/main.ts');

      const entries = (editorEnv().PATH ?? '').split(':');
      expect(entries.slice(1)).toEqual(['/usr/bin', '/bin']);
    });

    describe('and the only Node runtime is Electron itself', () => {
      beforeEach(() => {
        resolveNodeRuntimeSpy.mockReturnValue({ source: 'electron', binDir: SHIM_DIR });
      });

      it('should not put the shim directory back in', async () => {
        await open('/projects/scene/main.ts');

        expect((editorEnv().PATH ?? '').split(':')).toEqual(['/usr/bin', '/bin']);
      });
    });

    it('should not leak ELECTRON_RUN_AS_NODE into the editor', async () => {
      await open('/projects/scene/main.ts');

      expect(editorEnv()).not.toHaveProperty('ELECTRON_RUN_AS_NODE');
    });
  });
});

describe('when the platform is Linux', () => {
  const originalPlatform = process.platform;
  const originalPath = process.env.PATH;
  const INSTALLED = [
    '/usr/bin/code',
    '/usr/bin/subl',
    '/opt/zed/bin/zed',
    '/snap/bin/cursor',
    '/opt/tools/constructor',
  ];
  // Files that are there but that the user cannot run.
  const NOT_EXECUTABLE = ['/home/user/bin/code', '/usr/share/applications/code.desktop'];
  const LAUNCHER = '/home/user/Desktop/code.desktop';

  beforeEach(() => {
    vi.clearAllMocks();
    Object.defineProperty(process, 'platform', { value: 'linux' });
    process.env.PATH = '/home/user/bin:/usr/bin';
    editors = [];
    vi.mocked(fs.stat).mockImplementation((async (file: string) => {
      if (INSTALLED.includes(file) || NOT_EXECUTABLE.includes(file) || file === LAUNCHER) {
        return { isDirectory: () => false, isFile: () => true };
      }
      if (file === '/usr/bin') return { isDirectory: () => true, isFile: () => false };
      throw new Error('ENOENT');
    }) as never);
    vi.mocked(fs.access).mockImplementation((async (file: string) => {
      if (!INSTALLED.includes(file) && file !== LAUNCHER) throw new Error('EACCES');
    }) as never);
  });

  afterEach(() => {
    Object.defineProperty(process, 'platform', { value: originalPlatform });
    process.env.PATH = originalPath;
    vi.mocked(fs.stat).mockImplementation((async () => ({ isDirectory: () => true })) as never);
    vi.mocked(fs.access).mockImplementation((async () => undefined) as never);
  });

  describe('code.addEditorsPathsToConfig', () => {
    it('should add the editors whose command is in PATH or the snap folder, and skip a file that cannot run', async () => {
      await addEditorsPathsToConfig();

      expect(editors).toEqual([
        { name: 'Visual Studio Code', path: '/usr/bin/code', isDefault: true },
        { name: 'Cursor', path: '/snap/bin/cursor', isDefault: false },
        { name: 'Sublime Text', path: '/usr/bin/subl', isDefault: false },
      ]);
    });

    it('should find a Flatpak editor that is not in PATH', async () => {
      INSTALLED.push('/var/lib/flatpak/exports/bin/com.vscodium.codium');
      try {
        await addEditorsPathsToConfig();
      } finally {
        INSTALLED.pop();
      }

      expect(editors).toContainEqual({
        name: 'VSCodium',
        path: '/var/lib/flatpak/exports/bin/com.vscodium.codium',
        isDefault: false,
      });
    });

    it('should list an editor installed in two ways once', async () => {
      INSTALLED.push('/var/lib/flatpak/exports/bin/com.visualstudio.code');
      try {
        await addEditorsPathsToConfig();
      } finally {
        INSTALLED.pop();
      }

      expect(editors.filter(editor => editor.name === 'Visual Studio Code')).toEqual([
        { name: 'Visual Studio Code', path: '/usr/bin/code', isDefault: true },
      ]);
    });

    it('should not run the search of another platform', async () => {
      await addEditorsPathsToConfig();

      expect(execSpy).not.toHaveBeenCalled();
    });
  });

  describe('code.addEditor', () => {
    it('should accept a command that has no extension', async () => {
      await addEditor('/opt/zed/bin/zed');

      expect(editors).toEqual([
        { name: 'zed', path: '/opt/zed/bin/zed', isDefault: true, hidden: false },
      ]);
    });

    it('should give a known command the name of its editor', async () => {
      await addEditor('/snap/bin/cursor');

      expect(editors[0].name).toBe('Cursor');
    });

    it('should use the file name of a command that has the name of an object property', async () => {
      await addEditor('/opt/tools/constructor');

      expect(editors[0].name).toBe('constructor');
    });

    it('should reject a path that does not exist', async () => {
      await expect(addEditor('/usr/bin/missing')).rejects.toThrow('file_not_found');
    });

    it('should reject a file that the user cannot run', async () => {
      await expect(addEditor('/usr/share/applications/code.desktop')).rejects.toThrow(
        'invalid_executable',
      );
    });

    it('should reject a desktop entry that is marked executable', async () => {
      await expect(addEditor(LAUNCHER)).rejects.toThrow('invalid_executable');
    });

    it('should reject a directory', async () => {
      await expect(addEditor('/usr/bin')).rejects.toThrow('invalid_executable');
    });
  });
});
