import type { AiPendingRecording } from '/shared/types/ai';
import { formatRecordingDuration } from '/shared/types/recording';

import { t } from '/@/modules/store/translation/utils';

/** The chip a sent message carries for the session recording that went out with it. */
export function recordingAttachmentLabel(recording: AiPendingRecording): string {
  return t('editor.ai.recording.attachment', {
    duration: formatRecordingDuration(recording.durationMs),
  });
}
