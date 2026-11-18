import { useId, type ReactNode } from 'react';
import Dialog from '@ui/atoms/Dialog';
import IconButton from '@ui/atoms/IconButton';
import { CloseIcon } from '@ui/atoms/icons-material';

import { t } from '/@/modules/store/translation/utils';

import styles from './TitleBarModal.module.css';

type Props = {
  open: boolean;
  title: string;
  onClose: () => void;
  children: ReactNode;
};

/** The DS modal shell (ReportBug): a title bar with close over a hugging content column. */
export function TitleBarModal({ open, title, onClose, children }: Props) {
  const titleId = useId();
  return (
    <Dialog
      bare
      open={open}
      onClose={onClose}
      className={styles.paper}
      backdropClassName={styles.backdrop}
      aria-labelledby={titleId}
    >
      <div className={styles.frame}>
        <header className={styles.bar}>
          <h2
            id={titleId}
            className={styles.barTitle}
          >
            {title}
          </h2>
          <IconButton
            className={styles.close}
            aria-label={t('modal.close')}
            onClick={onClose}
          >
            <CloseIcon />
          </IconButton>
        </header>
        <div className={styles.body}>{children}</div>
      </div>
    </Dialog>
  );
}
