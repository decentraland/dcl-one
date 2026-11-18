import fs from 'node:fs';
import fsp from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { TranscriptCutShort } from '../src/modules/recording/words';

import {
  TRANSCRIBE_JOB_FILE_ENV,
  type TranscribeWorkerJob,
  type TranscribeWorkerResult,
} from '../../shared/types/recording';

const mocks = vi.hoisted(() => ({
  run: vi.fn(),
  isInstalled: vi.fn(async () => true),
  toolsDir: '',
  StreamError: class StreamError extends Error {
    constructor(
      public type: string,
      message: string,
      public stdout: Buffer,
      public stderr: Buffer,
    ) {
      super(message);
    }
  },
}));
vi.mock('electron-log/main', () => ({
  default: { info: vi.fn(), warn: vi.fn(), error: vi.fn() },
}));
vi.mock('../src/modules/bin', () => ({ run: mocks.run, StreamError: mocks.StreamError }));
vi.mock('../src/modules/downloaded-tools', () => ({ ensureWorkerPackage: vi.fn(async () => {}) }));
vi.mock('../src/modules/recording/tools', () => ({
  isInstalled: mocks.isInstalled,
  getToolsDir: () => mocks.toolsDir,
  getModelPath: (id: string) => `/models/${id}.bin`,
  getVadModelPath: () => '/models/vad.bin',
}));

import { transcribe } from '../src/modules/recording/transcribe';

const { StreamError } = mocks;

type EnvOf = Record<string, string>;

type FakeWorker = {
  env: Record<string, string>;
  job: TranscribeWorkerJob | null;
};

function fakeWorker(outcome: {
  result?: TranscribeWorkerResult;
  stdout?: string;
  stderr?: string[];
  exit?: 'clean' | 'failed';
}): FakeWorker {
  const worker: FakeWorker = { env: {}, job: null };
  mocks.run.mockImplementation(
    (_pkg: string, _bin: string, options: { env: Record<string, string> }) => {
      worker.env = options.env;
      const handlers: ((data?: string) => void)[] = [];
      return {
        on: (_pattern: RegExp, handler: (data?: string) => void) => {
          handlers.push(handler);
          return handlers.length - 1;
        },
        kill: vi.fn(async () => {}),
        wait: async () => {
          const jobFile = options.env[TRANSCRIBE_JOB_FILE_ENV];
          worker.job = fs.existsSync(jobFile)
            ? (JSON.parse(fs.readFileSync(jobFile, 'utf8')) as TranscribeWorkerJob)
            : null;
          if (worker.job && outcome.result) {
            fs.writeFileSync(worker.job.resultPath, JSON.stringify(outcome.result));
          }
          for (const line of outcome.stderr ?? []) handlers.forEach(h => h(`${line}\n`));
          const stdout = Buffer.from(outcome.stdout ?? '');
          if (outcome.exit === 'failed') {
            throw new StreamError('COMMAND_FAILED', 'exited with code=1', stdout, Buffer.alloc(0));
          }
          return stdout;
        },
      };
    },
  );
  return worker;
}

describe('when the worker never answers', () => {
  let sessionDir: string;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    mocks.toolsDir = fs.mkdtempSync(path.join(os.tmpdir(), 'transcribe-tools-'));
    sessionDir = fs.mkdtempSync(path.join(os.tmpdir(), 'transcribe-session-'));
    fs.writeFileSync(path.join(sessionDir, 'audio.wav'), Buffer.alloc(44));
  });

  afterEach(() => {
    vi.useRealTimers();
    fs.rmSync(sessionDir, { recursive: true, force: true });
    fs.rmSync(mocks.toolsDir, { recursive: true, force: true });
  });

  it('should kill it, wait for it to die, then fail', async () => {
    const order: string[] = [];
    let started!: () => void;
    const running = new Promise<void>(resolve => (started = resolve));
    let die!: () => void;
    mocks.run.mockImplementation(() => {
      started();
      return {
        on: () => 0,
        kill: vi.fn(async () => {
          order.push('kill');
          await new Promise(resolve => setTimeout(resolve, 50));
          die();
          order.push('dead');
        }),
        wait: () =>
          new Promise<Buffer>((_, reject) => {
            die = () => reject(new Error('killed'));
          }),
      };
    });
    const result = transcribe(path.join(sessionDir, 'audio.wav'), 'en', 'en', undefined).then(
      () => order.push('resolved'),
      () => order.push('rejected'),
    );
    await running;
    await vi.advanceTimersByTimeAsync(10 * 60_000 + 100);
    await result;
    expect(order).toEqual(['kill', 'dead', 'rejected']);
  });

  it('should keep the words it had already written when it is killed for taking too long', async () => {
    let die!: () => void;
    mocks.run.mockImplementation((_pkg: string, _bin: string, options: { env: EnvOf }) => ({
      on: () => 0,
      kill: vi.fn(async () => die()),
      wait: () =>
        new Promise<Buffer>((_, reject) => {
          die = () => reject(new Error('killed'));
          const job = JSON.parse(
            fs.readFileSync(options.env[TRANSCRIBE_JOB_FILE_ENV], 'utf8'),
          ) as TranscribeWorkerJob;
          fs.writeFileSync(
            job.resultPath,
            JSON.stringify({ words: [{ start: 0, end: 200, text: ' half' }] }),
          );
        }),
    }));
    const outcome = transcribe(path.join(sessionDir, 'audio.wav'), 'en', 'en', undefined);
    await vi.waitFor(() => expect(mocks.run).toHaveBeenCalled());
    await vi.advanceTimersByTimeAsync(10 * 60_000 + 100);

    await expect(outcome).rejects.toEqual(
      new TranscriptCutShort([{ start: 0, end: 200, text: ' half' }]),
    );
  });

  it('should say that it took too long', async () => {
    mocks.run.mockImplementation(() => {
      let die!: () => void;
      return {
        on: () => 0,
        kill: vi.fn(async () => die()),
        wait: () =>
          new Promise<Buffer>((_, reject) => {
            die = () => reject(new Error('killed'));
          }),
      };
    });
    const result = transcribe(path.join(sessionDir, 'audio.wav'), 'en', 'en', undefined);
    const outcome = result.catch((error: Error) => error.message);
    await vi.waitFor(() => expect(mocks.run).toHaveBeenCalled());
    await vi.advanceTimersByTimeAsync(10 * 60_000 + 100);
    expect(await outcome).toBe('Transcription took too long.');
  });
});

describe('when the user skips the transcription', () => {
  let sessionDir: string;

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.toolsDir = fs.mkdtempSync(path.join(os.tmpdir(), 'transcribe-tools-'));
    sessionDir = fs.mkdtempSync(path.join(os.tmpdir(), 'transcribe-session-'));
    fs.writeFileSync(path.join(sessionDir, 'audio.wav'), Buffer.alloc(44));
  });

  afterEach(() => {
    fs.rmSync(sessionDir, { recursive: true, force: true });
    fs.rmSync(mocks.toolsDir, { recursive: true, force: true });
  });

  it('should kill the worker, wait for it to die, then say it was skipped', async () => {
    const order: string[] = [];
    let started!: () => void;
    const running = new Promise<void>(resolve => (started = resolve));
    let die!: () => void;
    mocks.run.mockImplementation(() => {
      started();
      return {
        on: () => 0,
        kill: vi.fn(async () => {
          order.push('kill');
          die();
          order.push('dead');
        }),
        wait: () =>
          new Promise<Buffer>((_, reject) => {
            die = () => reject(new Error('killed'));
          }),
      };
    });
    const controller = new AbortController();
    const outcome = transcribe(
      path.join(sessionDir, 'audio.wav'),
      'en',
      'en',
      undefined,
      controller.signal,
    ).catch((error: Error) => order.push(error.message));
    await running;
    controller.abort();
    await outcome;
    expect(order).toEqual(['kill', 'dead', 'Transcription was skipped.']);
  });
});

describe('when transcribing a recording', () => {
  let sessionDir: string;
  let wavPath: string;

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.toolsDir = fs.mkdtempSync(path.join(os.tmpdir(), 'transcribe-tools-'));
    sessionDir = fs.mkdtempSync(path.join(os.tmpdir(), 'transcribe-session-'));
    wavPath = path.join(sessionDir, 'audio.wav');
    fs.writeFileSync(wavPath, Buffer.alloc(44));
  });

  afterEach(() => {
    fs.rmSync(sessionDir, { recursive: true, force: true });
    fs.rmSync(mocks.toolsDir, { recursive: true, force: true });
  });

  describe('and the worker returns words', () => {
    let worker: FakeWorker;
    let words: Awaited<ReturnType<typeof transcribe>>;

    beforeEach(async () => {
      worker = fakeWorker({ result: { words: [{ start: 0, end: 200, text: ' hi' }] } });
      words = await transcribe(wavPath, 'en', 'en', 'Barrel_3');
    });

    it('should hand the job over as a file next to the audio, and remove it afterwards', () => {
      const jobFile = worker.env[TRANSCRIBE_JOB_FILE_ENV];
      expect(path.dirname(jobFile)).toBe(sessionDir);
      expect(worker.job).toEqual({
        modelPath: '/models/en.bin',
        vadModelPath: '/models/vad.bin',
        wavPath,
        resultPath: expect.stringContaining(sessionDir),
        language: 'en',
        prompt: 'Barrel_3',
      });
      expect(fs.existsSync(jobFile)).toBe(false);
    });

    it('should return the words from the result file and leave nothing behind', () => {
      expect(words).toEqual([{ start: 0, end: 200, text: ' hi' }]);
      expect(fs.existsSync(worker.job!.resultPath)).toBe(false);
    });
  });

  describe('and the worker prints the transcript on stdout instead', () => {
    it('should ignore it, so no transcript can reach the app log through a parsed line', async () => {
      fakeWorker({
        stdout: '{"type":"result","words":[{"start":0,"end":200,"text":" secret"}]}\n',
      });
      await expect(transcribe(wavPath, 'en', 'en', undefined)).rejects.toThrow(
        'The transcription worker exited without a result.',
      );
    });
  });

  describe('and the worker reports an error', () => {
    it('should surface its message', async () => {
      fakeWorker({ result: { error: 'model file is corrupt' }, exit: 'failed' });
      await expect(transcribe(wavPath, 'en', 'en', undefined)).rejects.toThrow(
        'model file is corrupt',
      );
    });
  });

  describe('and the worker dies before it can report (a native crash)', () => {
    it('should surface the end of what it wrote to stderr', async () => {
      fakeWorker({
        stderr: ['ggml_metal_init: loading', 'ggml_metal_init: error: failed', 'Abort trap: 6'],
        exit: 'clean',
      });
      await expect(transcribe(wavPath, 'en', 'en', undefined)).rejects.toThrow(
        'ggml_metal_init: loading\nggml_metal_init: error: failed\nAbort trap: 6',
      );
    });

    it('should say it exited without a result when stderr is empty too', async () => {
      fakeWorker({ exit: 'clean' });
      await expect(transcribe(wavPath, 'en', 'en', undefined)).rejects.toThrow(
        'The transcription worker exited without a result.',
      );
    });
  });

  describe('and the worker exits non-zero after writing the result', () => {
    it('should still take the result', async () => {
      fakeWorker({ result: { words: [] }, exit: 'failed' });
      await expect(transcribe(wavPath, 'en', 'en', undefined)).resolves.toEqual([]);
    });
  });

  describe('and the job and result files are left when the worker fails', () => {
    it('should remove both', async () => {
      const worker = fakeWorker({ result: { error: 'boom' }, exit: 'failed' });
      await expect(transcribe(wavPath, 'en', 'en', undefined)).rejects.toThrow();
      expect(fs.existsSync(worker.env[TRANSCRIBE_JOB_FILE_ENV])).toBe(false);
      expect(fs.existsSync(worker.job!.resultPath)).toBe(false);
    });
  });

  describe('and the worker was killed with a half-written result beside it', () => {
    it('should leave neither the result nor its part file behind', async () => {
      const worker = fakeWorker({ result: { words: [] } });
      const partials: string[] = [];
      mocks.run.mockImplementation(((pkg: string, bin: string, options: { env: EnvOf }) => {
        const job = JSON.parse(
          fs.readFileSync(options.env[TRANSCRIBE_JOB_FILE_ENV], 'utf8'),
        ) as TranscribeWorkerJob;
        partials.push(`${job.resultPath}.part`);
        fs.writeFileSync(`${job.resultPath}.part`, '{"words":');
        fs.writeFileSync(job.resultPath, '{"words":[]}');
        return { on: () => 0, kill: vi.fn(async () => {}), wait: async () => Buffer.alloc(0) };
      }) as never);

      await transcribe(wavPath, 'en', 'en', undefined);

      expect(worker.job).toBeNull();
      expect(partials).toHaveLength(1);
      expect(fs.readdirSync(sessionDir)).toEqual(['audio.wav']);
    });
  });

  describe('and the job and result files cannot be deleted afterwards', () => {
    it('should still return the words the worker found', async () => {
      fakeWorker({ result: { words: [{ start: 0, end: 200, text: ' hi' }] } });
      const rm = vi
        .spyOn(fsp, 'rm')
        .mockRejectedValue(Object.assign(new Error('EPERM'), { code: 'EPERM' }));
      try {
        await expect(transcribe(wavPath, 'en', 'en', undefined)).resolves.toEqual([
          { start: 0, end: 200, text: ' hi' },
        ]);
      } finally {
        rm.mockRestore();
      }
    });
  });

  describe('and speech-to-text is not installed', () => {
    it('should not start a worker', async () => {
      mocks.isInstalled.mockResolvedValueOnce(false);
      await expect(transcribe(wavPath, 'en', 'en', undefined)).rejects.toThrow(
        'Speech-to-text is not installed yet.',
      );
      expect(mocks.run).not.toHaveBeenCalled();
    });
  });
});
