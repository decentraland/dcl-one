import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import type * as Chokidar from 'chokidar';
import { watch } from 'chokidar';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { PROJECT_ASSETS_CHANGED_EVENT } from '/shared/types/ipc';

import { isCatalogAsset, startProjectWatcher, stopProjectWatcher } from './project-watcher';
import { getWindow } from './window';

vi.mock('chokidar', async importOriginal => {
  const chokidar = await importOriginal<typeof Chokidar>();
  return { ...chokidar, watch: vi.fn(chokidar.watch) };
});
vi.mock('./window', () => ({ getWindow: vi.fn() }));
vi.mock('../mainWindow', () => ({ MAIN_WINDOW_ID: 'main' }));
vi.mock('electron-log/main', () => ({ default: { info: vi.fn(), warn: vi.fn() } }));

const ROOT = '/scenes/demo';
const under = (rel: string) => `${ROOT}/${rel}`;

describe('project-watcher isCatalogAsset', () => {
  it('accepts a model dropped anywhere under assets/', () => {
    expect(isCatalogAsset(ROOT, under('assets/tree.glb'))).toBe(true);
    expect(isCatalogAsset(ROOT, under('assets/models/nested/tree.glb'))).toBe(true);
  });

  it('accepts the other catalog extensions under assets/', () => {
    for (const rel of [
      'assets/a.gltf',
      'assets/b.png',
      'assets/c.jpg',
      'assets/d.jpeg',
      'assets/e.mp3',
      'assets/f.ogg',
      'assets/g.wav',
      'assets/h.mp4',
      'assets/i.json',
    ]) {
      expect(isCatalogAsset(ROOT, under(rel))).toBe(true);
    }
  });

  it('ignores files outside the assets/ tree', () => {
    expect(isCatalogAsset(ROOT, under('src/index.ts'))).toBe(false);
    expect(isCatalogAsset(ROOT, under('scene.json'))).toBe(false);
    expect(isCatalogAsset(ROOT, under('package.json'))).toBe(false);
  });

  it('ignores the scene-graph autosave the editor writes itself', () => {
    // These are the feedback-loop guards: without them the inspector's own ~100ms
    // main.composite autosave (a `.composite`, a catalog extension) would loop the refresh.
    expect(isCatalogAsset(ROOT, under('assets/scene/main.composite'))).toBe(false);
    expect(isCatalogAsset(ROOT, under('assets/scene/main.crdt'))).toBe(false);
    expect(isCatalogAsset(ROOT, under('assets/scene/entity-names.ts'))).toBe(false);
  });

  it('ignores a non-catalog extension even under assets/', () => {
    expect(isCatalogAsset(ROOT, under('assets/notes.txt'))).toBe(false);
    expect(isCatalogAsset(ROOT, under('assets/model.blend'))).toBe(false);
  });

  it('ignores a path outside the project root', () => {
    expect(isCatalogAsset(ROOT, '/elsewhere/assets/tree.glb')).toBe(false);
  });

  it('ignores the project root itself', () => {
    expect(isCatalogAsset(ROOT, ROOT)).toBe(false);
  });
});

describe('startProjectWatcher', () => {
  let project: string;
  let send: ReturnType<typeof vi.fn>;

  async function watchedFolders(): Promise<string[]> {
    const watcher = vi.mocked(watch).mock.results[0].value;
    await new Promise(resolve => watcher.once('ready', resolve));
    return Object.keys(watcher.getWatched())
      .map(folder => path.relative(project, folder).replace(/\\/g, '/'))
      .sort();
  }

  beforeEach(() => {
    project = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'project-watcher-')));
    for (const folder of [
      'assets/models',
      'src',
      'node_modules/pkg/assets',
      '.git/objects',
      'bin',
    ]) {
      fs.mkdirSync(path.join(project, folder), { recursive: true });
    }
    fs.writeFileSync(path.join(project, 'node_modules/pkg/assets/model.glb'), 'model');
    send = vi.fn();
    vi.mocked(getWindow).mockReturnValue({
      isDestroyed: () => false,
      webContents: { send },
    } as unknown as ReturnType<typeof getWindow>);
  });

  afterEach(async () => {
    await stopProjectWatcher(project);
    fs.rmSync(project, { recursive: true, force: true });
    vi.mocked(watch).mockClear();
  });

  describe('when the project has folders that the watcher ignores', () => {
    it('should watch the other folders only', async () => {
      startProjectWatcher(project);

      const folders = await watchedFolders();

      expect(folders).toEqual(expect.arrayContaining(['', 'assets', 'assets/models', 'src']));
      expect(folders.filter(folder => /^(node_modules|\.git|bin)(\/|$)/.test(folder))).toEqual([]);
    });
  });

  describe('when a model is added below assets after the start', () => {
    it('should tell the main window that the assets of the project changed', async () => {
      startProjectWatcher(project);
      await watchedFolders();

      fs.writeFileSync(path.join(project, 'assets/models/tree.glb'), 'model');

      await vi.waitFor(() => expect(send).toHaveBeenCalled(), { timeout: 10000, interval: 50 });
      expect(send).toHaveBeenCalledWith(PROJECT_ASSETS_CHANGED_EVENT, { path: project });
    });
  });

  describe('when the same project is started two times', () => {
    it('should keep one watcher', () => {
      startProjectWatcher(project);
      startProjectWatcher(project);

      expect(watch).toHaveBeenCalledTimes(1);
    });
  });
});
