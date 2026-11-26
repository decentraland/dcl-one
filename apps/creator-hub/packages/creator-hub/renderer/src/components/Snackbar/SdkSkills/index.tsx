import { useCallback } from 'react';
import { NotificationImportantIcon, CloseIcon } from '@ui/atoms/icons-material';
import Button from '@ui/atoms/Button';
import IconButton from '@ui/atoms/IconButton';

import type { Project } from '/shared/types/projects';
import { t } from '/@/modules/store/translation/utils';
import { useWorkspace } from '/@/hooks/useWorkspace';

import { SnackbarAlert } from '../Alert';

export function SdkSkills({
  project,
  installed,
  onClose,
}: {
  project: Project;
  installed: boolean;
  onClose: () => void;
}) {
  const { updateSdkSkills } = useWorkspace();

  const handleClickAction = useCallback(() => {
    updateSdkSkills(project);
    onClose();
  }, [project, updateSdkSkills, onClose]);

  return (
    <SnackbarAlert
      icon={<NotificationImportantIcon tone="secondary" />}
      severity="info"
      action={
        <>
          <Button
            variant="text"
            onClick={handleClickAction}
          >
            {installed
              ? t('snackbar.sdk_skills.outdated.action')
              : t('snackbar.sdk_skills.missing.action')}
          </Button>
          <IconButton
            aria-label={t('modal.close')}
            onClick={onClose}
          >
            <CloseIcon tone="secondary" />
          </IconButton>
        </>
      }
    >
      {installed ? t('snackbar.sdk_skills.outdated.title') : t('snackbar.sdk_skills.missing.title')}
    </SnackbarAlert>
  );
}
