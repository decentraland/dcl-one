import { useLayoutEffect, useRef } from 'react';
import IconButton from '@ui/atoms/IconButton';
import Spinner from '@ui/atoms/Spinner';

import sendIcon from '/assets/images/recording/send.svg';
import stopIcon from '/assets/images/recording/stop.svg';

import type { RecordingToolsStatus } from '/shared/types/recording';

import { t } from '/@/modules/store/translation/utils';
import { busyLabel } from './time';
import styles from './ComposerRecorder.module.css';

type Props = {
  /** The button that ended the recording: the spinner takes its place. */
  endedWith: 'stop' | 'send' | null;
  tools: RecordingToolsStatus | null;
  /** Called when the bar leaves while it still holds the keyboard. */
  onFocusRelease?: () => void;
};

/**
 * A stopped recording while its speech is transcribed, in the text field's place. It is sent
 * as soon as the transcript is ready, so nothing here can be pressed.
 */
export function TranscribingBar({ endedWith, tools, onFocusRelease }: Props) {
  const label = busyLabel('stopping', tools);
  const barRef = useRef<HTMLDivElement>(null);
  const release = useRef(onFocusRelease);
  release.current = onFocusRelease;
  useLayoutEffect(
    () => () => {
      if (barRef.current?.contains(document.activeElement)) release.current?.();
    },
    [],
  );

  const spinner = (
    <span className={styles.spinnerSlot}>
      <Spinner
        ring="wide"
        size={16}
        color="inherit"
        role="progressbar"
        label={label}
      />
    </span>
  );

  return (
    <div
      ref={barRef}
      className={styles.bar}
      data-state="transcribing"
      data-testid="recording-transcribing"
      data-recording-ignore
    >
      <span
        className={styles.status}
        role="status"
      >
        {label}
      </span>
      {endedWith === 'send' ? (
        <>
          <IconButton
            className={styles.round}
            aria-label={t('editor.recording.stop')}
            disabled
          >
            <img
              className={styles.icon}
              src={stopIcon}
              alt=""
              width={16}
              height={16}
            />
          </IconButton>
          {spinner}
        </>
      ) : (
        <>
          {spinner}
          <IconButton
            className={styles.send}
            aria-label={t('editor.ai.send')}
            disabled
          >
            <img
              className={styles.icon}
              src={sendIcon}
              alt=""
              width={16}
              height={16}
            />
          </IconButton>
        </>
      )}
    </div>
  );
}
