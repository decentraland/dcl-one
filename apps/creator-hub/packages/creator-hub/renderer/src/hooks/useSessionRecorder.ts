import { useEffect, useSyncExternalStore } from 'react';

import { useDispatch, useSelector } from '#store';
import type { RecorderStatus } from '/shared/types/ai';
import type { Project } from '/shared/types/projects';

import { actions as aiActions } from '/@/modules/store/ai';
import type { initRpc } from '/@/modules/rpc';
import {
  abortRecording,
  acceptRecordingConsent,
  cancelRecording,
  dismissRecordingConsent,
  getConsentPrompt,
  onConsentPrompt,
  pressRecord,
  reapplyInspectorCapture,
  sendRecording,
  setRecorderHost,
} from '/@/modules/recording/controller';

export type { RecorderStatus };

type Rpc = ReturnType<typeof initRpc>;

/**
 * Binds the session recorder to the editor: its state comes from the store, its controls are
 * the controller's, and it lends the controller this editor's project and inspector iframe.
 *
 * @param enabled The assistant is on — that is where a recording goes, so nothing records without it.
 * @param inspectorReadyNonce Bumped whenever the inspector's scene RPC reports ready again.
 * @param onFinished Called once a stopped recording has reached the assistant.
 */
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
  const recorder = useSelector(state => state.ai.recorder);
  const consent = useSyncExternalStore(onConsentPrompt, getConsentPrompt);
  const projectPath = project?.path;

  useEffect(() => {
    setRecorderHost({
      projectPath,
      setInspectorRecording: async on => void (await rpcRef.current?.scene.setRecording(on)),
      onFinished,
    });
  });

  useEffect(() => {
    if (!enabled) abortRecording();
  }, [enabled]);

  useEffect(() => {
    reapplyInspectorCapture();
  }, [inspectorReadyNonce]);

  useEffect(
    () => () => {
      abortRecording();
      setRecorderHost(null);
      dispatch(aiActions.setRecording(null));
    },
    [projectPath, dispatch],
  );

  return {
    ...recorder,
    consent,
    press: pressRecord,
    cancel: cancelRecording,
    send: sendRecording,
    acceptConsent: acceptRecordingConsent,
    dismissConsent: dismissRecordingConsent,
  };
}
