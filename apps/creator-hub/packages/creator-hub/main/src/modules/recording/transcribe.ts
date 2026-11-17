import fs from 'node:fs/promises';
import path from 'node:path';
import log from 'electron-log/main';

import {
  TRANSCRIBE_JOB_FILE_ENV,
  type TranscribeWorkerJob,
  type TranscribeWorkerResult,
  type TranscriptWord,
} from '/shared/types/recording';

import { StreamError, run as runBin } from '../bin';
import { ensureWorkerPackage } from '../downloaded-tools';
import { getModelPath, getToolsDir, getVadModelPath, isInstalled, type ModelId } from './tools';

const WORKER_PKG = '@dcl-creator-hub/transcribe-worker';
const WORKER_BIN = 'transcribe-worker';
const TRANSCRIBE_TIMEOUT_MS = 10 * 60_000;
const STDERR_TAIL_LINES = 3;

function tail(lines: string[]): string {
  return lines.join('').trim().split(/\r?\n/).slice(-STDERR_TAIL_LINES).join('\n');
}

async function readResult(resultPath: string): Promise<TranscribeWorkerResult | null> {
  try {
    return JSON.parse(await fs.readFile(resultPath, 'utf8')) as TranscribeWorkerResult;
  } catch {
    return null;
  }
}

/** Transcribes a session's audio in a worker process; `signal` kills it and gives up. */
export async function transcribe(
  wavPath: string,
  model: ModelId,
  language: string,
  prompt: string | undefined,
  signal?: AbortSignal,
): Promise<TranscriptWord[]> {
  if (!(await isInstalled(model))) throw new Error('Speech-to-text is not installed yet.');
  const toolsDir = getToolsDir();
  await ensureWorkerPackage({
    toolsDir,
    pkg: WORKER_PKG,
    bin: WORKER_BIN,
    bundle: 'transcribe-worker.js',
  });
  const sessionDir = path.dirname(wavPath);
  const jobFile = path.join(sessionDir, 'transcribe-job.json');
  const resultPath = path.join(sessionDir, 'transcribe-result.json');
  const job: TranscribeWorkerJob = {
    modelPath: getModelPath(model),
    vadModelPath: getVadModelPath(),
    wavPath,
    resultPath,
    language,
    prompt,
  };
  await fs.writeFile(jobFile, JSON.stringify(job));
  try {
    return await runWorker(toolsDir, jobFile, resultPath, signal);
  } finally {
    await fs.rm(jobFile, { force: true });
    await fs.rm(resultPath, { force: true });
  }
}

async function runWorker(
  toolsDir: string,
  jobFile: string,
  resultPath: string,
  signal?: AbortSignal,
): Promise<TranscriptWord[]> {
  const child = runBin(WORKER_PKG, WORKER_BIN, {
    workspace: toolsDir,
    cwd: toolsDir,
    env: { [TRANSCRIBE_JOB_FILE_ENV]: jobFile },
  });
  const stderrLines: string[] = [];
  child.on(/[\s\S]/, line => stderrLines.push(line ?? ''), { type: 'stderr' });

  let killed: Promise<void> | undefined;
  let timedOut = false;
  const stop = () => {
    killed = child.kill().catch(() => undefined);
  };
  const timer = setTimeout(() => {
    timedOut = true;
    stop();
  }, TRANSCRIBE_TIMEOUT_MS);
  signal?.addEventListener('abort', stop, { once: true });
  if (signal?.aborted) stop();

  let failure: string | null = null;
  try {
    await child.wait();
  } catch (error) {
    if (!killed) {
      if (error instanceof StreamError) failure = error.message;
      else throw error;
    }
  } finally {
    clearTimeout(timer);
    signal?.removeEventListener('abort', stop);
  }
  if (killed) {
    await killed;
    throw new Error(timedOut ? 'Transcription took too long.' : 'Transcription was skipped.');
  }

  const stderr = stderrLines.join('').trim();
  if (stderr) log.error(`[Recording] transcribe worker stderr:\n${stderr}`);
  const result = await readResult(resultPath);
  if (result && 'error' in result) throw new Error(result.error);
  if (result) return result.words;
  throw new Error(
    tail(stderrLines) || failure || 'The transcription worker exited without a result.',
  );
}
