import fs from 'fs';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  APPIMAGE_DESKTOP_ENTRY,
  appImageDesktopEntry,
  registerAppImageProtocolClient,
  runningAppImage,
} from '../src/modules/appimage';

const { execFileSpy } = vi.hoisted(() => ({ execFileSpy: vi.fn() }));

vi.mock('child_process', () => ({ execFile: execFileSpy }));
vi.mock('fs', () => ({
  default: { mkdirSync: vi.fn(), writeFileSync: vi.fn(), existsSync: vi.fn(() => false) },
}));
vi.mock('electron-log/main', () => ({ default: { error: vi.fn(), info: vi.fn() } }));

/** Answers `xdg-mime query default` with `current`, and every other command with success. */
function xdgMimeDefault(current: string) {
  execFileSpy.mockImplementation(
    (_file: string, args: string[], callback: (error: Error | null, stdout: string) => void) => {
      callback(null, args[0] === 'query' ? `${current}\n` : '');
    },
  );
}

const APPIMAGE = '/home/user/Apps/Decentraland Creator Hub-1.2.3-linux-x86_64.AppImage';

describe('appImageDesktopEntry', () => {
  it('should quote the path, which has spaces, and take the link as an argument', () => {
    expect(appImageDesktopEntry(APPIMAGE, 'dcl-creator-hub')).toContain(`Exec="${APPIMAGE}" %U\n`);
  });

  it('should name the link scheme that the entry opens', () => {
    expect(appImageDesktopEntry(APPIMAGE, 'dcl-creator-hub')).toContain(
      'MimeType=x-scheme-handler/dcl-creator-hub;\n',
    );
  });

  it('should escape the characters that the Exec key reserves', () => {
    expect(appImageDesktopEntry('/tmp/a"b$c\\d%e.AppImage', 'x')).toContain(
      'Exec="/tmp/a\\\\"b\\\\$c\\\\\\\\d%%e.AppImage" %U\n',
    );
  });
});

describe('registerAppImageProtocolClient', () => {
  const originalDataHome = process.env.XDG_DATA_HOME;

  beforeEach(() => {
    vi.clearAllMocks();
    xdgMimeDefault('');
    vi.mocked(fs.existsSync).mockImplementation(() => false);
    process.env.XDG_DATA_HOME = '/home/user/.local/share';
    vi.stubEnv('XDG_DATA_DIRS', '/usr/local/share:/usr/share');
    return () => {
      vi.unstubAllEnvs();
      if (originalDataHome === undefined) delete process.env.XDG_DATA_HOME;
      else process.env.XDG_DATA_HOME = originalDataHome;
    };
  });

  it('should write the entry in the applications folder of the user', () => {
    registerAppImageProtocolClient(APPIMAGE, 'dcl-creator-hub');

    expect(fs.writeFileSync).toHaveBeenCalledWith(
      `/home/user/.local/share/applications/${APPIMAGE_DESKTOP_ENTRY}`,
      appImageDesktopEntry(APPIMAGE, 'dcl-creator-hub'),
    );
  });

  it('should make the entry the handler of the link scheme', () => {
    registerAppImageProtocolClient(APPIMAGE, 'dcl-creator-hub');

    expect(execFileSpy).toHaveBeenCalledWith(
      'xdg-mime',
      ['default', APPIMAGE_DESKTOP_ENTRY, 'x-scheme-handler/dcl-creator-hub'],
      expect.any(Function),
    );
  });

  it('should replace its own entry as the handler', () => {
    xdgMimeDefault(APPIMAGE_DESKTOP_ENTRY);

    registerAppImageProtocolClient(APPIMAGE, 'dcl-creator-hub');

    expect(execFileSpy).toHaveBeenCalledWith(
      'xdg-mime',
      ['default', APPIMAGE_DESKTOP_ENTRY, 'x-scheme-handler/dcl-creator-hub'],
      expect.any(Function),
    );
  });

  it('should keep another installed handler, such as the deb', () => {
    xdgMimeDefault('creator-hub.desktop');
    vi.mocked(fs.existsSync).mockImplementation(
      file => file === '/usr/share/applications/creator-hub.desktop',
    );

    registerAppImageProtocolClient(APPIMAGE, 'dcl-creator-hub');

    expect(execFileSpy).toHaveBeenCalledTimes(1);
    expect(execFileSpy).toHaveBeenCalledWith(
      'xdg-mime',
      ['query', 'default', 'x-scheme-handler/dcl-creator-hub'],
      expect.any(Function),
    );
  });

  it('should take over from a handler whose entry is gone', () => {
    xdgMimeDefault('creator-hub.desktop');

    registerAppImageProtocolClient(APPIMAGE, 'dcl-creator-hub');

    expect(execFileSpy).toHaveBeenCalledWith(
      'xdg-mime',
      ['default', APPIMAGE_DESKTOP_ENTRY, 'x-scheme-handler/dcl-creator-hub'],
      expect.any(Function),
    );
  });

  it('should not register a handler when it cannot write the entry', () => {
    vi.mocked(fs.writeFileSync).mockImplementationOnce(() => {
      throw new Error('EACCES');
    });

    registerAppImageProtocolClient(APPIMAGE, 'dcl-creator-hub');

    expect(execFileSpy).not.toHaveBeenCalled();
  });

  it('should not register a handler when the path has a line break', () => {
    registerAppImageProtocolClient('/tmp/a\nTerminal=true/x.AppImage', 'dcl-creator-hub');

    expect(fs.writeFileSync).not.toHaveBeenCalled();
    expect(execFileSpy).not.toHaveBeenCalled();
  });
});

describe('runningAppImage', () => {
  const env = { APPIMAGE, APPDIR: '/tmp/.mount_Creato1a2b3c' };

  it('should give the AppImage when the app runs from its mount folder', () => {
    expect(runningAppImage(env, '/tmp/.mount_Creato1a2b3c/creator-hub')).toBe(APPIMAGE);
  });

  it('should give nothing when the variables come from another AppImage', () => {
    expect(runningAppImage(env, '/opt/Decentraland Creator Hub/creator-hub')).toBeUndefined();
    expect(runningAppImage(env, '/tmp/.mount_Creato1a2b3cd/creator-hub')).toBeUndefined();
  });

  it('should give nothing when a variable is not set', () => {
    expect(runningAppImage({ APPIMAGE }, '/tmp/.mount_Creato1a2b3c/creator-hub')).toBeUndefined();
    expect(runningAppImage({}, '/usr/bin/creator-hub')).toBeUndefined();
  });
});
