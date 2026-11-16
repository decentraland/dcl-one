import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { MAX_RECORDING_MS } from '/shared/types/recording';

const dispatch = vi.fn();
vi.mock('#store', () => ({
  useDispatch: () => dispatch,
  useSelector: (select: (state: unknown) => unknown) => select({ translation: { locale: 'en' } }),
}));
vi.mock('/@/modules/store/translation/utils', () => ({ t: (key: string) => key }));

const mic = vi.hoisted(() => ({ startMicrophone: vi.fn() }));
vi.mock('/@/modules/recording/microphone', () => mic);
const session = vi.hoisted(() => ({
  setActiveRecordingSession: vi.fn(),
  startHubDomRecorder: vi.fn(() => vi.fn()),
}));
vi.mock('/@/modules/recording/session', () => session);
const chime = vi.hoisted(() => ({ playRecordingLimitChime: vi.fn(async () => {}) }));
vi.mock('/@/modules/recording/chime', () => chime);

const { recording } = await import('#preload');
const { useSessionRecorder } = await import('./useSessionRecorder');

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

const project = { path: '/scenes/demo', title: 'Demo' } as never;

describe('useSessionRecorder', () => {
  let microphone: { label: string; stop: ReturnType<typeof vi.fn> };
  let setRecording: ReturnType<typeof vi.fn>;
  let rpcRef: { current: { scene: { setRecording: typeof setRecording } } };
  let onFinished: ReturnType<typeof vi.fn>;

  function render(enabled = true) {
    return renderHook(
      ({ on }: { on: boolean }) =>
        useSessionRecorder({
          enabled: on,
          project,
          rpcRef: rpcRef as never,
          inspectorReadyNonce: 0,
          onFinished,
        }),
      { initialProps: { on: enabled } },
    );
  }

  beforeEach(() => {
    dispatch.mockReset();
    microphone = { label: 'Built-in mic', stop: vi.fn() };
    mic.startMicrophone.mockReset().mockResolvedValue(microphone);
    session.setActiveRecordingSession.mockReset();
    session.startHubDomRecorder.mockReset().mockReturnValue(vi.fn());
    chime.playRecordingLimitChime.mockClear();
    setRecording = vi.fn(async () => {});
    rpcRef = { current: { scene: { setRecording } } };
    onFinished = vi.fn();
    vi.mocked(recording.toolsStatus).mockResolvedValue({ state: 'ready' });
    vi.mocked(recording.start).mockResolvedValue({ sessionId: 's1' });
    vi.mocked(recording.abort).mockResolvedValue(undefined);
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
    vi.useRealTimers();
  });

  describe('when the user starts and stops a recording', () => {
    it('should record, then hand the result to the assistant', async () => {
      const { result } = render();
      await act(() => result.current.start());
      expect(result.current.status).toBe('recording');
      expect(result.current.micLabel).toBe('Built-in mic');
      expect(setRecording).toHaveBeenCalledWith(true);

      await act(() => result.current.stop());
      expect(microphone.stop).toHaveBeenCalled();
      expect(recording.stop).toHaveBeenCalledWith('s1');
      expect(result.current.status).toBe('idle');
      expect(onFinished).toHaveBeenCalled();
      expect(chime.playRecordingLimitChime).not.toHaveBeenCalled();
    });

    it('should keep the relay on the session until the inspector has flushed', async () => {
      const flush = deferred<void>();
      const { result } = render();
      await act(() => result.current.start());
      session.setActiveRecordingSession.mockClear();
      setRecording.mockReturnValueOnce(flush.promise);
      let stopped: Promise<void>;
      act(() => {
        stopped = result.current.stop();
      });
      expect(session.setActiveRecordingSession).not.toHaveBeenCalled();
      flush.resolve();
      await act(() => stopped);
      expect(session.setActiveRecordingSession).toHaveBeenCalledWith(null);
    });
  });

  describe('when the editor goes away while main is still opening the session', () => {
    it('should abort that session and never open the microphone', async () => {
      const opening = deferred<{ sessionId: string }>();
      vi.mocked(recording.start).mockReturnValue(opening.promise);
      const { result, rerender } = render();
      let started: Promise<void>;
      act(() => {
        started = result.current.start();
      });
      expect(result.current.status).toBe('starting');
      await act(async () => {
        await Promise.resolve();
      });
      expect(recording.start).toHaveBeenCalled();
      rerender({ on: false });
      opening.resolve({ sessionId: 's1' });
      await act(() => started);
      expect(mic.startMicrophone).not.toHaveBeenCalled();
      expect(recording.abort).toHaveBeenCalledWith('s1');
      expect(result.current.status).toBe('idle');
    });
  });

  describe('when the editor goes away while the microphone is opening', () => {
    it('should release the microphone as soon as it opens', async () => {
      const opening = deferred<typeof microphone>();
      mic.startMicrophone.mockReturnValue(opening.promise);
      const { result, rerender } = render();
      let started: Promise<void>;
      act(() => {
        started = result.current.start();
      });
      await act(async () => {
        await Promise.resolve();
      });
      rerender({ on: false });
      expect(recording.abort).toHaveBeenCalledWith('s1');
      opening.resolve(microphone);
      await act(() => started);
      expect(microphone.stop).toHaveBeenCalled();
      expect(result.current.status).toBe('idle');
      expect(setRecording).not.toHaveBeenCalledWith(true);
    });
  });

  describe('when the editor goes away while the inspector is switching capture on', () => {
    it('should stay idle, with the microphone released and the session aborted', async () => {
      const switching = deferred<void>();
      setRecording.mockImplementation((on: boolean) =>
        on ? switching.promise : Promise.resolve(),
      );
      const { result, rerender } = render();
      let started: Promise<void>;
      act(() => {
        started = result.current.start();
      });
      await act(async () => {
        await Promise.resolve();
        await Promise.resolve();
      });
      expect(mic.startMicrophone).toHaveBeenCalled();
      rerender({ on: false });
      expect(microphone.stop).toHaveBeenCalled();
      expect(recording.abort).toHaveBeenCalledWith('s1');
      switching.resolve();
      await act(() => started);
      expect(result.current.status).toBe('idle');
      expect(result.current.startedAt).toBeNull();
    });
  });

  describe('when the microphone cannot be opened', () => {
    it('should abort the session and report the error', async () => {
      mic.startMicrophone.mockRejectedValue(new Error('Permission denied'));
      const { result } = render();
      await act(() => result.current.start());
      expect(recording.abort).toHaveBeenCalledWith('s1');
      expect(result.current.status).toBe('idle');
      expect(dispatch).toHaveBeenCalledWith(
        expect.objectContaining({
          type: 'snackbar/pushSnackbar',
          payload: expect.objectContaining({ severity: 'error' }),
        }),
      );
    });
  });

  describe('when a recording reaches the length limit', () => {
    beforeEach(async () => {
      vi.useFakeTimers();
      const { result } = render();
      await act(() => result.current.start());
      dispatch.mockClear();
      await act(async () => {
        await vi.advanceTimersByTimeAsync(MAX_RECORDING_MS);
      });
    });

    it('should stop on its own', () => {
      expect(microphone.stop).toHaveBeenCalled();
      expect(recording.stop).toHaveBeenCalledWith('s1');
    });

    it('should play the chime and say why it stopped', () => {
      expect(chime.playRecordingLimitChime).toHaveBeenCalled();
      expect(dispatch).toHaveBeenCalledWith(
        expect.objectContaining({
          type: 'snackbar/pushSnackbar',
          payload: expect.objectContaining({
            severity: 'info',
            message: 'editor.recording.limit_reached',
          }),
        }),
      );
    });
  });

  describe('when the user stops before the limit', () => {
    it('should not stop a second time at the limit', async () => {
      vi.useFakeTimers();
      const { result } = render();
      await act(() => result.current.start());
      await act(() => result.current.stop());
      await act(async () => {
        await vi.advanceTimersByTimeAsync(MAX_RECORDING_MS);
      });
      expect(recording.stop).toHaveBeenCalledTimes(1);
      expect(chime.playRecordingLimitChime).not.toHaveBeenCalled();
    });
  });
});
