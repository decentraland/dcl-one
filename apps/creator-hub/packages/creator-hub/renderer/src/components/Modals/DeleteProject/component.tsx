import { useCallback, useState } from 'react';
import Checkbox from '@ui/atoms/Checkbox';

import { t } from '/@/modules/store/translation/utils';

import { Modal } from '..';
import { Button } from '../../Button';

import type { Props } from './types';

import './styles.css';

export function DeleteProject({ open, project, onClose, onSubmit }: Props) {
  const [shouldDeleteFiles, setShouldDeleteFiles] = useState(false);

  const handleSubmit = useCallback(() => {
    onSubmit(project, shouldDeleteFiles);
  }, [shouldDeleteFiles, onSubmit, project]);

  return (
    <Modal
      open={open}
      title={t('modal.delete_project.title', { title: project.title })}
      size="tiny"
      className="DeleteProjectModal"
      actions={
        <>
          <Button
            color="secondary"
            onClick={onClose}
          >
            {t('modal.cancel')}
          </Button>
          <Button onClick={handleSubmit}>{t('modal.confirm')}</Button>
        </>
      }
    >
      <Checkbox
        className="delete-files"
        checked={shouldDeleteFiles}
        onChange={setShouldDeleteFiles}
      >
        {t('modal.delete_project.files_checkbox')}
      </Checkbox>

      {shouldDeleteFiles && (
        <p className="delete-files-warning">{t('modal.delete_project.files_warning')}</p>
      )}
    </Modal>
  );
}
