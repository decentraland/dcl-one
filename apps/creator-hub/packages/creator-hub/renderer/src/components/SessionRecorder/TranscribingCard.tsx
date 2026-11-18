import { useLayoutEffect, useRef, useState } from 'react';

import type { RecordingToolsStatus } from '/shared/types/recording';

import { t } from '/@/modules/store/translation/utils';
import { elapsed } from './time';
import styles from './TranscribingCard.module.css';

type Props = {
  startedAt: number | null;
  actionCount: number;
  tools: RecordingToolsStatus | null;
  /** Gives up on the transcript: the recording goes to the chat with its actions only. */
  onSkip?: () => void;
  /** Called when the card leaves while it still holds the keyboard. */
  onFocusRelease?: () => void;
};

/** A stopped recording while its speech is transcribed, in the place the recording card had. */
export function TranscribingCard({ startedAt, actionCount, tools, onSkip, onFocusRelease }: Props) {
  const [recorded] = useState(() => elapsed(startedAt, Date.now()));
  const cardRef = useRef<HTMLDivElement>(null);
  const release = useRef(onFocusRelease);
  release.current = onFocusRelease;
  useLayoutEffect(
    () => () => {
      if (cardRef.current?.contains(document.activeElement)) release.current?.();
    },
    [],
  );

  const preparing = tools?.state === 'installing';
  const percent = tools?.state === 'installing' ? tools.percent : undefined;
  const title = preparing
    ? t('editor.recording.preparing_title')
    : t('editor.recording.transcribing_title');
  const detail = preparing
    ? t('editor.recording.downloading_detail')
    : t('editor.recording.transcribing_detail');

  return (
    <div
      ref={cardRef}
      className={styles.card}
      role="group"
      aria-label={title}
      data-testid="recording-transcribing"
      data-recording-ignore
    >
      <div className={styles.header}>
        <span className={styles.dot} />
        <span className={styles.title}>{title}</span>
        <span className={styles.time}>{recorded}</span>
      </div>
      <div className={styles.progress}>
        <div
          className={styles.track}
          role="progressbar"
          aria-label={title}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={percent}
        >
          {percent === undefined ? (
            <span className={styles.sweep} />
          ) : (
            <span
              className={styles.fill}
              style={{ width: `${percent}%` }}
            />
          )}
        </div>
        {percent !== undefined && <span className={styles.percent}>{percent}%</span>}
      </div>
      <p
        className={styles.detail}
        role="status"
      >
        {detail}
      </p>
      <div className={styles.footer}>
        <span className={styles.meta}>
          {t('editor.recording.actions_captured', { count: actionCount })}
        </span>
        {onSkip !== undefined && (
          <button
            className={styles.skip}
            type="button"
            onClick={onSkip}
          >
            {t('editor.recording.send_without_transcript')}
          </button>
        )}
      </div>
    </div>
  );
}
