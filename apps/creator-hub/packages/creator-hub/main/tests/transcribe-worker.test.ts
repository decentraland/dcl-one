import fs from 'node:fs';
import fsp from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
  TRANSCRIBE_JOB_FILE_ENV,
  type TranscribeWorkerJob,
  type TranscribeWorkerResult,
} from '../../shared/types/recording';
import { wavHeader } from '../src/modules/recording/wav';

const whisper = vi.hoisted(() => ({
  segments: [
    { t0: 0, t1: 300, text: ' delete' },
    { t0: 300, t1: 600, text: ' this' },
  ],
  speech: [{ t0: 0, t1: 100 }],
  clips: [] as number[],
}));
vi.mock('@fugood/whisper.node', () => ({
  initWhisperVad: async () => ({
    detectSpeechData: async () => whisper.speech,
    release: async () => {},
  }),
  initWhisper: async () => ({
    transcribeData: (pcm: ArrayBuffer) => {
      whisper.clips.push(pcm.byteLength);
      return { promise: Promise.resolve({ segments: whisper.segments }) };
    },
    release: async () => {},
  }),
}));

const SAMPLE_RATE = 16_000;

function tone(ms: number): Buffer {
  const samples = (ms / 1000) * SAMPLE_RATE;
  const pcm = Buffer.alloc(samples * 2);
  for (let i = 0; i < samples; i++) pcm.writeInt16LE(Math.round(9000 * Math.sin(i / 3)), i * 2);
  return Buffer.concat([wavHeader(pcm.byteLength, SAMPLE_RATE), pcm]);
}

describe('when the transcription worker runs a job', () => {
  let dir: string;
  let resultPath: string;
  let printed: string[];

  beforeEach(async () => {
    vi.resetModules();
    dir = fs.mkdtempSync(path.join(os.tmpdir(), 'transcribe-worker-'));
    resultPath = path.join(dir, 'transcribe-result.json');
    const wavPath = path.join(dir, 'audio.wav');
    fs.writeFileSync(wavPath, tone(1000));
    const job: TranscribeWorkerJob = {
      modelPath: path.join(dir, 'model.bin'),
      vadModelPath: path.join(dir, 'vad.bin'),
      wavPath,
      resultPath,
      language: 'en',
      prompt: 'Barrel_3',
    };
    fs.writeFileSync(path.join(dir, 'job.json'), JSON.stringify(job));
    process.env[TRANSCRIBE_JOB_FILE_ENV] = path.join(dir, 'job.json');
    printed = [];
    vi.spyOn(process.stdout, 'write').mockImplementation((chunk: unknown) => {
      printed.push(String(chunk));
      return true;
    });
    await import('../src/transcribe-worker');
    await vi.waitFor(() => expect(fs.existsSync(resultPath)).toBe(true));
  });

  afterEach(() => {
    vi.restoreAllMocks();
    delete process.env[TRANSCRIBE_JOB_FILE_ENV];
    process.exitCode = 0;
    fs.rmSync(dir, { recursive: true, force: true });
  });

  it('should write the words it heard to the result file', () => {
    const result = JSON.parse(fs.readFileSync(resultPath, 'utf8')) as TranscribeWorkerResult;
    expect(result).toEqual({
      words: [
        { start: 0, end: 300, text: ' delete', segment: 0 },
        { start: 300, end: 600, text: ' this', segment: 0 },
      ],
    });
  });

  it('should print nothing at all on stdout, which the app log records', () => {
    expect(printed).toEqual([]);
  });
});

describe('when the user spoke in two stretches close together', () => {
  let dir: string;
  let resultPath: string;

  beforeEach(async () => {
    vi.resetModules();
    whisper.clips = [];
    whisper.speech = [
      { t0: 0, t1: 100 },
      { t0: 150, t1: 250 },
    ];
    dir = fs.mkdtempSync(path.join(os.tmpdir(), 'transcribe-worker-'));
    resultPath = path.join(dir, 'transcribe-result.json');
    const wavPath = path.join(dir, 'audio.wav');
    fs.writeFileSync(wavPath, tone(4000));
    const job: TranscribeWorkerJob = {
      modelPath: path.join(dir, 'model.bin'),
      vadModelPath: path.join(dir, 'vad.bin'),
      wavPath,
      resultPath,
      language: 'en',
    };
    fs.writeFileSync(path.join(dir, 'job.json'), JSON.stringify(job));
    process.env[TRANSCRIBE_JOB_FILE_ENV] = path.join(dir, 'job.json');
    await import('../src/transcribe-worker');
    await vi.waitFor(() => expect(fs.existsSync(resultPath)).toBe(true));
  });

  afterEach(() => {
    whisper.speech = [{ t0: 0, t1: 100 }];
    delete process.env[TRANSCRIBE_JOB_FILE_ENV];
    process.exitCode = 0;
    fs.rmSync(dir, { recursive: true, force: true });
  });

  it('should hand whisper one clip covering both, not one padded clip each', () => {
    expect(whisper.clips).toEqual([2500 * (SAMPLE_RATE / 1000) * 2]);
  });

  it('should still place the words on the recording clock', () => {
    const result = JSON.parse(fs.readFileSync(resultPath, 'utf8')) as TranscribeWorkerResult;
    expect(result).toEqual({
      words: [
        { start: 0, end: 300, text: ' delete', segment: 0 },
        { start: 300, end: 600, text: ' this', segment: 0 },
      ],
    });
  });
});

describe('when the result file cannot be replaced', () => {
  let dir: string;
  let resultPath: string;

  function startJob() {
    const wavPath = path.join(dir, 'audio.wav');
    fs.writeFileSync(wavPath, tone(1000));
    const job: TranscribeWorkerJob = {
      modelPath: path.join(dir, 'model.bin'),
      vadModelPath: path.join(dir, 'vad.bin'),
      wavPath,
      resultPath,
      language: 'en',
    };
    fs.writeFileSync(path.join(dir, 'job.json'), JSON.stringify(job));
    process.env[TRANSCRIBE_JOB_FILE_ENV] = path.join(dir, 'job.json');
    return import('../src/transcribe-worker');
  }

  beforeEach(() => {
    vi.resetModules();
    dir = fs.mkdtempSync(path.join(os.tmpdir(), 'transcribe-worker-'));
    resultPath = path.join(dir, 'transcribe-result.json');
  });

  afterEach(() => {
    vi.restoreAllMocks();
    delete process.env[TRANSCRIBE_JOB_FILE_ENV];
    process.exitCode = 0;
    fs.rmSync(dir, { recursive: true, force: true });
  });

  it('should carry on when a window could only be saved as progress', async () => {
    const rename = vi
      .spyOn(fsp, 'rename')
      .mockRejectedValueOnce(Object.assign(new Error('EPERM'), { code: 'EPERM' }));

    await startJob();
    await vi.waitFor(() => expect(fs.existsSync(resultPath)).toBe(true));

    expect(rename).toHaveBeenCalledTimes(2);
    expect(JSON.parse(fs.readFileSync(resultPath, 'utf8'))).toEqual({
      words: [
        { start: 0, end: 300, text: ' delete', segment: 0 },
        { start: 300, end: 600, text: ' this', segment: 0 },
      ],
    });
    expect(process.exitCode).toBe(0);
  });

  it('should fail the job when the result itself cannot be written', async () => {
    vi.spyOn(fsp, 'rename').mockRejectedValue(Object.assign(new Error('EPERM'), { code: 'EPERM' }));

    await startJob();
    await vi.waitFor(() => expect(process.exitCode).toBe(1));

    expect(fs.existsSync(resultPath)).toBe(false);
  });
});
