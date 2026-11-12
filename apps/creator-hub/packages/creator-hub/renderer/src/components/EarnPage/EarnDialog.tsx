import { IconButton, Modal as MuiModal } from 'decentraland-ui2';

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
    <MuiModal
      className={styles.welcomeRoot}
      open
      onClose={onClose}
    >
      <div
        className={styles.welcomePaper}
        role="dialog"
        aria-modal="true"
        aria-labelledby={labelledBy}
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
      </div>
    </MuiModal>
  );
}
