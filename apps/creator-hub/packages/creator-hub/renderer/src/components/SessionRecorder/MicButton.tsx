import IconButton from '@ui/atoms/IconButton';
import Spinner from '@ui/atoms/Spinner';
import Tooltip from '@ui/atoms/Tooltip';

import microphoneIcon from '/assets/images/recording/microphone.svg';

import { t } from '/@/modules/store/translation/utils';
import styles from './ComposerRecorder.module.css';

type Props = {
  /** Pressed: the microphone and the editor's capture are coming up. */
  starting: boolean;
  disabled: boolean;
  onClick: () => void;
};

/** The composer's Record button, beside Send. */
export function MicButton({ starting, disabled, onClick }: Props) {
  const label = t('editor.recording.record_tooltip');
  return (
    <Tooltip
      label={label}
      side="top"
      portal
      keepInViewport
    >
      <IconButton
        className={styles.round}
        aria-label={label}
        disabled={disabled || starting}
        onClick={onClick}
        data-testid="editor-record-button"
        data-recording-ignore
      >
        {starting ? (
          <Spinner
            ring="wide"
            size={16}
            color="inherit"
            role="progressbar"
            label={t('editor.recording.starting')}
          />
        ) : (
          <img
            className={styles.icon}
            src={microphoneIcon}
            alt=""
            width={16}
            height={16}
          />
        )}
      </IconButton>
    </Tooltip>
  );
}
