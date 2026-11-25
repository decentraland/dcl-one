import fs from 'fs/promises';
import os from 'os';
import path from 'path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({ warn: vi.fn(), fetch: vi.fn(), extract: vi.fn() }));

vi.mock('electron-log/main', () => ({ default: { info: vi.fn(), warn: mocks.warn } }));
vi.mock('/shared/fetch', () => ({ fetch: mocks.fetch }));
vi.mock('@dcl/zip-extract', () => ({ default: mocks.extract }));

import { downloadGithubRepo } from '../src/modules/download-github-folder';

describe('downloadGithubRepo', () => {
  let destination: string;

  beforeEach(async () => {
    destination = await fs.mkdtemp(path.join(os.tmpdir(), 'download-github-folder-'));
    mocks.fetch.mockResolvedValue({ ok: true, arrayBuffer: async () => new ArrayBuffer(0) });
  });

  afterEach(async () => {
    vi.clearAllMocks();
    await fs.rm(destination, { recursive: true, force: true });
  });

  it('should log a warning for each entry the extraction skips', async () => {
    mocks.extract.mockImplementation(async (_zip: string, options: any) => {
      const root = path.join(options.dir, 'owner-repo-abc');
      await fs.mkdir(root);
      await fs.writeFile(path.join(root, 'scene.json'), '{}');
      options.onSkip({ fileName: 'owner-repo-abc/link' }, 'it is a link');
    });

    await downloadGithubRepo('https://github.com/owner/repo', destination);

    expect(mocks.warn).toHaveBeenCalledTimes(1);
    expect(mocks.warn.mock.calls[0][0]).toContain('"owner-repo-abc/link"');
    expect(mocks.warn.mock.calls[0][0]).toContain('it is a link');
    expect(await fs.readdir(destination)).toEqual(['scene.json']);
  });

  describe('when GitHub is slow to answer the first time', () => {
    beforeEach(() => {
      mocks.extract.mockImplementation(async (_zip: string, options: any) => {
        const root = path.join(options.dir, 'owner-repo-abc');
        await fs.mkdir(root);
        await fs.writeFile(path.join(root, 'scene.json'), '{}');
      });
      mocks.fetch
        .mockRejectedValueOnce(new Error('Request timeout'))
        .mockResolvedValueOnce({ ok: true, arrayBuffer: async () => new ArrayBuffer(0) });
    });

    it('should wait longer than the default request timeout and retry once', async () => {
      await downloadGithubRepo('https://github.com/owner/repo', destination);

      expect(mocks.fetch).toHaveBeenCalledTimes(2);
      expect(mocks.fetch).toHaveBeenCalledWith(
        'https://api.github.com/repos/owner/repo/zipball/main',
        undefined,
        30_000,
      );
      expect(await fs.readdir(destination)).toEqual(['scene.json']);
    });
  });

  describe('when GitHub answers with a server error and then the zip', () => {
    let cancel: ReturnType<typeof vi.fn>;

    beforeEach(() => {
      cancel = vi.fn().mockResolvedValue(undefined);
      mocks.extract.mockImplementation(async (_zip: string, options: any) => {
        const root = path.join(options.dir, 'owner-repo-abc');
        await fs.mkdir(root);
        await fs.writeFile(path.join(root, 'scene.json'), '{}');
      });
      mocks.fetch
        .mockResolvedValueOnce({ ok: false, status: 502, body: { cancel } })
        .mockResolvedValueOnce({ ok: true, arrayBuffer: async () => new ArrayBuffer(0) });
    });

    it('should release the failed response and use the retry', async () => {
      await downloadGithubRepo('https://github.com/owner/repo', destination);

      expect(cancel).toHaveBeenCalled();
      expect(mocks.fetch).toHaveBeenCalledTimes(2);
      expect(await fs.readdir(destination)).toEqual(['scene.json']);
    });
  });

  describe('when every attempt fails', () => {
    beforeEach(() => {
      mocks.fetch.mockRejectedValue(new Error('Request timeout'));
    });

    it('should give up after the retry', async () => {
      await expect(
        downloadGithubRepo('https://github.com/owner/repo', destination),
      ).rejects.toThrow();
      expect(mocks.fetch).toHaveBeenCalledTimes(2);
    });
  });

  describe('when GitHub says the repository does not exist', () => {
    beforeEach(() => {
      mocks.fetch.mockResolvedValue({ ok: false, status: 404, body: null });
    });

    it('should not retry', async () => {
      await expect(
        downloadGithubRepo('https://github.com/owner/repo', destination),
      ).rejects.toThrow('404');
      expect(mocks.fetch).toHaveBeenCalledTimes(1);
    });
  });
});
