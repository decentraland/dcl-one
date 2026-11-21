import { misc, recording as recordingPreload } from '#preload';
import type { AiRecorderState } from '/shared/types/ai';
import { MAX_RECORDING_MS, type RecordingToolsStatus } from '/shared/types/recording';

import { store } from '/@/modules/store';
import { actions as aiActions } from '/@/modules/store/ai';
import { actions as snackbarActions } from '/@/modules/store/snackbar';
import type { Severity } from '/@/modules/store/snackbar/types';
import { createGenericNotification } from '/@/modules/store/snackbar/utils';
import { actions as workspaceActions } from '/@/modules/store/workspace';
import { t } from '/@/modules/store/translation/utils';
import { playRecordingLimitChime } from './chime';
import { publishMicLevel } from './level';
import { startMicrophone, type Microphone } from './microphone';
import { onRecordedActionCount, setActiveRecordingSession, startHubDomRecorder } from './session';

/** The one owner of the editor's session recorder; everything it shows lives in `ai.recorder`. */

/** What the editor window lends the recorder for as long as it is open. */
export type RecorderHost = {
  projectPath: string | undefined;
  setInspectorRecording: (on: boolean) => Promise<void>;
  onFinished: () => void;
};

export type StopReason = 'user' | 'limit' | 'device';

/** The question asked before the first recording, with the speech model still to download. */
export type ConsentPrompt = { downloadMb: number };

type Active = {
  sessionId: string;
  microphone: Microphone | null;
  deviceLost: boolean;
  stopHubDom: () => void;
  stopCounting: () => void;
  stopToolsProgress: () => void;
  limitTimer: ReturnType<typeof setTimeout> | null;
};

const IDLE: AiRecorderState = {
  status: 'idle',
  startedAt: null,
  stoppedAt: null,
  micLabel: '',
  tools: null,
  actionCount: 0,
  endedWith: null,
};

let host: RecorderHost | null = null;
let active: Active | null = null;
let stoppingSessionId: string | null = null;
let consentPrompt: ConsentPrompt | null = null;
const consentListeners = new Set<() => void>();
let attempt = 0;

/**
 * Whether the start or stop that took `token` is still the current one.
 *
 * @returns false once an abort (or another start) has superseded it, leaving it to undo
 * whatever it did instead of carrying on.
 */
function isCurrentAttempt(token: number): boolean {
  return attempt === token;
}

/** Lends the recorder the open editor's project and inspector; `null` once it is gone. */
export function setRecorderHost(next: RecorderHost | null): void {
  host = next;
}

export function onConsentPrompt(listener: () => void): () => void {
  consentListeners.add(listener);
  return () => void consentListeners.delete(listener);
}

export function getConsentPrompt(): ConsentPrompt | null {
  return consentPrompt;
}

function setConsentPrompt(next: ConsentPrompt | null): void {
  consentPrompt = next;
  consentListeners.forEach(listener => listener());
}

function state(): AiRecorderState {
  return store.getState().ai.recorder;
}

function patch(next: Partial<AiRecorderState>): void {
  store.dispatch(aiActions.setRecorderState({ ...state(), ...next }));
}

function notify(severity: Severity, message: string, description?: string): void {
  store.dispatch(
    snackbarActions.pushSnackbar(
      createGenericNotification(severity, message, {
        ...(description !== undefined && { description }),
        ...(severity === 'error' && { duration: 0 }),
      }),
    ),
  );
}

function fail(error: unknown): void {
  const message = error instanceof Error ? error.message : String(error);
  notify('error', t('editor.recording.error', { message }));
}

async function applyInspectorRecording(on: boolean): Promise<void> {
  try {
    await host?.setInspectorRecording(on);
  } catch (error) {
    console.warn('[Recording] the inspector did not switch capture:', error);
  }
}

function stillTheSameEditor(projectPath: string | undefined): boolean {
  return host !== null && host.projectPath === projectPath;
}

function release(current: Active): void {
  if (current.limitTimer) clearTimeout(current.limitTimer);
  current.microphone?.stop();
  current.stopHubDom();
  current.stopCounting();
  current.stopToolsProgress();
  setActiveRecordingSession(null);
}

function watchToolsProgress(): () => void {
  const { cleanup } = recordingPreload.onToolsProgress((tools: RecordingToolsStatus) =>
    patch({ tools }),
  );
  return cleanup;
}

function microphoneLost(): void {
  if (active !== null && state().status !== 'recording') {
    active.deviceLost = true;
    return;
  }
  void stopRecording('device');
}

function sendSkipTranscription(sessionId: string): void {
  void recordingPreload.skipTranscription(sessionId).catch(() => undefined);
}

/** Opens a session: the microphone, the window's own events and the inspector's capture. */
export async function startRecording(): Promise<void> {
  const project = store.getState().editor.project;
  if (project === undefined || active !== null || state().status !== 'idle') return;
  const token = (attempt += 1);
  const locale = store.getState().translation.locale;
  patch({ status: 'starting' });
  let sessionId: string | null = null;
  let microphone: Microphone | null = null;
  try {
    const tools = await recordingPreload.toolsStatus(locale);
    if (!isCurrentAttempt(token) || host === null) return;
    patch({ tools });
    if (tools.state === 'missing') {
      void recordingPreload.installTools(locale).catch(() => undefined);
    }
    ({ sessionId } = await recordingPreload.start(project.path, project.title, locale));
    const id = sessionId;
    if (!isCurrentAttempt(token)) {
      void recordingPreload.abort(id).catch(() => undefined);
      return;
    }
    setActiveRecordingSession(id);
    active = {
      sessionId: id,
      microphone: null,
      deviceLost: false,
      stopHubDom: startHubDomRecorder(),
      stopCounting: onRecordedActionCount(count => patch({ actionCount: count })),
      stopToolsProgress: watchToolsProgress(),
      limitTimer: null,
    };
    microphone = await startMicrophone((pcm, firstSampleAt) => {
      recordingPreload.sendAudio(id, pcm, firstSampleAt);
      publishMicLevel(pcm);
    }, microphoneLost);
    if (!isCurrentAttempt(token)) {
      microphone.stop();
      return;
    }
    active.microphone = microphone;
    active.limitTimer = setTimeout(() => void stopRecording('limit'), MAX_RECORDING_MS);
    patch({ micLabel: microphone.label, startedAt: Date.now() });
    await applyInspectorRecording(true);
    if (!isCurrentAttempt(token)) return;
    patch({ status: 'recording' });
    if (active.deviceLost) void stopRecording('device');
  } catch (error) {
    if (!isCurrentAttempt(token)) {
      microphone?.stop();
      return;
    }
    const current = active;
    active = null;
    if (current) release(current);
    else microphone?.stop();
    if (sessionId !== null) void recordingPreload.abort(sessionId).catch(() => undefined);
    store.dispatch(aiActions.setRecorderState(IDLE));
    fail(error);
  }
}

/**
 * Ends the recording and sends what main made of it to the assistant: the transcript as the
 * message text, the timeline as its recording attachment.
 *
 * @param reason Why it ended: the limit and a lost microphone say so before main answers.
 * @param endedWith The button that ended it, which only changes how the wait is shown.
 */
export async function stopRecording(
  reason: StopReason = 'user',
  endedWith: 'stop' | 'send' = 'stop',
): Promise<void> {
  const current = active;
  if (current === null || state().status !== 'recording') return;
  const token = attempt;
  patch({ status: 'stopping', stoppedAt: Date.now(), endedWith });
  if (current.limitTimer) clearTimeout(current.limitTimer);
  current.microphone?.stop();
  current.stopHubDom();
  current.stopCounting();
  if (reason === 'limit') {
    void playRecordingLimitChime().catch(() => undefined);
    notify('info', t('editor.recording.limit_reached', { minutes: MAX_RECORDING_MS / 60_000 }));
  }
  if (reason === 'device') notify('info', t('editor.recording.microphone_lost'));
  await applyInspectorRecording(false);
  if (!isCurrentAttempt(token)) return;
  setActiveRecordingSession(null);
  active = null;
  stoppingSessionId = current.sessionId;
  const projectPath = host?.projectPath;
  const finishing = recordingPreload.stop(current.sessionId);
  try {
    const result = await finishing;
    if (!isCurrentAttempt(token) || !stillTheSameEditor(projectPath)) return;
    store.dispatch(
      aiActions.addRecording({
        durationMs: result.durationMs,
        actionCount: result.actionCount,
        dir: result.dir,
        context: result.context,
        ...(result.transcriptionError && { transcriptionError: result.transcriptionError }),
      }),
    );
    store.dispatch(
      aiActions.setDraftPrompt({ text: result.transcript ?? '', mode: 'append', send: true }),
    );
    host?.onFinished();
    if (result.transcriptionError && result.durationMs > 0) {
      notify('warning', t('editor.recording.no_transcript'), result.transcriptionError);
    }
  } catch (error) {
    if (isCurrentAttempt(token)) fail(error);
  } finally {
    if (stoppingSessionId === current.sessionId) stoppingSessionId = null;
    current.stopToolsProgress();
    if (isCurrentAttempt(token)) {
      patch({ status: 'idle', startedAt: null, stoppedAt: null, endedWith: null });
    }
  }
}

/** The recording's Send button: stop, then send it as soon as it is transcribed. */
export async function sendRecording(): Promise<void> {
  await stopRecording('user', 'send');
}

/** Throws the running recording away: like an abort, and main deletes what it captured. */
export function cancelRecording(): void {
  const current = active;
  if (current === null || state().status !== 'recording') return;
  attempt += 1;
  active = null;
  release(current);
  void applyInspectorRecording(false);
  void recordingPreload.cancel(current.sessionId).catch(fail);
  store.dispatch(aiActions.setRecorderState(IDLE));
}

/** Synchronous teardown for unmount, a project change or the assistant being turned off. */
export function abortRecording(): void {
  attempt += 1;
  const current = active;
  if (current !== null) {
    active = null;
    release(current);
    void applyInspectorRecording(false);
    void recordingPreload.abort(current.sessionId).catch(() => undefined);
  }
  if (stoppingSessionId !== null) sendSkipTranscription(stoppingSessionId);
  stoppingSessionId = null;
  setConsentPrompt(null);
  store.dispatch(aiActions.setRecorderState(IDLE));
}

/** Switches capture back on in an inspector iframe that came up (or reloaded) mid-session. */
export function reapplyInspectorCapture(): void {
  if (active === null || state().status === 'stopping') return;
  void applyInspectorRecording(true);
}

/** The Record button: stop a running recording, else start one once the creator has agreed. */
export async function pressRecord(): Promise<void> {
  const status = state().status;
  if (status === 'recording') {
    await stopRecording('user');
    return;
  }
  if (status !== 'idle') return;
  if (store.getState().workspace.settings.recordingConsentAcknowledged) {
    await startRecording();
    return;
  }
  const locale = store.getState().translation.locale;
  const tools = await recordingPreload
    .toolsStatus(locale)
    .catch(() => ({ state: 'missing' as const, downloadMb: 0 }));
  if (host === null) return;
  setConsentPrompt({ downloadMb: tools.state === 'missing' ? tools.downloadMb : 0 });
  void misc.focusWindow().catch(() => undefined);
}

/** Agreed: remember it for next time and start recording. */
export function acceptRecordingConsent(): void {
  setConsentPrompt(null);
  const settings = store.getState().workspace.settings;
  void store.dispatch(
    workspaceActions.updateSettings({ ...settings, recordingConsentAcknowledged: true }),
  );
  void startRecording();
}

export function dismissRecordingConsent(): void {
  setConsentPrompt(null);
}
