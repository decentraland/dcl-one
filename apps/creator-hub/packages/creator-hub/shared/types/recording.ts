// Types shared across main/preload/renderer for the session recorder: the Record button that
// captures the creator's speech plus a timestamped log of what they do in the editor, so the AI
// assistant can match "delete this one" to the entity they were pointing at. Preload cannot
// import from main, so the channel names and payload shapes live here.

/** Every {@link RecordedEvent} kind. The inspector mirrors this list in `lib/recorder/types.ts`. */
export const RECORDED_EVENT_KINDS = [
  'click',
  'dblclick',
  'contextmenu',
  'component',
  'dialog',
  'entity',
  'hover',
  'input',
  'key',
  'menu',
  'mode',
  'panel',
  'player',
  'scene-click',
  'scene-appear',
  'scene-remove',
  'scroll',
  'select',
  'setting',
  'viewport-hover',
] as const;

export type RecordedEventKind = (typeof RECORDED_EVENT_KINDS)[number];

/** One thing the user did, from the inspector or the Creator Hub window. */
export type RecordedEvent = {
  /** Epoch ms, on the wall clock every source shares. */
  t: number;
  kind: RecordedEventKind;
  /** One line that reads on its own. */
  text: string;
  /** What a `scene-appear` or `scene-remove` event is about, without the event's own wording. */
  label?: string;
  entity?: { id: number; name?: string };
  area?: string;
  x?: number;
  y?: number;
};

export const HOVER_EVENT_KINDS: ReadonlySet<RecordedEventKind> = new Set([
  'viewport-hover',
  'hover',
  'player',
]);

export const SCENE_EVENT_KINDS: ReadonlySet<RecordedEventKind> = new Set([
  'scene-appear',
  'scene-remove',
]);

/** Whether an event counts toward the "N actions" shown for a recording. */
export function isRecordedAction(kind: RecordedEventKind): boolean {
  return !HOVER_EVENT_KINDS.has(kind) && !SCENE_EVENT_KINDS.has(kind);
}

/** `m:ss`, rounded down, for every recording length the app shows. */
export function formatRecordingDuration(ms: number): string {
  const seconds = Math.floor(Math.max(0, ms) / 1000);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
}

// A word from the transcript, in ms from the start of the audio. `segment` is the stretch of
// speech it was transcribed in; a new stretch always starts a new utterance.
export type TranscriptWord = { start: number; end: number; text: string; segment?: number };

export type RecordingToolsStatus =
  | { state: 'ready' }
  | { state: 'missing'; downloadMb: number }
  | { state: 'installing'; message: string; percent?: number }
  | { state: 'unsupported'; reason: string };

export type RecordingStartResult = { sessionId: string };

export type RecordingResult = {
  sessionId: string;
  // The session folder (audio.wav, events.jsonl, transcript.json, SESSION.md).
  dir: string;
  durationMs: number;
  // What the user said, as plain text, to seed the composer.
  transcript: string;
  actionCount: number;
  // The condensed timeline handed to the assistant as editor context with the next message.
  context: string;
  /** Set when the audio could not be transcribed; the timeline still has every action. */
  transcriptionError?: string;
  /** Set when the user chose to send without waiting for the transcript. */
  transcriptionSkipped?: boolean;
};

// Fire-and-forget channels (ipcRenderer.send): audio chunks and event batches arrive many times
// a second, and the generic `handle` wrapper logs every argument to the app log — a privacy and
// size problem for raw audio and a log of the user's actions.
export const RECORDING_AUDIO_CHANNEL = 'recording.audio';
export const RECORDING_EVENTS_CHANNEL = 'recording.events';
export const RECORDING_TOOLS_PROGRESS_EVENT = 'recording.toolsProgress';

// The recorder captures 16 kHz mono 16-bit PCM: what Whisper consumes, and ~32 KB/s.
export const RECORDING_SAMPLE_RATE = 16_000;

// A recording stops itself at this length. Ten minutes is ~19 MB of WAV, a transcript of a
// couple of thousand words at most and well under the event cap, so stop() stays quick and the
// SESSION.md the assistant may read stays a few hundred lines. The renderer stops the session at
// the limit; main drops audio past it as a backstop.
export const MAX_RECORDING_MS = 10 * 60_000;

/** Days a finished session folder stays on disk once nothing touches it. */
export const RECORDING_RETENTION_DAYS = 30;

/** Names the JSON file holding the worker's {@link TranscribeWorkerJob}. */
export const TRANSCRIBE_JOB_FILE_ENV = 'TRANSCRIBE_JOB_FILE';

export type TranscribeWorkerJob = {
  modelPath: string;
  wavPath: string;
  /** Where the worker writes its {@link TranscribeWorkerResult}; main reads it and deletes it. */
  resultPath: string;
  language: string;
  /** Whisper's initial prompt: vocabulary to bias toward (entity names, editor terms). */
  prompt?: string;
  /** Silero VAD model; without it the worker falls back to its own loudness detection. */
  vadModelPath?: string;
};

/** The worker's outcome, written to the job's `resultPath`, never to the logged stdout. */
export type TranscribeWorkerResult = { words: TranscriptWord[] } | { error: string };
