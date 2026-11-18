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
});
