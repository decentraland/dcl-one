import fs from 'node:fs/promises';
import type * as WhisperNodeModule from '@fugood/whisper.node';

import {
  RECORDING_SAMPLE_RATE,
  TRANSCRIBE_JOB_FILE_ENV,
  type TranscribeWorkerJob,
  type TranscribeWorkerResult,
  type TranscriptWord,
} from '/shared/types/recording';

import { speechRegions } from './modules/recording/vad';
import { WAV_HEADER_BYTES } from './modules/recording/wav';
import { toWindows, toWords, type Region } from './modules/recording/words';

const BYTES_PER_MS = (RECORDING_SAMPLE_RATE * 2) / 1000;
const VAD_CENTISECONDS_TO_MS = 10;

const VAD_OPTIONS = {
  threshold: 0.4,
  minSpeechDurationMs: 250,
  minSilenceDurationMs: 500,
  speechPadMs: 400,
};

const WHISPER_ON_CPU = { useGpu: false, nThreads: 4 };

type WhisperNode = typeof WhisperNodeModule;

async function detectSpeech(
  whisper: WhisperNode,
  job: TranscribeWorkerJob,
  pcm: ArrayBuffer,
): Promise<Region[]> {
  const loudness = () => toWindows(speechRegions(new Int16Array(pcm), RECORDING_SAMPLE_RATE));
  if (!job.vadModelPath) return loudness();
  try {
    const vad = await whisper.initWhisperVad({ filePath: job.vadModelPath, ...WHISPER_ON_CPU });
    try {
      const segments = await vad.detectSpeechData(pcm, VAD_OPTIONS);
      return toWindows(
        segments.map(s => ({
          start: s.t0 * VAD_CENTISECONDS_TO_MS,
          end: s.t1 * VAD_CENTISECONDS_TO_MS,
        })),
      );
    } finally {
      await vad.release();
    }
  } catch (error) {
    process.stderr.write(`speech detection failed, using the loudness regions: ${String(error)}\n`);
    return loudness();
  }
}

function wholeSamples(file: Buffer): ArrayBuffer {
  const dataBytes = Math.max(0, file.byteLength - WAV_HEADER_BYTES);
  return file.buffer.slice(
    file.byteOffset + WAV_HEADER_BYTES,
    file.byteOffset + WAV_HEADER_BYTES + dataBytes - (dataBytes % 2),
  ) as ArrayBuffer;
}

async function writeResult(
  job: TranscribeWorkerJob,
  result: TranscribeWorkerResult,
): Promise<void> {
  const partial = `${job.resultPath}.part`;
  await fs.writeFile(partial, JSON.stringify(result));
  await fs.rename(partial, job.resultPath);
}

async function transcribeJob(job: TranscribeWorkerJob): Promise<TranscriptWord[]> {
  const whisper = await import('@fugood/whisper.node');
  const pcm = wholeSamples(await fs.readFile(job.wavPath));
  const regions = await detectSpeech(whisper, job, pcm);
  if (regions.length === 0) return [];

  const context = await whisper.initWhisper({ filePath: job.modelPath, useGpu: true });
  try {
    const oneSegmentPerWord = { maxLen: 1, tokenTimestamps: true };
    const options = {
      language: job.language,
      temperature: 0,
      prompt: job.prompt,
      ...oneSegmentPerWord,
    };
    const words: TranscriptWord[] = [];
    for (const region of regions) {
      const from = Math.floor((region.start * BYTES_PER_MS) / 2) * 2;
      const to = Math.min(pcm.byteLength, Math.floor((region.end * BYTES_PER_MS) / 2) * 2);
      if (to <= from) continue;
      const { segments } = await context.transcribeData(pcm.slice(from, to), options).promise;
      words.push(...toWords(region, segments));
      await writeResult(job, { words }).catch(() => undefined);
    }
    return words;
  } finally {
    await context.release();
  }
}

function reason(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function main(): Promise<void> {
  const jobFile = process.env[TRANSCRIBE_JOB_FILE_ENV];
  if (!jobFile) throw new Error(`${TRANSCRIBE_JOB_FILE_ENV} is not set`);
  const job = JSON.parse(await fs.readFile(jobFile, 'utf8')) as TranscribeWorkerJob;
  let result: TranscribeWorkerResult;
  try {
    result = { words: await transcribeJob(job) };
  } catch (error) {
    result = { error: reason(error) };
    process.exitCode = 1;
  }
  await writeResult(job, result);
}

main().catch(error => {
  process.stderr.write(`${reason(error)}\n`);
  process.exitCode = 1;
});
