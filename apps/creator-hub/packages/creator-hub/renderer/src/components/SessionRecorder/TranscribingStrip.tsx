import { useState } from 'react';
import { CircularProgress } from 'decentraland-ui2';

import type { RecorderStatus } from '/shared/types/ai';
import type { RecordingToolsStatus } from '/shared/types/recording';

import { t } from '/@/modules/store/translation/utils';
import { busyLabel, elapsed } from './time';
import styles from './TranscribingStrip.module.css';

type Props = {
  status: RecorderStatus;
  startedAt: number | null;
  tools: RecordingToolsStatus | null;
};

// Stands in for the composer's text field while a stopped recording is transcribed: what was
// said lands in that field when it finishes.
export function TranscribingStrip({ status, startedAt, tools }: Props) {
  // Mounted when the recording stops, so this is its length.
  const [recorded] = useState(() => (startedAt === null ? null : elapsed(startedAt, Date.now())));
  return (
    <div
      className={styles.strip}
      role="status"
      data-testid="recording-transcribing"
    >
      <CircularProgress
        size={16}
        color="inherit"
      />
      <span className={styles.label}>{busyLabel(status, tools)}</span>
      {recorded !== null && (
        <span className={styles.meta}>{t('editor.recording.recorded', { time: recorded })}</span>
      )}
    </div>
  );
}
