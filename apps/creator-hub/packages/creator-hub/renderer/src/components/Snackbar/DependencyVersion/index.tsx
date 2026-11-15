import { useCallback } from 'react';
import { NotificationImportantIcon, CloseIcon } from '/@/components/Icons/material';
import Button from '@ui/atoms/HubButton';
import IconButton from '@ui/atoms/IconButton';

import { t } from '/@/modules/store/translation/utils';
import { useEditor } from '/@/hooks/useEditor';
import { useWorkspace } from '/@/hooks/useWorkspace';

import { SnackbarAlert } from '../Alert';

export function NewDependencyVersion({ onClose }: { onClose: () => void }) {
  const { project, isInstallingProject } = useEditor();
  const { updatePackages } = useWorkspace();

  const handleClickUpdate = useCallback(() => {
    if (isInstallingProject) return;
    if (project) {
      updatePackages(project);
    }
    onClose();
  }, [project, isInstallingProject]);

  const renderActions = useCallback(
    () => (
      <>
        <Button
          variant="text"
          onClick={handleClickUpdate}
          disabled={isInstallingProject}
        >
          {t('snackbar.new_dependency_version.actions.update')}
        </Button>
        <IconButton
          className="MuiIconButton-root"
          aria-label={t('modal.close')}
          onClick={onClose}
        >
          <CloseIcon color="secondary" />
        </IconButton>
      </>
    ),
    [project, isInstallingProject, handleClickUpdate],
  );

  return (
    <SnackbarAlert
      icon={<NotificationImportantIcon color="secondary" />}
      severity="info"
      action={renderActions()}
    >
      {t('snackbar.new_dependency_version.title')}
    </SnackbarAlert>
  );
}
