import Dialog from '@ui/atoms/Dialog';
import IconButton from '@ui/atoms/IconButton';

import { t } from '/@/modules/store/translation/utils';

import { CloseIcon } from './icons';
import styles from './EarnModal.module.css';

/**
 * The EARN modal shell over the Black 80 overlay; the overlay, Escape and the close button all
 * call `onClose`.
 */
export function EarnDialog({
  labelledBy,
  onClose,
  children,
}: {
  labelledBy: string;
  onClose: () => void;
  children: React.ReactNode;
}) {
  return (
    <Dialog
      bare
      open
      onClose={() => onClose()}
      aria-labelledby={labelledBy}
      className={styles.welcomePaper}
      backdropClassName={styles.welcomeRoot}
    >
      <div className={styles.statusDialog}>
        <div
          className={styles.glowLayer}
          aria-hidden
        >
          <div className={styles.statusGlow} />
        </div>
        <IconButton
          className={styles.closeButton}
          aria-label={t('earn.close')}
          onClick={onClose}
        >
          <CloseIcon />
        </IconButton>
        {children}
      </div>
    </Dialog>
  );
}
