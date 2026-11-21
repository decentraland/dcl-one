import { useEffect, useState } from 'react';

import type { RecorderStatus } from '/shared/types/ai';
import {
  MAX_RECORDING_MS,
  formatRecordingDuration,
  type RecordingToolsStatus,
} from '/shared/types/recording';

import { t } from '/@/modules/store/translation/utils';

/** How long a recording that began at `startedAt` has been running, capped at the limit. */
export function elapsed(startedAt: number | null, now: number): string {
  return formatRecordingDuration(
    startedAt === null ? 0 : Math.min(MAX_RECORDING_MS, now - startedAt),
  );
}

export const LIMIT = formatRecordingDuration(MAX_RECORDING_MS);

// The current time, refreshed twice a second while `live`, for the recording timer.
export function useNow(live: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!live) return;
    setNow(Date.now());
    const timer = setInterval(() => setNow(Date.now()), 500);
    return () => clearInterval(timer);
  }, [live]);
  return now;
}

export function busyLabel(status: RecorderStatus, tools: RecordingToolsStatus | null): string {
  if (status === 'starting') return t('editor.recording.starting');
  if (tools?.state === 'installing' && tools.percent !== undefined) {
    return t('editor.recording.downloading', { percent: tools.percent });
  }
  return t('editor.recording.transcribing');
}
