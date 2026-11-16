import { useEffect, useState } from 'react';
import { Tooltip } from 'decentraland-ui2';

import { MAX_RECORDING_MS } from '/shared/types/recording';

import { onMicLevel } from '/@/modules/recording/level';
import { t } from '/@/modules/store/translation/utils';
import { DefaultModal } from '../Modals/DefaultModal';
import { LIMIT, elapsed, useNow } from './time';
import styles from './RecordingCard.module.css';

const BARS = 48;
const BAR_MIN_PX = 4;
const BAR_RANGE_PX = 24;

// The microphone's recent loudness, newest on the right. Renders nothing until a level arrives:
// only the window that records hears the microphone, so the popped-out chat has no waveform.
function Waveform() {
  const [levels, setLevels] = useState<number[]>([]);
  useEffect(
    () => onMicLevel(level => setLevels(previous => [...previous.slice(1 - BARS), level])),
    [],
  );
  if (levels.length === 0) return null;
  return (
    <div
      className={styles.wave}
      aria-hidden="true"
    >
      {levels.map((level, index) => (
        <span
          key={index}
          className={styles.bar}
          // Square root: speech sits low in the range, and a linear bar would barely move.
          style={{ height: BAR_MIN_PX + Math.round(Math.sqrt(level) * BAR_RANGE_PX) }}
        />
      ))}
    </div>
  );
}

type Props = {
  startedAt: number | null;
  micLabel: string;
  actionCount: number;
  onStop: () => void;
  // Throws the running recording away, after the user confirms. Without it there is no Discard.
  onDiscard?: () => void;
};

// The running recording, shown above the composer: timer, waveform, what was captured so far,
// and the two ways to end it.
export function RecordingCard({ startedAt, micLabel, actionCount, onStop, onDiscard }: Props) {
  const now = useNow(true);
  const [confirming, setConfirming] = useState(false);

  return (
    <div
      className={styles.card}
      role="group"
      aria-label={t('editor.recording.live')}
      data-testid="recording-card"
      data-recording-ignore
    >
      <div className={styles.header}>
        <span className={styles.dot} />
        <Tooltip
          title={t('editor.recording.tooltip_live', {
            mic: micLabel || t('editor.recording.default_mic'),
            minutes: MAX_RECORDING_MS / 60_000,
          })}
        >
          <span className={styles.title}>{t('editor.recording.live')}</span>
        </Tooltip>
        <span
          className={styles.time}
          role="timer"
        >
          {elapsed(startedAt, now)}
        </span>
        <span className={styles.limit}>/ {LIMIT}</span>
      </div>
      <Waveform />
      <div className={styles.footer}>
        <span className={styles.meta}>
          {t('editor.recording.actions_captured', { count: actionCount })}
        </span>
        {onDiscard !== undefined && (
          <button
            className={styles.discard}
            type="button"
            onClick={() => setConfirming(true)}
          >
            {t('editor.recording.discard')}
          </button>
        )}
        <button
          className={styles.stop}
          type="button"
          onClick={onStop}
          aria-label={t('editor.recording.stop')}
          data-testid="editor-record-button"
        >
          <span className={styles.square} />
          {t('editor.recording.stop_short')}
        </button>
      </div>
      {onDiscard !== undefined && (
        <DefaultModal
          open={confirming}
          title={t('editor.recording.discard_confirm.title')}
          body={t('editor.recording.discard_confirm.body')}
          onClose={() => setConfirming(false)}
          secondary={{
            label: t('editor.recording.discard_confirm.cancel'),
            onClick: () => setConfirming(false),
          }}
          primary={{
            label: t('editor.recording.discard_confirm.confirm'),
            onClick: () => {
              setConfirming(false);
              onDiscard();
            },
            state: 'idle',
          }}
        />
      )}
    </div>
  );
}
