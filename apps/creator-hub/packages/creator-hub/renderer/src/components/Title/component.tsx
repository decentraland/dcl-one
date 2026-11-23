import { cx } from '/@/lib/cx';
import IconButton from '@ui/atoms/IconButton';
import Text from '@ui/atoms/Text';
import { ArrowBackIosNewRoundedIcon } from '@ui/atoms/icons-material';

import { t } from '/@/modules/store/translation/utils';

import type { Props } from './types';
import styles from './component.module.css';

export function Title({ id, value, onBack }: Props) {
  return (
    <div className={cx('Title', styles.root)}>
      {onBack && (
        <IconButton
          className={styles.back}
          tone="primary"
          aria-label={t('title.back')}
          onClick={onBack}
        >
          <ArrowBackIosNewRoundedIcon fontSize="inherit" />
        </IconButton>
      )}
      <Text
        id={id}
        className={styles.heading}
        variant="h3"
        as="h1"
      >
        {value}
      </Text>
    </div>
  );
}
