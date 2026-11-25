import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from 'react';
import IconButton from '@ui/atoms/IconButton';
import { Tooltip } from '@dcl/one-ui';

import closeIcon from '/assets/images/recording/close-thick.svg';
import sendIcon from '/assets/images/recording/send.svg';
import stopIcon from '/assets/images/recording/stop.svg';

import { onMicLevel } from '/@/modules/recording/level';
import { pauseHubCapture } from '/@/modules/recording/session';
import { t } from '/@/modules/store/translation/utils';
import { DefaultModal } from '../Modals/DefaultModal';
import styles from './ComposerRecorder.module.css';

/** More than fit in the widest panel: the bar clips the oldest on the left. */
const MAX_BARS = 160;
const BAR_MIN_PX = 3;
const BAR_RANGE_PX = 19;

/** Speech sits low in the range, so a bar follows the square root; a linear one barely moves. */
export function barHeightPx(level: number): number {
  return BAR_MIN_PX + Math.round(Math.sqrt(level) * BAR_RANGE_PX);
}

// The microphone's recent loudness, newest on the right. Empty until a level arrives: only the
// window that records hears the microphone, so the popped-out chat shows no bars.
function Waveform() {
  const [levels, setLevels] = useState<number[]>([]);
  useEffect(
    () => onMicLevel(level => setLevels(previous => [...previous.slice(1 - MAX_BARS), level])),
    [],
  );
  return (
    <div
      className={styles.wave}
      aria-hidden="true"
      data-testid="recording-waveform"
    >
      {levels.map((level, index) => (
        <span
          key={index}
          className={styles.waveBar}
          style={{ '--bar-height': `${barHeightPx(level)}px` } as CSSProperties}
        />
      ))}
    </div>
  );
}

function BarIcon({ src }: { src: string }) {
  return (
    <img
      className={styles.icon}
      src={src}
      alt=""
      width={16}
      height={16}
    />
  );
}

type Props = {
  onStop: () => void;
  /** Stops and sends the recording once transcribed. Without it there is no Send. */
  onSend?: () => void;
  /** Throws the running recording away, after the user confirms. Without it there is no Discard. */
  onDiscard?: () => void;
  /** Take the keyboard: the recording started just now, so the Record button has gone. */
  focusStopOnMount?: boolean;
  /** Called when the bar leaves while it still holds the keyboard. */
  onFocusRelease?: () => void;
};

/** The running recording, in the text field's place: Discard, the live waveform, Stop and Send. */
export function RecordingBar({
  onStop,
  onSend,
  onDiscard,
  focusStopOnMount = false,
  onFocusRelease,
}: Props) {
  const [confirming, setConfirming] = useState(false);
  useEffect(() => (confirming ? pauseHubCapture() : undefined), [confirming]);
  const barRef = useRef<HTMLDivElement>(null);
  const stopRef = useRef<HTMLButtonElement>(null);
  const release = useRef(onFocusRelease);
  release.current = onFocusRelease;
  useLayoutEffect(() => {
    if (focusStopOnMount) stopRef.current?.focus();
    return () => {
      if (barRef.current?.contains(document.activeElement)) release.current?.();
    };
  }, []);

  return (
    <div
      ref={barRef}
      className={styles.bar}
      data-state="recording"
      role="group"
      aria-label={t('editor.recording.live')}
      data-testid="recording-bar"
      data-recording-ignore
    >
      {onDiscard !== undefined && (
        <Tooltip
          label={t('editor.recording.discard')}
          side="top"
          portal
          keepInViewport
        >
          <IconButton
            className={styles.round}
            aria-label={t('editor.recording.discard')}
            onClick={() => setConfirming(true)}
          >
            <BarIcon src={closeIcon} />
          </IconButton>
        </Tooltip>
      )}
      <Waveform />
      <Tooltip
        label={t('editor.recording.stop')}
        side="top"
        portal
        keepInViewport
      >
        <IconButton
          ref={stopRef}
          className={styles.round}
          aria-label={t('editor.recording.stop')}
          onClick={onStop}
          data-testid="editor-record-button"
        >
          <BarIcon src={stopIcon} />
        </IconButton>
      </Tooltip>
      {onSend !== undefined && (
        <Tooltip
          label={t('editor.recording.send')}
          side="top"
          portal
          keepInViewport
        >
          <IconButton
            className={styles.send}
            aria-label={t('editor.recording.send')}
            onClick={onSend}
            data-testid="recording-send"
          >
            <BarIcon src={sendIcon} />
          </IconButton>
        </Tooltip>
      )}
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
