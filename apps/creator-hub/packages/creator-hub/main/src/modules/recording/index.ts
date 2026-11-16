import crypto from 'node:crypto';
import fs from 'node:fs';
import fsp from 'node:fs/promises';
import path from 'node:path';
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
import { buildTimeline, renderContext, renderSessionMarkdown } from './timeline';
import { install, isInstalled, modelFor } from './tools';
import { transcribe } from './transcribe';
import { pcmPeak, wavHeader, WAV_HEADER_BYTES } from './wav';

// The session recorder's host side. The renderer streams microphone PCM and event batches here
// while the creator records; this module owns the session folder, so a project or app closed
// mid-recording still leaves a playable WAV and the actions captured so far. Writes are
// synchronous on purpose: chunks are small (~3 KB every 100 ms) and abort() must finish inside
// a quit handler that can't wait on stream flushes.

const KEEP_SESSIONS_PER_PROJECT = 20;
// A runaway inspector must not grow the log without bound: ~4 h of busy editing.
const MAX_EVENTS = 50_000;
// The renderer stops at MAX_RECORDING_MS; audio past it is dropped here in case it doesn't.
const MAX_AUDIO_BYTES = (RECORDING_SAMPLE_RATE * 2 * MAX_RECORDING_MS) / 1000;
// Below this 16-bit peak the microphone heard nothing worth transcribing (Whisper invents
// "Thank you." on silence).
const SILENCE_PEAK = 600;
// The inspector's own cap (lib/recorder/types.ts MAX_TEXT_LENGTH); the same here so a line is
// never cut differently on the two sides.
const MAX_TEXT = 200;
const MAX_KIND = 40;
const MAX_NAME = 120;
const MAX_AREA = 60;

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
  detach: () => void;
};

let session: Session | null = null;
// Folders whose audio.wav a transcription worker is still reading; discard() refuses them.
const transcribing = new Set<string>();

export function getRecordingsDir(): string {
  return path.join(app.getPath('userData'), 'recordings');
}

function projectFolder(projectPath: string): string {
  const slug = path
    .basename(projectPath)
    .replace(/[^\w.-]+/g, '-')
    .slice(0, 40);
  const hash = crypto.createHash('sha256').update(projectPath).digest('hex').slice(0, 8);
  return `${slug}-${hash}`;
}

// Readable and unique: two sessions started within the same second (an abort followed by a
// restart) must not share a folder, since the files are opened for truncation.
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

// macOS gates the microphone per app; the first request shows the system prompt. Elsewhere the
// OS doesn't ask the app, so this is a pass.
async function ensureMicrophoneAccess(): Promise<void> {
  if (process.platform !== 'darwin') return;
  // QA/e2e runs feed a WAV through Chromium's fake capture device; the system prompt would
  // block them and there is no real microphone to protect.
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
  const audioFd = fs.openSync(path.join(dir, 'audio.wav'), 'w');
  fs.writeSync(audioFd, wavHeader(0, RECORDING_SAMPLE_RATE));
  const eventsFd = fs.openSync(path.join(dir, 'events.jsonl'), 'w');

  const id = crypto.randomUUID();
  session = {
    id,
    dir,
    ownerId: owner.id,
    projectTitle,
    language,
    startedAt,
    audioStartedAt: null,
    audioFd,
    audioBytes: 0,
    peak: 0,
    eventsFd,
    events: [],
    ended: false,
    detach: () => {},
  };
  // The window going away (closed, reloaded, crashed) ends the recording on the spot. In-app
  // route changes are same-document navigations and don't count.
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

  // Get the speech tools downloading now (the consent dialog said so), so they are usually
  // ready by the time the user stops.
  const model = modelFor(language);
  if (!(await isInstalled(model))) {
    install(model, () => {}).catch(error =>
      log.warn('[Recording] speech-to-text install failed:', error),
    );
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

// A failed write (disk full, the folder gone) ends the session with what was captured, instead
// of throwing inside an ipcMain listener ten times a second.
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

// Every string in an event comes from the inspector, which reads it off the scene: a mesh or
// entity name in an imported glTF or a smart item is third-party text. One line each, no
// control characters, so none of it can forge a timeline line (a "SAID:" line in particular).
export function cleanText(text: string, max: number): string {
  return (
    text
      // eslint-disable-next-line no-control-regex
      .replace(/[\u0000-\u0008\u000e-\u001f\u007f-\u009f]+/g, '')
      .replace(/\s+/g, ' ')
      .trim()
      .slice(0, max)
  );
}

export function sanitizeEvent(raw: unknown): RecordedEvent | null {
  if (!raw || typeof raw !== 'object') return null;
  const e = raw as Record<string, unknown>;
  if (typeof e.t !== 'number' || typeof e.kind !== 'string' || typeof e.text !== 'string') {
    return null;
  }
  const event: RecordedEvent = {
    t: e.t,
    kind: cleanText(e.kind, MAX_KIND),
    text: cleanText(e.text, MAX_TEXT),
  };
  const entity = e.entity as Record<string, unknown> | undefined;
  if (entity && typeof entity.id === 'number') {
    const name = typeof entity.name === 'string' ? cleanText(entity.name, MAX_NAME) : '';
    event.entity = { id: entity.id, ...(name && { name }) };
  }
  if (typeof e.area === 'string') event.area = cleanText(e.area, MAX_AREA);
  if (typeof e.x === 'number') event.x = e.x;
  if (typeof e.y === 'number') event.y = e.y;
  return event;
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

// Close the files, leaving a valid WAV. Synchronous so a quit handler can call it.
function finalizeFiles(s: Session): number {
  s.ended = true;
  s.detach();
  const endedAt = Date.now();
  try {
    fs.writeSync(s.audioFd, wavHeader(s.audioBytes, RECORDING_SAMPLE_RATE), 0, WAV_HEADER_BYTES, 0);
  } finally {
    fs.closeSync(s.audioFd);
    fs.closeSync(s.eventsFd);
  }
  return endedAt;
}

// Distinct entity names the user worked with: Whisper's initial prompt, so "Barrel_3" comes out
// as written rather than "barrel three" or a homophone.
export function vocabularyPrompt(events: RecordedEvent[]): string | undefined {
  const counts = new Map<string, number>();
  for (const e of events) {
    const name = e.entity?.name;
    if (name) counts.set(name, (counts.get(name) ?? 0) + 1);
  }
  const names = [...counts.entries()].sort((a, b) => b[1] - a[1]).map(([name]) => name);
  let prompt = '';
  for (const name of names) {
    const next = prompt ? `${prompt}, ${name}` : name;
    if (next.length > 200) break;
    prompt = next;
  }
  return prompt || undefined;
}

function writeSessionFiles(
  s: Session,
  endedAt: number,
  words: TranscriptWord[],
  transcriptionError?: string,
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
      transcriptionError,
    }),
  );
  return {
    sessionId: s.id,
    dir: s.dir,
    durationMs: timeline.durationMs,
    transcript: timeline.utterances.map(u => u.text).join(' '),
    actionCount: timeline.actionCount,
    context: renderContext(timeline, s.startedAt, sessionFile),
    ...(transcriptionError && { transcriptionError }),
  };
}

// End without transcribing: the project or app is closing. Keeps what was captured.
export function abort(reason: string): void {
  const s = session;
  if (!s || s.ended) return;
  session = null;
  try {
    const endedAt = finalizeFiles(s);
    writeSessionFiles(s, endedAt, [], `skipped, the recording ended because ${reason}`);
    log.info(`[Recording] ended early (${reason}): ${s.dir}`);
  } catch (error) {
    log.error('[Recording] failed to close the session files:', error);
  }
}

export function abortIfOwner(senderId: number, sessionId: string): void {
  if (session && session.id === sessionId && session.ownerId === senderId) {
    abort('the project was closed');
  }
}

export async function stop(senderId: number, sessionId: string): Promise<RecordingResult> {
  const s = session;
  if (!s || s.id !== sessionId || s.ownerId !== senderId) {
    throw new Error('This recording is no longer active.');
  }
  session = null;
  const endedAt = finalizeFiles(s);

  const dir = path.resolve(s.dir);
  transcribing.add(dir);
  let words: TranscriptWord[] = [];
  let transcriptionError: string | undefined;
  try {
    ({ words, transcriptionError } = await transcribeSession(s));
  } finally {
    transcribing.delete(dir);
  }
  const result = writeSessionFiles(s, endedAt, words, transcriptionError);
  void prune(path.dirname(s.dir));
  log.info(
    `[Recording] stopped ${s.dir} (${Math.round(result.durationMs / 1000)}s, ${result.actionCount} actions, ${words.length} words)`,
  );
  return result;
}

async function prune(projectDir: string): Promise<void> {
  try {
    const entries = (await fsp.readdir(projectDir)).sort();
    const stale = entries.slice(0, Math.max(0, entries.length - KEEP_SESSIONS_PER_PROJECT));
    await Promise.all(
      stale.map(entry => fsp.rm(path.join(projectDir, entry), { recursive: true, force: true })),
    );
  } catch (error) {
    log.warn('[Recording] could not prune old sessions:', error);
  }
}

export function isRecording(): boolean {
  return session !== null;
}

// The user discarded a finished recording before sending it: nothing of it stays on disk. Only
// a session folder under the recordings dir qualifies, never the one still being written.
async function transcribeSession(
  s: Session,
): Promise<{ words: TranscriptWord[]; transcriptionError?: string }> {
  if (s.audioBytes === 0) return { words: [], transcriptionError: 'no audio was captured' };
  if (s.peak === 0) {
    // Exact digital zero is a muted or blocked input, not a quiet room (which still has noise).
    return {
      words: [],
      transcriptionError:
        'the microphone sent only silence. Check that it is not muted and that Creator Hub is allowed to use it in your system settings',
    };
  }
  if (s.peak < SILENCE_PEAK) return { words: [] };
  try {
    const model = modelFor(s.language);
    await install(model, () => {});
    // The worker finds the speech (Silero, or loudness when that fails) and transcribes each
    // stretch; nothing of the audio is read on this thread.
    const words = dropNonSpeech(
      await transcribe(
        path.join(s.dir, 'audio.wav'),
        model,
        s.language.slice(0, 2),
        vocabularyPrompt(s.events),
      ),
    );
    return { words };
  } catch (error) {
    log.error('[Recording] transcription failed:', error);
    return {
      words: [],
      transcriptionError: error instanceof Error ? error.message : String(error),
    };
  }
}

export async function discard(dir: string): Promise<void> {
  const root = getRecordingsDir();
  const target = path.resolve(dir);
  const relative = path.relative(root, target);
  const inside = relative !== '' && !relative.startsWith('..') && !path.isAbsolute(relative);
  if (!inside || relative.split(path.sep).length !== 2) {
    throw new Error('Not a session recording folder.');
  }
  if (session && path.resolve(session.dir) === target) {
    throw new Error('This recording is still running.');
  }
  if (transcribing.has(target)) {
    throw new Error('This recording is still being transcribed.');
  }
  await fsp.rm(target, { recursive: true, force: true });
  log.info(`[Recording] discarded ${target}`);
}
