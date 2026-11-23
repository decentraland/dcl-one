import type { AiPendingRecording } from '/shared/types/ai';
import { formatRecordingDuration } from '/shared/types/recording';

import { t } from '/@/modules/store/translation/utils';

/** The chip a sent message carries for the recording (or recordings) that went out with it. */
export function recordingAttachmentLabel(recording: AiPendingRecording): string {
  const duration = formatRecordingDuration(recording.durationMs);
  const count = recording.sessions.length;
  return count > 1
    ? t('editor.ai.recording.attachment_many', { count, duration })
    : t('editor.ai.recording.attachment', { duration });
}
