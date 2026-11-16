import { useCallback, useEffect, useRef, useState } from 'react';

import { recording as recordingPreload } from '#preload';
import { useDispatch, useSelector } from '#store';
import type { RecorderStatus } from '/shared/types/ai';
import { MAX_RECORDING_MS, type RecordingToolsStatus } from '/shared/types/recording';
import type { Project } from '/shared/types/projects';

import { actions as aiActions } from '/@/modules/store/ai';
import { actions as snackbarActions } from '/@/modules/store/snackbar';
import { createGenericNotification } from '/@/modules/store/snackbar/utils';
import { t } from '/@/modules/store/translation/utils';
import type { initRpc } from '/@/modules/rpc';
import { playRecordingLimitChime } from '/@/modules/recording/chime';
import { startMicrophone, type Microphone } from '/@/modules/recording/microphone';
import { setActiveRecordingSession, startHubDomRecorder } from '/@/modules/recording/session';

export type { RecorderStatus };

type Rpc = ReturnType<typeof initRpc>;

type Active = {
  sessionId: string;
  microphone: Microphone | null;
  stopHubDom: () => void;
  // Stops the recording at MAX_RECORDING_MS.
  limitTimer: ReturnType<typeof setTimeout> | null;
};

export type StopReason = 'user' | 'limit';

// The session recorder behind the editor's Record button: microphone + the Creator Hub's own
// clicks + the inspector's capture (switched on over the scene RPC), all streamed to main, which
// owns the session folder and transcribes on stop. The result is handed to the AI assistant as
// a pending recording, and what was said seeds the composer.
//
// Leaving the editor (closing the project) ends a running recording at once: the microphone is
// released and main keeps what was captured without transcribing it. A recording also ends on
// its own at MAX_RECORDING_MS, with a chime and a notice, so a forgotten one can't run for hours.
export function useSessionRecorder({
  enabled,
  project,
  rpcRef,
  inspectorReadyNonce,
  onFinished,
}: {
  enabled: boolean;
  project: Project | undefined;
  rpcRef: React.MutableRefObject<Rpc | undefined>;
  inspectorReadyNonce: number;
  onFinished: () => void;
}) {
  const dispatch = useDispatch();
  const locale = useSelector(state => state.translation.locale);
  const [status, setStatus] = useState<RecorderStatus>('idle');
  const [startedAt, setStartedAt] = useState<number | null>(null);
  const [micLabel, setMicLabel] = useState('');
  const [tools, setTools] = useState<RecordingToolsStatus | null>(null);
  const active = useRef<Active | null>(null);
  // Bumped by every start and every abort: a start that finds the counter moved on after one of
  // its awaits was aborted meanwhile and must undo what it did, not carry on.
  const attempt = useRef(0);
  const latest = useRef({
    onFinished,
    rpcRef,
    projectPath: project?.path,
    stop: (_reason: StopReason): Promise<void> => Promise.resolve(),
  });
  latest.current.onFinished = onFinished;
  latest.current.rpcRef = rpcRef;
  latest.current.projectPath = project?.path;

  // The button lives in the assistant panel and the detached window, which render it from the
  // store; publish every change there, and reset it when the editor goes away.
  useEffect(() => {
    dispatch(aiActions.setRecorderState({ status, startedAt, micLabel, tools }));
  }, [dispatch, status, startedAt, micLabel, tools]);
  useEffect(
    () => () => {
      dispatch(
        aiActions.setRecorderState({ status: 'idle', startedAt: null, micLabel: '', tools: null }),
      );
    },
    [dispatch],
  );

  const fail = useCallback(
    (error: unknown) => {
      const message = error instanceof Error ? error.message : String(error);
      dispatch(
        snackbarActions.pushSnackbar(
          createGenericNotification('error', t('editor.recording.error', { message }), {
            duration: 0,
          }),
        ),
      );
    },
    [dispatch],
  );

  const release = (current: Active) => {
    if (current.limitTimer) clearTimeout(current.limitTimer);
    current.microphone?.stop();
    current.stopHubDom();
    setActiveRecordingSession(null);
  };

  // Synchronous teardown for unmount / project change / assistant turned off. Also invalidates
  // a start still in flight, which then cleans up after itself (see `attempt`).
  const abort = useCallback(() => {
    attempt.current += 1;
    const current = active.current;
    if (current) {
      active.current = null;
      release(current);
      void latest.current.rpcRef.current?.scene.setRecording(false).catch(() => undefined);
      void recordingPreload.abort(current.sessionId).catch(() => undefined);
    }
    setStatus('idle');
    setStartedAt(null);
  }, []);

  const start = useCallback(async () => {
    if (!project || active.current || status !== 'idle') return;
    const token = (attempt.current += 1);
    const live = () => attempt.current === token;
    setStatus('starting');
    let sessionId: string | null = null;
    let microphone: Microphone | null = null;
    try {
      const toolsStatus = await recordingPreload.toolsStatus(locale);
      if (!live()) return;
      setTools(toolsStatus);
      if (toolsStatus.state === 'missing') {
        // Downloads in the background while the user records; stop waits for it if needed.
        void recordingPreload.installTools(locale).catch(() => undefined);
      }
      ({ sessionId } = await recordingPreload.start(project.path, project.title, locale));
      const id = sessionId;
      if (!live()) {
        // Aborted while main was opening the session: nothing here knew about it yet.
        void recordingPreload.abort(id).catch(() => undefined);
        return;
      }
      setActiveRecordingSession(id);
      const stopHubDom = startHubDomRecorder();
      active.current = { sessionId: id, microphone: null, stopHubDom, limitTimer: null };
      microphone = await startMicrophone((pcm, firstSampleAt) =>
        recordingPreload.sendAudio(id, pcm, firstSampleAt),
      );
      if (!live()) {
        // The abort released everything it knew of; the microphone opened after that.
        microphone.stop();
        return;
      }
      active.current.microphone = microphone;
      setMicLabel(microphone.label);
      await rpcRef.current?.scene.setRecording(true).catch(error => {
        console.warn('[Recording] the inspector did not start capturing:', error);
      });
      if (!live()) return;
      active.current.limitTimer = setTimeout(
        () => void latest.current.stop('limit'),
        MAX_RECORDING_MS,
      );
      setStartedAt(Date.now());
      setStatus('recording');
    } catch (error) {
      if (!live()) {
        microphone?.stop();
        return;
      }
      const current = active.current;
      active.current = null;
      if (current) release(current);
      else microphone?.stop();
      if (sessionId) void recordingPreload.abort(sessionId).catch(() => undefined);
      setStatus('idle');
      fail(error);
    }
  }, [project, status, locale, rpcRef, fail]);

  const stop = useCallback(
    async (reason: StopReason = 'user') => {
      const current = active.current;
      if (!current || status !== 'recording') return;
      setStatus('stopping');
      if (current.limitTimer) clearTimeout(current.limitTimer);
      current.microphone?.stop();
      current.stopHubDom();
      if (reason === 'limit') {
        void playRecordingLimitChime().catch(() => undefined);
        dispatch(
          snackbarActions.pushSnackbar(
            createGenericNotification(
              'info',
              t('editor.recording.limit_reached', { minutes: MAX_RECORDING_MS / 60_000 }),
            ),
          ),
        );
      }
      // Let the inspector flush what it buffered before the session closes: the relay must
      // still know the session until then.
      await rpcRef.current?.scene.setRecording(false).catch(() => undefined);
      setActiveRecordingSession(null);
      active.current = null;
      const projectPath = latest.current.projectPath;
      try {
        const result = await recordingPreload.stop(current.sessionId);
        // Transcription can outlast the editor: never hand it to another project's chat.
        if (latest.current.projectPath !== projectPath) return;
        dispatch(
          aiActions.setRecording({
            durationMs: result.durationMs,
            actionCount: result.actionCount,
            dir: result.dir,
            context: result.context,
            ...(result.transcriptionError && { transcriptionError: result.transcriptionError }),
          }),
        );
        if (result.transcript) dispatch(aiActions.setDraftPrompt(result.transcript));
        latest.current.onFinished();
        if (result.transcriptionError && result.durationMs > 0) {
          dispatch(
            snackbarActions.pushSnackbar(
              createGenericNotification('warning', t('editor.recording.no_transcript'), {
                description: result.transcriptionError,
              }),
            ),
          );
        }
      } catch (error) {
        fail(error);
      } finally {
        setStatus('idle');
        setStartedAt(null);
      }
    },
    [status, rpcRef, dispatch, fail],
  );
  latest.current.stop = stop;

  // A reloaded inspector iframe comes up with capture off; turn it back on.
  useEffect(() => {
    if (status === 'recording') {
      void rpcRef.current?.scene.setRecording(true).catch(() => undefined);
    }
  }, [inspectorReadyNonce]);

  // Download progress for the speech model, shown on the button while transcription waits.
  useEffect(() => {
    if (status === 'idle') return;
    const { cleanup } = recordingPreload.onToolsProgress(setTools);
    return cleanup;
  }, [status]);

  useEffect(() => {
    if (!enabled) abort();
  }, [enabled, abort]);

  useEffect(
    () => () => {
      abort();
      latest.current.projectPath = undefined;
      dispatch(aiActions.setRecording(null));
    },
    [project?.path, abort, dispatch],
  );

  return { status, startedAt, micLabel, tools, start, stop };
}
