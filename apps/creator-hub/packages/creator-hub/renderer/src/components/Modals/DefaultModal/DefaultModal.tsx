import { useId, type ReactNode } from 'react';
import Dialog from '@ui/atoms/Dialog';
import IconButton from '@ui/atoms/IconButton';
import { CloseIcon } from '@ui/atoms/icons-material';

import { t } from '/@/modules/store/translation/utils';

import { DialogButton } from '../../DialogButton';
import { Loader } from '../../Loader';

import styles from './DefaultModal.module.css';

type Action = { label: string; onClick: () => void };

type PrimaryAction = Action & { state: 'idle' | 'blocked' | 'busy' };

type Props = {
  open: boolean;
  title: string;
  onClose: () => void;
  body?: ReactNode;
  children?: ReactNode;
  secondary?: Action;
  primary?: PrimaryAction;
  initialFocus?: 'secondary' | 'content';
  corner?: 'close' | 'none';
};

/** DefaultModal's content, for a modal that swaps one step for another while staying open. */
export type ModalStep = Omit<Props, 'open'>;

/** The DS DefaultModal: title, body, optional content and a Secondary + Primary action row. */
export function DefaultModal({
  open,
  title,
  onClose,
  body,
  children,
  secondary,
  primary,
  initialFocus = 'secondary',
  corner = 'close',
}: Props) {
  const titleId = useId();
  const bodyId = useId();
  const busy = primary?.state === 'busy';

  return (
    <Dialog
      bare
      open={open}
      onClose={onClose}
      width={700}
      className={styles.paper}
      backdropClassName={styles.backdrop}
      aria-labelledby={titleId}
      aria-describedby={body ? bodyId : undefined}
    >
      <div className={styles.frame}>
        {corner === 'close' && (
          <IconButton
            className={styles.close}
            aria-label={t('modal.close')}
            onClick={onClose}
          >
            <CloseIcon />
          </IconButton>
        )}
        <div className={styles.content}>
          <h2
            id={titleId}
            className={styles.title}
          >
            {title}
          </h2>
          {(body || children) && (
            <div className={styles.group}>
              {body && (
                <div
                  id={bodyId}
                  className={styles.body}
                >
                  {body}
                </div>
              )}
              {children}
            </div>
          )}
          {(secondary || primary) && (
            <div className={styles.actions}>
              {secondary && (
                <DialogButton
                  type="button"
                  tone="secondary"
                  autoFocus={initialFocus === 'secondary'}
                  disabled={busy}
                  onClick={secondary.onClick}
                >
                  {secondary.label}
                </DialogButton>
              )}
              {primary && (
                <DialogButton
                  type="button"
                  tone="primary"
                  disabled={primary.state !== 'idle'}
                  onClick={primary.onClick}
                >
                  {busy ? <Loader size={24} /> : primary.label}
                </DialogButton>
              )}
            </div>
          )}
        </div>
      </div>
    </Dialog>
  );
}
