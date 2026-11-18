import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { captureException } from '@sentry/electron/main';
import { app, systemPreferences, type WebContents } from 'electron';
import log from 'electron-log/main';

import {
  MAX_RECORDING_MS,
  RECORDING_SAMPLE_RATE,
  type RecordedEvent,
  type RecordingResult,
  type RecordingStartResult,
  type TranscriptWord,
} from '/shared/types/recording';

import { dropNonSpeech } from './nonspeech';
import { discard, getRecordingsDir, holdSession, pruneProject } from './retention';
import { cleanTitle, sanitizeEvent, vocabularyPrompt } from './sanitize';
import { buildTimeline, renderContext, renderSessionMarkdown } from './timeline';
import { install, isInstalled, modelFor } from './tools';
import { transcribe } from './transcribe';
import { pcmPeak, wavHeader, WAV_HEADER_BYTES } from './wav';

const MAX_EVENTS = 20_000;
const MAX_AUDIO_BYTES = (RECORDING_SAMPLE_RATE * 2 * MAX_RECORDING_MS) / 1000;
const SILENCE_PEAK = 600;
const AUDIO_FILE = 'audio.wav';
const SESSION_FILES = ['events.jsonl', 'transcript.json'];
const SKIPPED_NOTE = 'skipped, the recording was sent without waiting for it';
const RUNNING_REFUSAL = 'This recording is still running.';
const TRANSCRIBING_REFUSAL = 'This recording is still being transcribed.';

type Session = {
  id: string;
  dir: string;
  ownerId: number;
  projectTitle: string;
  language: string;
  startedAt: number;
  audioStartedAt: number | null;
  audioFd: number;
  audioBytes: number;
  peak: number;
  eventsFd: number;
  events: RecordedEvent[];
  ended: boolean;
  release: () => void;
  detach: () => void;
};

type Stopping = { ownerId: number; skip: AbortController; done: Promise<unknown> };

let session: Session | null = null;
const stopping = new Map<string, Stopping>();

export { discard };

function projectFolder(projectPath: string): string {
  const slug = path
    .basename(projectPath)
    .replace(/[^\w.-]+/g, '-')
    .slice(0, 40);
  const hash = crypto.createHash('sha256').update(projectPath).digest('hex').slice(0, 8);
  return `${slug}-${hash}`;
}

export function timestampFolder(
  date: Date,
  suffix = crypto.randomBytes(2).toString('hex'),
): string {
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}_${pad(
    date.getHours(),
  )}-${pad(date.getMinutes())}-${pad(date.getSeconds())}.${String(date.getMilliseconds()).padStart(
    3,
    '0',
  )}-${suffix}`;
}

async function ensureMicrophoneAccess(): Promise<void> {
  if (process.platform !== 'darwin') return;
  if (app.commandLine.hasSwitch('use-fake-device-for-media-stream')) return;
  const status = systemPreferences.getMediaAccessStatus('microphone');
  if (status === 'granted') return;
  if (status === 'not-determined' && (await systemPreferences.askForMediaAccess('microphone'))) {
    return;
  }
  throw new Error(
    'Creator Hub is not allowed to use the microphone. Turn it on in System Settings → Privacy & Security → Microphone, then try again.',
  );
}

export async function start(
  owner: WebContents,
  projectPath: string,
  projectTitle: string,
  language: string,
): Promise<RecordingStartResult> {
  await ensureMicrophoneAccess();
  if (session) abort('a new recording started');

  const startedAt = Date.now();
  const dir = path.join(
    getRecordingsDir(),
    projectFolder(projectPath),
    timestampFolder(new Date(startedAt)),
  );
  fs.mkdirSync(dir, { recursive: true });
  const audioFd = fs.openSync(path.join(dir, AUDIO_FILE), 'w');
  fs.writeSync(audioFd, wavHeader(0, RECORDING_SAMPLE_RATE));
  const eventsFd = fs.openSync(path.join(dir, 'events.jsonl'), 'w');

  const id = crypto.randomUUID();
  session = {
    id,
    dir,
    ownerId: owner.id,
    projectTitle: cleanTitle(projectTitle),
    language,
    startedAt,
    audioStartedAt: null,
    audioFd,
    audioBytes: 0,
    peak: 0,
    eventsFd,
    events: [],
    ended: false,
    release: holdSession(dir, RUNNING_REFUSAL),
    detach: () => {},
  };
  const onGone = () => {
    if (session?.id === id) abort('the window closed');
  };
  const onNavigate = (details: { isMainFrame?: boolean; isSameDocument?: boolean }) => {
    if (details.isMainFrame && !details.isSameDocument) onGone();
  };
  owner.once('destroyed', onGone);
  owner.once('render-process-gone', onGone);
  owner.on('did-start-navigation', onNavigate);
  session.detach = () => {
    if (owner.isDestroyed()) return;
    owner.off('destroyed', onGone);
    owner.off('render-process-gone', onGone);
    owner.off('did-start-navigation', onNavigate);
  };

  const model = modelFor(language);
  if (!(await isInstalled(model))) {
    install(model, () => {}).catch(error => {
      log.warn('[Recording] speech-to-text install failed:', error);
      captureException(error, { tags: { source: 'recording' } });
    });
  }
  log.info(`[Recording] started ${dir}`);
  return { sessionId: id };
}

function active(sessionId: string, senderId: number): Session | null {
  if (!session || session.ended || session.id !== sessionId || session.ownerId !== senderId) {
    return null;
  }
  return session;
}

function writeOrAbort(s: Session, write: () => void): boolean {
  try {
    write();
    return true;
  } catch (error) {
    log.error('[Recording] could not write to the session folder:', error);
    if (session === s) abort('the recording could not be written to disk');
    return false;
  }
}

export function appendAudio(
  senderId: number,
  sessionId: string,
  pcm: Uint8Array,
  firstSampleAt?: number,
): void {
  const s = active(sessionId, senderId);
  if (!s || !(pcm instanceof Uint8Array) || pcm.byteLength % 2 !== 0) return;
  const room = MAX_AUDIO_BYTES - s.audioBytes;
  if (room <= 0) return;
  const chunk = pcm.byteLength > room ? pcm.subarray(0, room - (room % 2)) : pcm;
  if (s.audioStartedAt === null) {
    s.audioStartedAt = typeof firstSampleAt === 'number' ? firstSampleAt : Date.now();
  }
  if (!writeOrAbort(s, () => fs.writeSync(s.audioFd, chunk))) return;
  s.audioBytes += chunk.byteLength;
  s.peak = Math.max(s.peak, pcmPeak(chunk));
}

export function appendEvents(senderId: number, sessionId: string, events: unknown): void {
  const s = active(sessionId, senderId);
  if (!s || !Array.isArray(events)) return;
  for (const raw of events) {
    if (s.events.length >= MAX_EVENTS) return;
    const event = sanitizeEvent(raw);
    if (!event) continue;
    if (!writeOrAbort(s, () => fs.writeSync(s.eventsFd, `${JSON.stringify(event)}\n`))) return;
    s.events.push(event);
  }
}

function close(fd: number): void {
  try {
    fs.closeSync(fd);
  } catch (error) {
    log.warn('[Recording] could not close a session file:', error);
  }
}

function finalizeFiles(s: Session): number {
  s.ended = true;
  s.detach();
  const endedAt = Date.now();
  try {
    fs.writeSync(s.audioFd, wavHeader(s.audioBytes, RECORDING_SAMPLE_RATE), 0, WAV_HEADER_BYTES, 0);
  } finally {
    close(s.audioFd);
    close(s.eventsFd);
    s.release();
  }
  return endedAt;
}

function writeSessionFiles(
  s: Session,
  endedAt: number,
  words: TranscriptWord[],
  transcription: { error?: string; skipped?: boolean } = {},
): RecordingResult {
  const timeline = buildTimeline({
    startedAt: s.startedAt,
    endedAt,
    audioStartedAt: s.audioStartedAt,
    words,
    events: s.events,
  });
  const sessionFile = path.join(s.dir, 'SESSION.md');
  fs.writeFileSync(
    path.join(s.dir, 'transcript.json'),
    JSON.stringify({ audioStartedAt: s.audioStartedAt, words }, null, 2),
  );
  fs.writeFileSync(
    sessionFile,
    renderSessionMarkdown(timeline, {
      projectTitle: s.projectTitle,
      startedAt: s.startedAt,
      files: SESSION_FILES.filter(file => fs.existsSync(path.join(s.dir, file))),
      transcriptionNote: transcription.skipped ? SKIPPED_NOTE : transcription.error,
    }),
  );
  return {
    sessionId: s.id,
    dir: s.dir,
    durationMs: timeline.durationMs,
    transcript: timeline.utterances.map(u => u.text).join(' '),
    actionCount: timeline.actionCount,
    context: renderContext(timeline, s.startedAt, sessionFile),
    ...(transcription.error &&
      !transcription.skipped && { transcriptionError: transcription.error }),
    ...(transcription.skipped && { transcriptionSkipped: true }),
  };
}

export function abort(reason: string): void {
  const s = session;
  if (!s || s.ended) return;
  session = null;
  try {
    const endedAt = finalizeFiles(s);
    writeSessionFiles(s, endedAt, [], {
      error: `skipped, the recording ended because ${reason}`,
    });
    log.info(`[Recording] ended early (${reason}): ${s.dir}`);
  } catch (error) {
    log.error('[Recording] failed to close the session files:', error);
  } finally {
    removeAudio(s.dir);
  }
}

export function abortIfOwner(senderId: number, sessionId: string): void {
  if (session && session.id === sessionId && session.ownerId === senderId) {
    abort('the project was closed');
  }
}

export async function cancel(senderId: number, sessionId: string): Promise<void> {
  const s = session;
  if (!s || s.id !== sessionId || s.ownerId !== senderId) return;
  abort('the user discarded it');
  await discard(s.dir);
}

/** Gives up on the transcript of the session being stopped, so stop() answers right away. */
export async function skipTranscription(senderId: number, sessionId: string): Promise<void> {
  const pending = stopping.get(sessionId);
  if (!pending || pending.ownerId !== senderId) return;
  log.info('[Recording] sending without the transcript');
  pending.skip.abort();
}

/** Gives up on every transcript still running at quit, before the workers are killed. */
export function abortTranscription(): void {
  for (const pending of stopping.values()) {
    log.info('[Recording] giving up on the transcript: the app is closing');
    pending.skip.abort();
  }
}

/** Waits out the stops still writing their session files, up to `timeoutMs`. */
export async function waitForPendingStops(timeoutMs: number): Promise<void> {
  const pending = [...stopping.values()].map(entry => entry.done.catch(() => undefined));
  if (pending.length === 0) return;
  let timer: NodeJS.Timeout | undefined;
  const bound = new Promise<void>(resolve => {
    timer = setTimeout(resolve, timeoutMs);
  });
  try {
    await Promise.race([Promise.all(pending), bound]);
  } finally {
    clearTimeout(timer);
  }
}

export function stop(senderId: number, sessionId: string): Promise<RecordingResult> {
  const s = session;
  if (!s || s.id !== sessionId || s.ownerId !== senderId) {
    return Promise.reject(new Error('This recording is no longer active.'));
  }
  session = null;
  const endedAt = finalizeFiles(s);

  const skip = new AbortController();
  const entry: Stopping = { ownerId: s.ownerId, skip, done: Promise.resolve() };
  stopping.set(s.id, entry);
  const done = runStop(s, endedAt, skip.signal).finally(() => {
    if (stopping.get(s.id) === entry) stopping.delete(s.id);
  });
  entry.done = done;
  return done;
}

async function runStop(s: Session, endedAt: number, signal: AbortSignal): Promise<RecordingResult> {
  const release = holdSession(s.dir, TRANSCRIBING_REFUSAL);
  let words: TranscriptWord[] = [];
  let transcriptionError: string | undefined;
  let skipped = false;
  try {
    ({ words, transcriptionError, skipped = false } = await transcribeSession(s, signal));
    const result = writeSessionFiles(s, endedAt, words, { error: transcriptionError, skipped });
    void pruneProject(path.dirname(s.dir));
    log.info(
      `[Recording] stopped ${s.dir} (${Math.round(result.durationMs / 1000)}s, ${result.actionCount} actions, ${words.length} words)`,
    );
    return result;
  } finally {
    release();
    removeAudio(s.dir);
  }
}

function waitUnlessSkipped<T>(task: Promise<T>, signal: AbortSignal): Promise<T | undefined> {
  if (signal.aborted) {
    void task.catch(() => undefined);
    return Promise.resolve(undefined);
  }
  return new Promise((resolve, reject) => {
    const giveUp = () => resolve(undefined);
    signal.addEventListener('abort', giveUp, { once: true });
    void task.then(resolve, reject).finally(() => signal.removeEventListener('abort', giveUp));
  });
}

function removeAudio(dir: string): void {
  try {
    fs.rmSync(path.join(dir, AUDIO_FILE), { force: true });
  } catch (error) {
    log.warn('[Recording] could not delete the audio:', error);
  }
}

async function transcribeSession(
  s: Session,
  signal: AbortSignal,
): Promise<{ words: TranscriptWord[]; transcriptionError?: string; skipped?: boolean }> {
  if (s.audioBytes === 0) return { words: [], transcriptionError: 'no audio was captured' };
  if (s.peak === 0) {
    return {
      words: [],
      transcriptionError:
        'the microphone sent only silence. Check that it is not muted and that Creator Hub is allowed to use it in your system settings',
    };
  }
  if (s.peak < SILENCE_PEAK) return { words: [] };
  try {
    const model = modelFor(s.language);
    await waitUnlessSkipped(
      install(model, () => {}),
      signal,
    );
    if (signal.aborted) return { words: [], skipped: true };
    const words = dropNonSpeech(
      await transcribe(
        path.join(s.dir, AUDIO_FILE),
        model,
        s.language.slice(0, 2),
        vocabularyPrompt(s.events),
        signal,
      ),
    );
    return { words };
  } catch (error) {
    if (signal.aborted) return { words: [], skipped: true };
    log.error('[Recording] transcription failed:', error);
    captureException(error, { tags: { source: 'recording' } });
    return {
      words: [],
      transcriptionError: error instanceof Error ? error.message : String(error),
    };
  }
}
