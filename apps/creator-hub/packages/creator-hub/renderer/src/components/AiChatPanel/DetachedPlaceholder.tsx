import type { CSSProperties } from 'react';
import { OpenInNewIcon } from '@ui/atoms/icons-material';
import Button from '@ui/atoms/Button';

import { useSelector } from '#store';
import { t } from '/@/modules/store/translation/utils';

import { RecordButton, RecordingBar, TranscribingBar } from '../SessionRecorder';
import recordRowStyles from './RecordRow.module.css';
import styles from './component.module.css';

type Props = {
  onDock: () => void;
  width: number;
  onToggleRecording: () => void;
  onCancelRecording: () => void;
  onSendRecording: () => void;
};

/** Stands in for the inline panel while the chat is popped out, keeping this window's Record. */
export function DetachedPlaceholder({
  onDock,
  width,
  onToggleRecording,
  onCancelRecording,
  onSendRecording,
}: Props) {
  const recorder = useSelector(state => state.ai.recorder);
  const available = useSelector(
    state => state.ai.providers.find(p => p.id === state.ai.provider)?.available ?? false,
  );
  return (
    <div
      className={styles.panel}
      style={{ '--panel-width': `${width}px` } as CSSProperties}
      aria-label="ai-chat-detached"
    >
      <div className={styles.panelHeader}>
        <div className={styles.headerTitle}>{t('editor.ai.title')}</div>
      </div>
      <div className={recordRowStyles.recordRow}>
        {recorder.status === 'recording' ? (
          <RecordingBar
            onStop={onToggleRecording}
            onSend={onSendRecording}
            onDiscard={onCancelRecording}
          />
        ) : recorder.status === 'stopping' ? (
          <TranscribingBar
            endedWith={recorder.endedWith}
            tools={recorder.tools}
          />
        ) : (
          <RecordButton
            fullWidth
            status={recorder.status}
            startedAt={recorder.startedAt}
            micLabel={recorder.micLabel}
            tools={recorder.tools}
            onClick={onToggleRecording}
            disabled={!available}
          />
        )}
      </div>
      <div className={styles.emptyState}>
        <div>{t('editor.ai.detached_message')}</div>
        <div>
          <Button
            variant="text"
            tone="neutral"
            size="sm"
            className={styles.textButton}
            startIcon={<OpenInNewIcon fontSize="small" />}
            onClick={onDock}
          >
            {t('editor.ai.detached_dock')}
          </Button>
        </div>
      </div>
    </div>
  );
}
