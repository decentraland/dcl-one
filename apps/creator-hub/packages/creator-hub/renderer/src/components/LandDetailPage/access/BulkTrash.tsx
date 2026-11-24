import IconButton from '@ui/atoms/IconButton';

import { t } from '/@/modules/store/translation/utils';

import { DeleteIcon } from '../../Icons';

import styles from './access.module.css';

/** A list header's 30 trash, shown while rows are selected; removes the selection. */
export function BulkTrash({ onClick }: { onClick: () => void }) {
  return (
    <IconButton
      className={styles.bulkTrash}
      aria-label={t('land_detail.collaborators.remove_selected')}
      onClick={onClick}
    >
      <DeleteIcon />
    </IconButton>
  );
}
