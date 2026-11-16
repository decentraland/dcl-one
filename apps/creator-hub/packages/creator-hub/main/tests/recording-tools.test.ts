import crypto from 'crypto';
import fs from 'fs';
import os from 'os';
import path from 'path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({ userData: '/tmp', run: vi.fn() }));
vi.mock('electron', () => ({
  app: { getPath: () => mocks.userData, getVersion: () => '1.0.0' },
}));
vi.mock('electron-log/main', () => ({ default: { info: vi.fn(), warn: vi.fn(), error: vi.fn() } }));
vi.mock('../src/modules/bin', () => ({ run: mocks.run }));

import {
  downloadModel,
  install,
  MODELS,
  modelFor,
  modelUrl,
  platformManifest,
  VAD_MODEL,
  vadModelUrl,
} from '../src/modules/recording/tools';

const toolsDir = path.join(__dirname, '../src/modules/recording/tools');

describe('when installing speech-to-text on this platform', () => {
  it('should ask npm for exactly what the committed lockfile pins', () => {
    const lockfile = path.join(toolsDir, `${process.platform}-${process.arch}.package-lock.json`);
    const lock = JSON.parse(fs.readFileSync(lockfile, 'utf8'));
    expect(lock.packages[''].dependencies).toEqual(JSON.parse(platformManifest()).dependencies);
  });

  it('should make the plain CPU/Metal build a required package and every other build optional', () => {
    const lockfile = path.join(toolsDir, `${process.platform}-${process.arch}.package-lock.json`);
    const { packages } = JSON.parse(fs.readFileSync(lockfile, 'utf8'));
    const required = Object.keys(packages).filter(key => key !== '' && !packages[key].optional);
    expect(required.sort()).toEqual([
      `node_modules/@fugood/node-whisper-${process.platform}-${process.arch}`,
      'node_modules/@fugood/whisper.node',
    ]);
  });
});

describe('when picking the speech model', () => {
  it('should use the English-only model for English and the multilingual one otherwise', () => {
    expect(modelFor('en')).toBe('en');
    expect(modelFor('es')).toBe('multilingual');
    expect(modelFor('zh')).toBe('multilingual');
  });

  it('should download it from a pinned commit', () => {
    expect(modelUrl('en')).toMatch(
      /^https:\/\/huggingface\.co\/ggerganov\/whisper\.cpp\/resolve\/[0-9a-f]{40}\/ggml-small\.en-q8_0\.bin$/,
    );
    expect(MODELS.en.sha256).toMatch(/^[0-9a-f]{64}$/);
  });
});

describe('when picking the speech detector', () => {
  it('should download Silero from a pinned commit and check it', () => {
    expect(vadModelUrl()).toMatch(
      /^https:\/\/huggingface\.co\/ggml-org\/whisper-vad\/resolve\/[0-9a-f]{40}\/ggml-silero-v5\.1\.2\.bin$/,
    );
    expect(VAD_MODEL.sha256).toMatch(/^[0-9a-f]{64}$/);
  });
});

function sha256(data: Buffer): string {
  return crypto.createHash('sha256').update(data).digest('hex');
}

// A fetch whose body streams `chunks`, then ends or fails as told.
function fakeFetch(
  chunks: Buffer[],
  { status = 200, failAfter }: { status?: number; failAfter?: number } = {},
) {
  const cancel = vi.fn(async () => {});
  const fetchImpl = vi.fn(async () => {
    let sent = 0;
    const body = new ReadableStream<Uint8Array>({
      pull(controller) {
        if (failAfter !== undefined && sent >= failAfter) {
          controller.error(new Error('socket hang up'));
          return;
        }
        if (sent >= chunks.length) {
          controller.close();
          return;
        }
        controller.enqueue(new Uint8Array(chunks[sent]));
        sent += 1;
      },
    });
    const total = chunks.reduce((n, c) => n + c.byteLength, 0);
    return {
      ok: status >= 200 && status < 300,
      status,
      headers: new Headers({ 'content-length': String(total) }),
      body: Object.assign(body, { cancel }),
    } as unknown as Response;
  });
  return { fetchImpl, cancel };
}

describe('when downloading a speech model', () => {
  let dir: string;
  let target: string;
  const data = Buffer.alloc(3000, 7);

  beforeEach(() => {
    dir = fs.mkdtempSync(path.join(os.tmpdir(), 'model-download-'));
    target = path.join(dir, 'models', 'model.bin');
  });

  afterEach(() => fs.rmSync(dir, { recursive: true, force: true }));

  it('should write it through a partial file, check its hash and report progress', async () => {
    const { fetchImpl } = fakeFetch([data.subarray(0, 1000), data.subarray(1000)]);
    const percents: number[] = [];
    await downloadModel(
      { url: 'https://example.invalid/m', target, sha256: sha256(data), bytes: 3000 },
      percent => percents.push(percent),
      fetchImpl,
    );
    expect(fs.readFileSync(target)).toEqual(data);
    expect(fs.existsSync(`${target}.download`)).toBe(false);
    expect(percents).toEqual([33, 100]);
  });

  it('should cancel the body and fail on an HTTP error', async () => {
    const { fetchImpl, cancel } = fakeFetch([data], { status: 503 });
    await expect(
      downloadModel(
        { url: 'https://example.invalid/m', target, sha256: sha256(data), bytes: 3000 },
        () => {},
        fetchImpl,
      ),
    ).rejects.toThrow('Downloading the speech model failed (HTTP 503).');
    expect(cancel).toHaveBeenCalled();
    expect(fs.existsSync(target)).toBe(false);
  });

  it('should fail and clean up when the stream breaks mid-way', async () => {
    const { fetchImpl } = fakeFetch([data.subarray(0, 1000), data.subarray(1000)], {
      failAfter: 1,
    });
    await expect(
      downloadModel(
        { url: 'https://example.invalid/m', target, sha256: sha256(data), bytes: 3000 },
        () => {},
        fetchImpl,
      ),
    ).rejects.toThrow('socket hang up');
    expect(fs.existsSync(target)).toBe(false);
    expect(fs.existsSync(`${target}.download`)).toBe(false);
  });

  it('should fail and clean up when the file cannot be written', async () => {
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.mkdirSync(`${target}.download`);
    const { fetchImpl } = fakeFetch([data]);
    await expect(
      downloadModel(
        { url: 'https://example.invalid/m', target, sha256: sha256(data), bytes: 3000 },
        () => {},
        fetchImpl,
      ),
    ).rejects.toThrow(/EISDIR|illegal operation on a directory/);
    expect(fs.existsSync(target)).toBe(false);
  });

  it('should refuse a file whose hash is not the pinned one', async () => {
    const { fetchImpl } = fakeFetch([data]);
    await expect(
      downloadModel(
        { url: 'https://example.invalid/m', target, sha256: 'f'.repeat(64), bytes: 3000 },
        () => {},
        fetchImpl,
      ),
    ).rejects.toThrow('checksum mismatch');
    expect(fs.existsSync(target)).toBe(false);
    expect(fs.existsSync(`${target}.download`)).toBe(false);
  });

  it('should skip the download when the file is already there and intact', async () => {
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.writeFileSync(target, data);
    const { fetchImpl } = fakeFetch([data]);
    await downloadModel(
      { url: 'https://example.invalid/m', target, sha256: sha256(data), bytes: 3000 },
      () => {},
      fetchImpl,
    );
    expect(fetchImpl).not.toHaveBeenCalled();
  });
});

describe('when two installs are asked for', () => {
  let fetched: string[];

  beforeEach(() => {
    mocks.userData = fs.mkdtempSync(path.join(os.tmpdir(), 'recording-tools-'));
    mocks.run.mockImplementation((_pkg: string, _bin: string, options: { cwd: string }) => ({
      wait: async () => {
        const manifest = JSON.parse(
          fs.readFileSync(path.join(options.cwd, 'package.json'), 'utf8'),
        );
        for (const pkg of Object.keys(manifest.dependencies)) {
          const pkgDir = path.join(options.cwd, 'node_modules', pkg);
          fs.mkdirSync(pkgDir, { recursive: true });
          fs.writeFileSync(path.join(pkgDir, 'package.json'), '{}');
        }
        return Buffer.alloc(0);
      },
      kill: vi.fn(),
    }));
    fetched = [];
    // Every download fails its checksum: enough to see which models were asked for, in order.
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string) => {
        fetched.push(url);
        return new Response(new Uint8Array([1, 2, 3]), {
          status: 200,
          headers: { 'content-length': '3' },
        });
      }),
    );
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    fs.rmSync(mocks.userData, { recursive: true, force: true });
  });

  it('should share one install for the same model', async () => {
    const first = install('en', () => {});
    const second = install('en', () => {});
    expect(second).toBe(first);
    await expect(first).rejects.toThrow('checksum mismatch');
    expect(fetched).toEqual([modelUrl('en')]);
  });

  it('should install the other model after the running one, not report it ready', async () => {
    const statuses: string[] = [];
    const first = install('en', () => {});
    const second = install('multilingual', status => statuses.push(status.state));
    await expect(first).rejects.toThrow('checksum mismatch');
    await expect(second).rejects.toThrow('checksum mismatch');
    expect(fetched).toEqual([modelUrl('en'), modelUrl('multilingual')]);
    expect(statuses).not.toContain('ready');
  });
});
