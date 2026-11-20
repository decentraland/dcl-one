import { t } from '/@/modules/store/translation/utils';
import { DefaultModal } from '../Modals/DefaultModal';
import styles from './RecordingConsentModal.module.css';

type Props = {
  open: boolean;
  // The speech-to-text download still to come, in MB (0 when already installed).
  downloadMb: number;
  onAccept: () => void;
  onCancel: () => void;
};

const bold = (chunks: string) => <b>{chunks}</b>;

// Shown the first time the creator presses Record: what gets captured, where it goes, and that
// the system's default microphone is used.
export function RecordingConsentModal({ open, downloadMb, onAccept, onCancel }: Props) {
  return (
    <DefaultModal
      open={open}
      title={t('editor.recording.consent.title')}
      onClose={onCancel}
      corner="none"
      body={
        <div className={styles.sections}>
          <p>{t('editor.recording.consent.intro')}</p>
          <p>
            <b>{t('editor.recording.consent.what_title')}</b>
            <br />
            {t('editor.recording.consent.what')}
          </p>
          <p>
            <b>{t('editor.recording.consent.local_title')}</b>
            <br />
            {t('editor.recording.consent.local', { b: bold })}
            <br />
            {t('editor.recording.consent.stored')}
          </p>
          <p>
            <b>{t('editor.recording.consent.first_title')}</b>
            <br />
            {downloadMb > 0 && <>{t('editor.recording.consent.download', { size: downloadMb })} </>}
            {t('editor.recording.consent.microphone')}
          </p>
        </div>
      }
      secondary={{ label: t('editor.recording.consent.cancel'), onClick: onCancel }}
      primary={{
        label: t('editor.recording.consent.accept'),
        onClick: onAccept,
        state: 'idle',
      }}
    />
  );
}
