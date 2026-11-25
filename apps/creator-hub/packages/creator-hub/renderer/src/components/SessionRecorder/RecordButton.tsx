import Spinner from '@ui/atoms/Spinner';
import { Tooltip } from '@dcl/one-ui';

import type { RecorderStatus } from '/shared/types/ai';
import { MAX_RECORDING_MS, type RecordingToolsStatus } from '/shared/types/recording';

import { t } from '/@/modules/store/translation/utils';
import { MicrophoneIcon } from '../Icons';
import { LIMIT, busyLabel, elapsed, useNow } from './time';
import styles from './RecordButton.module.css';

type Props = {
  status: RecorderStatus;
  startedAt: number | null;
  micLabel: string;
  tools: RecordingToolsStatus | null;
  onClick: () => void;
  // The popped-out chat's placeholder: a full-width bar with a longer idle label.
  fullWidth?: boolean;
  disabled?: boolean;
};

export function RecordButton({
  status,
  startedAt,
  micLabel,
  tools,
  onClick,
  fullWidth = false,
  disabled = false,
}: Props) {
  const live = status === 'recording';
  const now = useNow(live);

  const busy = status === 'starting' || status === 'stopping';
  const tooltip = live
    ? t('editor.recording.tooltip_live', {
        mic: micLabel || t('editor.recording.default_mic'),
        minutes: MAX_RECORDING_MS / 60_000,
      })
    : t('editor.recording.tooltip_idle', { minutes: MAX_RECORDING_MS / 60_000 });

  return (
    <Tooltip
      label={tooltip}
      side="top"
      portal
      keepInViewport
    >
      <button
        className={styles.control}
        data-live={live}
        data-full={fullWidth}
        type="button"
        disabled={busy || (disabled && !live)}
        onClick={onClick}
        aria-label={live ? t('editor.recording.stop') : t('editor.recording.record')}
        data-testid="editor-record-button"
        data-recording-ignore
      >
        {busy ? (
          <Spinner
            ring="wide"
            size={14}
            color="inherit"
            role="progressbar"
          />
        ) : live ? (
          <span className={styles.dot} />
        ) : (
          <MicrophoneIcon
            className={styles.mic}
            size={16}
          />
        )}
        <span className={styles.label}>
          {busy
            ? busyLabel(status, tools)
            : live
              ? t('editor.recording.stop_with_time', {
                  time: elapsed(startedAt, now),
                  limit: LIMIT,
                })
              : fullWidth
                ? t('editor.recording.record_panel')
                : t('editor.recording.record')}
        </span>
      </button>
    </Tooltip>
  );
}
