import { renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { AiRecorderState } from '/shared/types/ai';

const dispatch = vi.fn();
const recorder: AiRecorderState = {
  status: 'recording',
  startedAt: 1000,
  micLabel: 'Built-in mic',
  tools: null,
  actionCount: 3,
};
vi.mock('#store', () => ({
  useDispatch: () => dispatch,
  useSelector: (select: (state: unknown) => unknown) => select({ ai: { recorder } }),
}));

const controller = vi.hoisted(() => ({
  abortRecording: vi.fn(),
  acceptRecordingConsent: vi.fn(),
  cancelRecording: vi.fn(),
  dismissRecordingConsent: vi.fn(),
  getConsentPrompt: vi.fn(() => null),
  onConsentPrompt: vi.fn(() => () => {}),
  pressRecord: vi.fn(),
  reapplyInspectorCapture: vi.fn(),
  setRecorderHost: vi.fn(),
  skipTranscription: vi.fn(),
}));
vi.mock('/@/modules/recording/controller', () => controller);

const { useSessionRecorder } = await import('./useSessionRecorder');

const project = { path: '/scenes/demo', title: 'Demo' } as never;

describe('useSessionRecorder', () => {
  let setRecording: ReturnType<typeof vi.fn>;
  let rpcRef: { current: { scene: { setRecording: typeof setRecording } } };

  function render({ on = true, nonce = 0 } = {}) {
    return renderHook(
      (props: { on: boolean; nonce: number }) =>
        useSessionRecorder({
          enabled: props.on,
          project,
          rpcRef: rpcRef as never,
          inspectorReadyNonce: props.nonce,
          onFinished: () => {},
        }),
      { initialProps: { on, nonce } },
    );
  }

  beforeEach(() => {
    setRecording = vi.fn(async () => {});
    rpcRef = { current: { scene: { setRecording } } };
  });

  describe('when the editor mounts it', () => {
    it('should show what the controller publishes to the store', () => {
      const { result } = render();

      expect(result.current.status).toBe('recording');
      expect(result.current.micLabel).toBe('Built-in mic');
      expect(result.current.actionCount).toBe(3);
    });

    it('should lend the controller this project and inspector', async () => {
      render();

      const host = controller.setRecorderHost.mock.lastCall?.[0];
      expect(host.projectPath).toBe('/scenes/demo');
      await host.setInspectorRecording(true);
      expect(setRecording).toHaveBeenCalledWith(true);
    });
  });

  describe('when the assistant is turned off', () => {
    it('should end a running recording', () => {
      const { rerender } = render();
      controller.abortRecording.mockClear();

      rerender({ on: false, nonce: 0 });

      expect(controller.abortRecording).toHaveBeenCalled();
    });
  });

  describe('when the inspector reports ready again', () => {
    it('should switch its capture back on', () => {
      const { rerender } = render();
      controller.reapplyInspectorCapture.mockClear();

      rerender({ on: true, nonce: 1 });

      expect(controller.reapplyInspectorCapture).toHaveBeenCalled();
    });
  });

  describe('when the editor goes away', () => {
    it('should end the recording and take the host back', () => {
      const { unmount } = render();
      controller.abortRecording.mockClear();

      unmount();

      expect(controller.abortRecording).toHaveBeenCalled();
      expect(controller.setRecorderHost).toHaveBeenLastCalledWith(null);
      expect(dispatch).toHaveBeenCalledWith(
        expect.objectContaining({ type: 'ai/setRecording', payload: null }),
      );
    });
  });
});
