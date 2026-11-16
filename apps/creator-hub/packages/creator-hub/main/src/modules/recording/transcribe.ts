import fs from 'node:fs/promises';
import path from 'node:path';
import log from 'electron-log/main';

import {
  TRANSCRIBE_JOB_FILE_ENV,
  type TranscribeWorkerJob,
  type TranscribeWorkerMessage,
  type TranscriptWord,
} from '/shared/types/recording';

import { StreamError, run as runBin } from '../bin';
import { ensureWorkerPackage } from '../downloaded-tools';
import { getModelPath, getToolsDir, getVadModelPath, isInstalled, type ModelId } from './tools';

const WORKER_PKG = '@dcl-creator-hub/transcribe-worker';
const WORKER_BIN = 'transcribe-worker';
// Generous: a long session on an Intel Mac without Metal transcribes at a few times real time.
const TRANSCRIBE_TIMEOUT_MS = 10 * 60_000;
const STDERR_TAIL_LINES = 3;

export function parseWorkerOutput(stdout: Buffer | string): TranscribeWorkerMessage[] {
  return stdout
    .toString()
    .split(/\r?\n/)
    .flatMap(line => {
      if (!line.startsWith('{')) return [];
      try {
        return [JSON.parse(line) as TranscribeWorkerMessage];
      } catch {
        return [];
      }
    });
}

function tail(lines: string[]): string {
  return lines.join('').trim().split(/\r?\n/).slice(-STDERR_TAIL_LINES).join('\n');
}

export async function transcribe(
  wavPath: string,
  model: ModelId,
  language: string,
  prompt: string | undefined,
): Promise<TranscriptWord[]> {
  if (!(await isInstalled(model))) throw new Error('Speech-to-text is not installed yet.');
  const toolsDir = getToolsDir();
  await ensureWorkerPackage({
    toolsDir,
    pkg: WORKER_PKG,
    bin: WORKER_BIN,
    bundle: 'transcribe-worker.js',
  });
  const job: TranscribeWorkerJob = {
    modelPath: getModelPath(model),
    vadModelPath: getVadModelPath(),
    wavPath,
    language,
    prompt,
  };
  const jobFile = path.join(path.dirname(wavPath), 'transcribe-job.json');
  await fs.writeFile(jobFile, JSON.stringify(job));
  try {
    return await runWorker(toolsDir, jobFile);
  } finally {
    await fs.rm(jobFile, { force: true });
  }
}

async function runWorker(toolsDir: string, jobFile: string): Promise<TranscriptWord[]> {
  const child = runBin(WORKER_PKG, WORKER_BIN, {
    workspace: toolsDir,
    cwd: toolsDir,
    env: { [TRANSCRIBE_JOB_FILE_ENV]: jobFile },
  });
  // Collected as it arrives: a worker killed by a signal (whisper.cpp aborting) exits with a
  // null code, which `bin` reports as a clean exit and clears its buffers, so the reason would
  // otherwise be gone by the time the missing result is noticed.
  const stderrLines: string[] = [];
  child.on(/[\s\S]/, line => stderrLines.push(line ?? ''), { type: 'stderr' });

  let timer: NodeJS.Timeout | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      void child.kill();
      reject(new Error('Transcription took too long.'));
    }, TRANSCRIBE_TIMEOUT_MS);
  });

  // Read the outcome from the complete buffer, not the live stream: `bin` can drop the last
  // chunk on exit (see the optimizer's runWorker).
  let stdout: Buffer;
  let failure: string | null = null;
  try {
    stdout = await Promise.race([child.wait(), timeout]);
  } catch (error) {
    if (!(error instanceof StreamError)) throw error;
    stdout = error.stdout;
    failure = error.message;
  } finally {
    clearTimeout(timer);
  }

  const stderr = stderrLines.join('').trim();
  if (stderr) log.error(`[Recording] transcribe worker stderr:\n${stderr}`);
  const messages = parseWorkerOutput(stdout);
  const error = messages.find(m => m.type === 'error');
  if (error?.type === 'error') throw new Error(error.message);
  const result = messages.find(m => m.type === 'result');
  if (result?.type === 'result') return result.words;
  const reason =
    tail(stderrLines) || failure || 'The transcription worker exited without a result.';
  throw new Error(reason);
}
