import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { AppSettings } from '/shared/types/settings';
import type { Project } from '/shared/types/projects';
import { MAX_RECORDING_MS } from '/shared/types/recording';

import type * as TranslationUtils from '/@/modules/store/translation/utils';

vi.mock('/@/modules/store', async () => {
  const { createTestStore } = await import('../../../tests/utils/testStore');
  return { store: createTestStore() };
});
vi.mock('/@/modules/store/translation/utils', async importOriginal => ({
  ...(await importOriginal<typeof TranslationUtils>()),
  t: (key: string) => key,
}));

const mic = vi.hoisted(() => ({ startMicrophone: vi.fn() }));
vi.mock('./microphone', () => mic);
const session = vi.hoisted(() => ({
  setActiveRecordingSession: vi.fn(),
  onRecordedActionCount: vi.fn(() => () => {}),
  startHubDomRecorder: vi.fn(() => vi.fn()),
}));
vi.mock('./session', () => session);
const chime = vi.hoisted(() => ({ playRecordingLimitChime: vi.fn(async () => {}) }));
vi.mock('./chime', () => chime);
vi.mock('./level', () => ({ publishMicLevel: vi.fn() }));

const { misc, recording } = await import('#preload');
const { store } = await import('/@/modules/store');
const { actions: aiActions } = await import('/@/modules/store/ai');
const { actions: workspaceActions } = await import('/@/modules/store/workspace');
const {
  abortRecording,
  acceptRecordingConsent,
  cancelRecording,
  dismissRecordingConsent,
  getConsentPrompt,
  pressRecord,
  reapplyInspectorCapture,
  setRecorderHost,
  skipTranscription,
  startRecording,
  stopRecording,
} = await import('./controller');

type Deferred<T> = {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (e: unknown) => void;
};
function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const PROJECT = { id: 'p1', path: '/scenes/demo', title: 'Demo' } as Project;

const recorderState = () => store.getState().ai.recorder;
const notifications = () => store.getState().snackbar.notifications;
const settle = () => new Promise<void>(resolve => setTimeout(resolve, 0));

describe('the session recorder', () => {
  let microphone: { label: string; stop: ReturnType<typeof vi.fn> };
  let endMicrophone: () => void;
  let setInspectorRecording: ReturnType<typeof vi.fn>;
  let onFinished: ReturnType<typeof vi.fn>;

  function acknowledgeConsent(acknowledged: boolean) {
    store.dispatch(
      workspaceActions.updateSettings.fulfilled(undefined as never, 'rid', {
        ...store.getState().workspace.settings,
        recordingConsentAcknowledged: acknowledged,
      } as AppSettings),
    );
  }

  beforeEach(() => {
    store.dispatch(
      workspaceActions.createProject.fulfilled(PROJECT, 'request-id', undefined as never),
    );
    acknowledgeConsent(true);
    microphone = { label: 'Built-in mic', stop: vi.fn() };
    mic.startMicrophone.mockReset().mockImplementation(async (_chunk, ended: () => void) => {
      endMicrophone = ended;
      return microphone;
    });
    session.setActiveRecordingSession.mockReset();
    session.startHubDomRecorder.mockReset().mockReturnValue(vi.fn());
    session.onRecordedActionCount.mockReset().mockReturnValue(() => {});
    chime.playRecordingLimitChime.mockClear();
    setInspectorRecording = vi.fn(async () => {});
    onFinished = vi.fn();
    setRecorderHost({ projectPath: PROJECT.path, setInspectorRecording, onFinished });
    vi.mocked(misc.focusWindow).mockResolvedValue(undefined);
    vi.mocked(recording.toolsStatus).mockResolvedValue({ state: 'ready' });
    vi.mocked(recording.installTools).mockResolvedValue(undefined);
    vi.mocked(recording.start).mockResolvedValue({ sessionId: 's1' });
    vi.mocked(recording.abort).mockResolvedValue(undefined);
    vi.mocked(recording.cancel).mockReset().mockResolvedValue(undefined);
    vi.mocked(recording.skipTranscription).mockReset().mockResolvedValue(undefined);
    vi.mocked(recording.onToolsProgress).mockReturnValue({ cleanup: vi.fn() });
    vi.mocked(recording.stop).mockResolvedValue({
      sessionId: 's1',
      dir: '/recordings/p/s1',
      durationMs: 1000,
      transcript: 'hello',
      actionCount: 2,
      context: '[Session recording]',
    });
  });

  afterEach(() => {
    abortRecording();
    dismissRecordingConsent();
    setRecorderHost(null);
    store.dispatch(aiActions.setRecording(null));
    store.dispatch(aiActions.consumeDraftPrompt());
    vi.useRealTimers();
  });

  describe('when the user starts and stops a recording', () => {
    it('should record, then hand the result to the assistant', async () => {
      await startRecording();
      expect(recorderState().status).toBe('recording');
      expect(recorderState().micLabel).toBe('Built-in mic');
      expect(setInspectorRecording).toHaveBeenCalledWith(true);

      await stopRecording();
      expect(microphone.stop).toHaveBeenCalled();
      expect(recording.stop).toHaveBeenCalledWith('s1');
      expect(recorderState().status).toBe('idle');
      expect(store.getState().ai.recording).toEqual({
        dir: '/recordings/p/s1',
        durationMs: 1000,
        actionCount: 2,
        context: '[Session recording]',
      });
      expect(store.getState().ai.draftPrompt?.text).toBe('hello');
      expect(onFinished).toHaveBeenCalled();
      expect(chime.playRecordingLimitChime).not.toHaveBeenCalled();
    });

    it('should keep the relay on the session until the inspector has flushed', async () => {
      const flush = deferred<void>();
      await startRecording();
      session.setActiveRecordingSession.mockClear();
      setInspectorRecording.mockReturnValueOnce(flush.promise);

      const stopped = stopRecording();
      await settle();
      expect(session.setActiveRecordingSession).not.toHaveBeenCalled();

      flush.resolve();
      await stopped;
      expect(session.setActiveRecordingSession).toHaveBeenCalledWith(null);
    });
  });

  describe('when the editor goes away while main is still checking the tools', () => {
    it('should never open a session', async () => {
      const checking = deferred<{ state: 'ready' }>();
      vi.mocked(recording.toolsStatus).mockReturnValue(checking.promise);

      const started = startRecording();
      abortRecording();
      checking.resolve({ state: 'ready' });
      await started;

      expect(recording.start).not.toHaveBeenCalled();
      expect(recorderState().status).toBe('idle');
    });
  });

  describe('when the editor goes away while main is still opening the session', () => {
    it('should abort that session and never open the microphone', async () => {
      const opening = deferred<{ sessionId: string }>();
      vi.mocked(recording.start).mockReturnValue(opening.promise);

      const started = startRecording();
      await settle();
      expect(recording.start).toHaveBeenCalled();
      abortRecording();
      opening.resolve({ sessionId: 's1' });
      await started;

      expect(mic.startMicrophone).not.toHaveBeenCalled();
      expect(recording.abort).toHaveBeenCalledWith('s1');
      expect(recorderState().status).toBe('idle');
    });
  });

  describe('when the user cancels the running recording', () => {
    it('should release the microphone and have main delete what was captured', async () => {
      await startRecording();

      cancelRecording();

      expect(microphone.stop).toHaveBeenCalled();
      expect(setInspectorRecording).toHaveBeenCalledWith(false);
      expect(recording.cancel).toHaveBeenCalledWith('s1');
      expect(recording.stop).not.toHaveBeenCalled();
      expect(recorderState().status).toBe('idle');
      expect(onFinished).not.toHaveBeenCalled();
    });

    it('should do nothing while no recording runs', () => {
      cancelRecording();
      expect(recording.cancel).not.toHaveBeenCalled();
    });
  });

  describe('when the editor goes away while the microphone is opening', () => {
    it('should release the microphone as soon as it opens', async () => {
      const opening = deferred<typeof microphone>();
      mic.startMicrophone.mockReturnValue(opening.promise);

      const started = startRecording();
      await settle();
      abortRecording();
      expect(recording.abort).toHaveBeenCalledWith('s1');

      opening.resolve(microphone);
      await started;

      expect(microphone.stop).toHaveBeenCalled();
      expect(recorderState().status).toBe('idle');
      expect(setInspectorRecording).not.toHaveBeenCalledWith(true);
    });
  });

  describe('when the editor goes away while the inspector is switching capture on', () => {
    it('should stay idle, with the microphone released and the session aborted', async () => {
      const switching = deferred<void>();
      setInspectorRecording.mockImplementation((on: boolean) =>
        on ? switching.promise : Promise.resolve(),
      );

      const started = startRecording();
      await settle();
      expect(mic.startMicrophone).toHaveBeenCalled();
      abortRecording();
      expect(microphone.stop).toHaveBeenCalled();
      expect(recording.abort).toHaveBeenCalledWith('s1');

      switching.resolve();
      await started;

      expect(recorderState().status).toBe('idle');
      expect(recorderState().startedAt).toBeNull();
    });
  });

  describe('when the inspector comes up late', () => {
    it('should switch capture on for a session that is still starting', async () => {
      const switching = deferred<void>();
      setInspectorRecording.mockImplementationOnce(() => switching.promise);
      const started = startRecording();
      await settle();
      expect(recorderState().status).toBe('starting');
      setInspectorRecording.mockClear();

      reapplyInspectorCapture();
      expect(setInspectorRecording).toHaveBeenCalledWith(true);

      switching.resolve();
      await started;
    });

    it('should leave the inspector alone while no session is open', () => {
      reapplyInspectorCapture();
      expect(setInspectorRecording).not.toHaveBeenCalled();
    });
  });

  describe('when the inspector is slow to switch capture on', () => {
    it('should still stop at the limit counted from the microphone opening', async () => {
      vi.useFakeTimers();
      const switching = deferred<void>();
      setInspectorRecording.mockImplementationOnce(() => switching.promise);
      const started = startRecording();
      await vi.advanceTimersByTimeAsync(5_000);
      expect(recorderState().startedAt).not.toBeNull();

      switching.resolve();
      await started;
      await vi.advanceTimersByTimeAsync(MAX_RECORDING_MS - 5_000);

      expect(recording.stop).toHaveBeenCalledWith('s1');
    });
  });

  describe('when the microphone is unplugged mid-recording', () => {
    beforeEach(async () => {
      await startRecording();
      endMicrophone();
      await settle();
    });

    it('should stop the recording as a press of Stop would', () => {
      expect(recording.stop).toHaveBeenCalledWith('s1');
      expect(onFinished).toHaveBeenCalled();
    });

    it('should say why it stopped', () => {
      expect(notifications().at(-1)).toMatchObject({
        severity: 'info',
        message: 'editor.recording.microphone_lost',
      });
    });
  });

  describe('when the user sends without waiting for the transcript', () => {
    let stopping: Deferred<Awaited<ReturnType<typeof recording.stop>>>;

    beforeEach(async () => {
      stopping = deferred();
      vi.mocked(recording.stop).mockReturnValue(stopping.promise);
      await startRecording();
      void stopRecording();
      await settle();
    });

    it('should ask main to give up on the stopping session', () => {
      skipTranscription();
      expect(recording.skipTranscription).toHaveBeenCalledWith('s1');
    });

    it('should hand over the recording without a transcript or a warning', async () => {
      const before = notifications().length;
      skipTranscription();
      stopping.resolve({
        sessionId: 's1',
        dir: '/recordings/p/s1',
        durationMs: 1000,
        transcript: '',
        actionCount: 2,
        context: '[Session recording]',
        transcriptionSkipped: true,
      });
      await settle();

      expect(store.getState().ai.recording?.dir).toBe('/recordings/p/s1');
      expect(store.getState().ai.draftPrompt).toBeNull();
      expect(notifications()).toHaveLength(before);
    });
  });

  describe('when a stop settles after the editor moved on', () => {
    let stopping: Deferred<Awaited<ReturnType<typeof recording.stop>>>;

    beforeEach(async () => {
      stopping = deferred();
      vi.mocked(recording.stop).mockReturnValue(stopping.promise);
      await startRecording();
      void stopRecording();
      await settle();
      abortRecording();
      vi.mocked(recording.start).mockResolvedValue({ sessionId: 's2' });
      await startRecording();
    });

    it('should leave the recording that started meanwhile running', async () => {
      stopping.resolve({
        sessionId: 's1',
        dir: '/recordings/p/s1',
        durationMs: 1000,
        transcript: 'hello',
        actionCount: 2,
        context: '[Session recording]',
      });
      await settle();

      expect(recorderState().status).toBe('recording');
      expect(recorderState().startedAt).not.toBeNull();
    });

    it('should not hand the superseded result to the assistant', async () => {
      stopping.resolve({
        sessionId: 's1',
        dir: '/recordings/p/s1',
        durationMs: 1000,
        transcript: 'hello',
        actionCount: 2,
        context: '[Session recording]',
      });
      await settle();

      expect(store.getState().ai.recording).toBeNull();
      expect(onFinished).not.toHaveBeenCalled();
    });
  });

  describe('when the editor goes away while the stop is flushing the inspector', () => {
    it('should not close the session that replaced it', async () => {
      const flush = deferred<void>();
      await startRecording();
      setInspectorRecording.mockReturnValueOnce(flush.promise);
      const stopped = stopRecording();
      await settle();
      abortRecording();
      vi.mocked(recording.start).mockResolvedValue({ sessionId: 's2' });
      await startRecording();
      session.setActiveRecordingSession.mockClear();

      flush.resolve();
      await stopped;

      expect(session.setActiveRecordingSession).not.toHaveBeenCalled();
      expect(recording.stop).not.toHaveBeenCalled();
      expect(recorderState().status).toBe('recording');
    });
  });

  describe('when the result arrives after the editor opened another project', () => {
    it('should not hand that project a recording of the previous one', async () => {
      const stopping = deferred<Awaited<ReturnType<typeof recording.stop>>>();
      vi.mocked(recording.stop).mockReturnValue(stopping.promise);
      await startRecording();
      void stopRecording();
      await settle();
      setRecorderHost({ projectPath: '/scenes/other', setInspectorRecording, onFinished });

      stopping.resolve({
        sessionId: 's1',
        dir: '/recordings/p/s1',
        durationMs: 1000,
        transcript: 'hello',
        actionCount: 2,
        context: '[Session recording]',
      });
      await settle();

      expect(store.getState().ai.recording).toBeNull();
      expect(onFinished).not.toHaveBeenCalled();
    });
  });

  describe('when the microphone is unplugged while the session is still starting', () => {
    it('should stop as soon as the recording is live, and say why', async () => {
      const switching = deferred<void>();
      setInspectorRecording.mockImplementationOnce(() => switching.promise);
      const started = startRecording();
      await settle();
      endMicrophone();
      await settle();
      expect(recording.stop).not.toHaveBeenCalled();

      switching.resolve();
      await started;
      await settle();

      expect(recording.stop).toHaveBeenCalledWith('s1');
      expect(notifications().at(-1)).toMatchObject({
        severity: 'info',
        message: 'editor.recording.microphone_lost',
      });
    });
  });

  describe('when Send without transcript is pressed before main has been asked to stop', () => {
    it('should still skip once the stop is on its way', async () => {
      const flush = deferred<void>();
      await startRecording();
      setInspectorRecording.mockReturnValueOnce(flush.promise);
      const stopped = stopRecording();
      await settle();

      skipTranscription();
      expect(recording.skipTranscription).not.toHaveBeenCalled();

      flush.resolve();
      await stopped;

      expect(recording.skipTranscription).toHaveBeenCalledWith('s1');
    });
  });

  describe('when the inspector reloads while the recording is stopping', () => {
    it('should not switch its capture back on', async () => {
      const flush = deferred<void>();
      await startRecording();
      setInspectorRecording.mockReturnValueOnce(flush.promise);
      const stopped = stopRecording();
      await settle();
      setInspectorRecording.mockClear();

      reapplyInspectorCapture();

      expect(setInspectorRecording).not.toHaveBeenCalled();
      flush.resolve();
      await stopped;
    });
  });

  describe('when nothing is being transcribed', () => {
    it('should not ask main to skip anything', () => {
      skipTranscription();
      expect(recording.skipTranscription).not.toHaveBeenCalled();
    });
  });

  describe('when the editor goes away while main is finishing the stop', () => {
    it('should not report the rejection that the abort itself causes', async () => {
      const stopping = deferred<Awaited<ReturnType<typeof recording.stop>>>();
      vi.mocked(recording.stop).mockReturnValue(stopping.promise);
      await startRecording();
      const before = notifications().length;

      const stopped = stopRecording();
      await settle();
      abortRecording();
      stopping.reject(new Error('This recording is no longer active.'));
      await stopped;

      expect(recorderState().status).toBe('idle');
      expect(notifications()).toHaveLength(before);
    });

    it('should still report a failure when the editor stays', async () => {
      vi.mocked(recording.stop).mockRejectedValue(new Error('disk full'));
      await startRecording();

      await stopRecording();

      expect(notifications().at(-1)).toMatchObject({ severity: 'error' });
    });
  });

  describe('when the microphone cannot be opened', () => {
    it('should abort the session and report the error', async () => {
      mic.startMicrophone.mockRejectedValue(new Error('Permission denied'));

      await startRecording();

      expect(recording.abort).toHaveBeenCalledWith('s1');
      expect(recorderState().status).toBe('idle');
      expect(notifications().at(-1)).toMatchObject({ severity: 'error' });
    });
  });

  describe('when a recording reaches the length limit', () => {
    beforeEach(async () => {
      vi.useFakeTimers();
      await startRecording();
      await vi.advanceTimersByTimeAsync(MAX_RECORDING_MS);
    });

    it('should stop on its own', () => {
      expect(microphone.stop).toHaveBeenCalled();
      expect(recording.stop).toHaveBeenCalledWith('s1');
    });

    it('should play the chime and say why it stopped', () => {
      expect(chime.playRecordingLimitChime).toHaveBeenCalled();
      expect(notifications().at(-1)).toMatchObject({
        severity: 'info',
        message: 'editor.recording.limit_reached',
      });
    });
  });

  describe('when the user stops before the limit', () => {
    it('should not stop a second time at the limit', async () => {
      vi.useFakeTimers();
      await startRecording();
      await stopRecording();

      await vi.advanceTimersByTimeAsync(MAX_RECORDING_MS);

      expect(recording.stop).toHaveBeenCalledTimes(1);
      expect(chime.playRecordingLimitChime).not.toHaveBeenCalled();
    });
  });

  describe('when the creator has not agreed to being recorded yet', () => {
    beforeEach(async () => {
      acknowledgeConsent(false);
      vi.mocked(recording.toolsStatus).mockResolvedValue({ state: 'missing', downloadMb: 75 });
      await pressRecord();
    });

    it('should ask first, naming the download still to come', () => {
      expect(getConsentPrompt()).toEqual({ downloadMb: 75 });
      expect(recording.start).not.toHaveBeenCalled();
    });

    it('should record once they agree, and not ask again', async () => {
      acceptRecordingConsent();
      await settle();

      expect(getConsentPrompt()).toBeNull();
      expect(store.getState().workspace.settings.recordingConsentAcknowledged).toBe(true);
      expect(recording.start).toHaveBeenCalledWith('/scenes/demo', 'Demo', 'en');
    });
  });

  describe('when the creator has already agreed', () => {
    it('should start recording on the press', async () => {
      await pressRecord();

      expect(getConsentPrompt()).toBeNull();
      expect(recording.start).toHaveBeenCalled();
    });

    it('should stop a running recording on the next press', async () => {
      await pressRecord();

      await pressRecord();

      expect(recording.stop).toHaveBeenCalledWith('s1');
    });
  });
});
