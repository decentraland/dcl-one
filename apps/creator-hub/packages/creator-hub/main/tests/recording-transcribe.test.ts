import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { TRANSCRIBE_JOB_FILE_ENV } from '../../shared/types/recording';

const mocks = vi.hoisted(() => ({
  run: vi.fn(),
  isInstalled: vi.fn(async () => true),
  toolsDir: '',
  // bin.ts pulls in Electron paths at import time, so its error class is stood in for here.
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

type FakeWorker = {
  env: Record<string, string>;
  jobFileDuringRun: string | null;
  stderr: (line: string) => void;
};

// A worker whose outcome the test picks: what it printed on stdout, how it exited, and what it
// wrote to stderr while running.
function fakeWorker(outcome: {
  stdout?: string;
  stderr?: string[];
  exit?: 'clean' | 'failed';
}): FakeWorker {
  const worker: FakeWorker = { env: {}, jobFileDuringRun: null, stderr: () => {} };
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
          worker.jobFileDuringRun = fs.existsSync(jobFile)
            ? fs.readFileSync(jobFile, 'utf8')
            : null;
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
      worker = fakeWorker({
        stdout:
          '{"type":"progress","percent":50}\n{"type":"result","words":[{"start":0,"end":200,"text":" hi"}]}\n',
      });
      words = await transcribe(wavPath, 'en', 'en', 'Barrel_3');
    });

    it('should hand the job over as a file next to the audio, and remove it afterwards', () => {
      const jobFile = worker.env[TRANSCRIBE_JOB_FILE_ENV];
      expect(path.dirname(jobFile)).toBe(sessionDir);
      expect(JSON.parse(worker.jobFileDuringRun!)).toEqual({
        modelPath: '/models/en.bin',
        vadModelPath: '/models/vad.bin',
        wavPath,
        language: 'en',
        prompt: 'Barrel_3',
      });
      expect(fs.existsSync(jobFile)).toBe(false);
    });

    it('should return the words', () => {
      expect(words).toEqual([{ start: 0, end: 200, text: ' hi' }]);
    });
  });

  describe('and the worker reports an error', () => {
    it('should surface its message', async () => {
      fakeWorker({
        stdout: '{"type":"error","message":"model file is corrupt"}\n',
        exit: 'failed',
      });
      await expect(transcribe(wavPath, 'en', 'en', undefined)).rejects.toThrow(
        'model file is corrupt',
      );
    });
  });

  describe('and the worker dies before it can report (a native crash)', () => {
    it('should surface the end of what it wrote to stderr', async () => {
      fakeWorker({
        stdout: '{"type":"progress","percent":10}\n',
        stderr: ['ggml_metal_init: loading', 'ggml_metal_init: error: failed', 'Abort trap: 6'],
        exit: 'clean',
      });
      await expect(transcribe(wavPath, 'en', 'en', undefined)).rejects.toThrow(
        'ggml_metal_init: loading\nggml_metal_init: error: failed\nAbort trap: 6',
      );
    });

    it('should say it exited without a result when stderr is empty too', async () => {
      fakeWorker({ stdout: '', exit: 'clean' });
      await expect(transcribe(wavPath, 'en', 'en', undefined)).rejects.toThrow(
        'The transcription worker exited without a result.',
      );
    });
  });

  describe('and the worker exits non-zero after printing the result', () => {
    it('should still take the result from the complete output', async () => {
      fakeWorker({ stdout: '{"type":"result","words":[]}\n', exit: 'failed' });
      await expect(transcribe(wavPath, 'en', 'en', undefined)).resolves.toEqual([]);
    });
  });

  describe('and the job file is removed when the worker fails', () => {
    it('should leave nothing behind', async () => {
      const worker = fakeWorker({ stdout: '', exit: 'failed' });
      await expect(transcribe(wavPath, 'en', 'en', undefined)).rejects.toThrow();
      expect(fs.existsSync(worker.env[TRANSCRIBE_JOB_FILE_ENV])).toBe(false);
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
