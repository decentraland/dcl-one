import { cx } from '/@/lib/cx';
import Button from '@ui/atoms/Button';

import { Actions, Description } from '/@/components/ErrorBoundary';
import { t } from '/@/modules/store/translation/utils';

import { PublishModal } from './PublishModal';

import styles from './buttons.module.css';

type Props = {
  open: boolean;
  onClose: () => void;
  onRetry: () => void;
};

export function PublishErrorFallback({ open, onClose, onRetry }: Props) {
  return (
    <PublishModal
      open={open}
      size="tiny"
      title={t('error_boundary.title')}
      onClose={onClose}
    >
      <Description
        role="alert"
        variant="body1"
        data-testid="publish-error-fallback"
      >
        {t('error_boundary.modal_description')}
      </Description>
      <Actions>
        <Button
          className={styles.contained}
          variant="primary"
          tone="neutral"
          onClick={onClose}
        >
          {t('error_boundary.close')}
        </Button>
        <Button
          className={cx(styles.contained, styles.primary)}
          variant="primary"
          onClick={onRetry}
        >
          {t('error_boundary.try_again')}
        </Button>
      </Actions>
    </PublishModal>
  );
}
