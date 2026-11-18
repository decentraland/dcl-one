import { useCallback, useEffect, useState } from 'react';

import { t } from '/@/modules/store/translation/utils';
import { useWorkspace } from '/@/hooks/useWorkspace';

import { SnackbarAlert } from '../Alert';
import Button from '@ui/atoms/Button';
import { MissingProjects } from '../../Modals/MissingProjects';

export function MissingScenes({ onClose }: { onClose: () => void }) {
  const [open, setOpen] = useState(false);
  const { unlistProjects, missing } = useWorkspace();

  useEffect(() => {
    if (missing.length === 0) onClose();
  }, [missing]);

  const handleModal = useCallback(
    (value: boolean) => () => {
      setOpen(value);
    },
    [],
  );

  const handleDiscardAll = useCallback(() => {
    unlistProjects(missing);
  }, []);

  const renderActions = () => (
    <>
      <Button
        variant="text"
        tone="neutral"
        size="sm"
        className="hub-button hub-button--inherit"
        onClick={handleModal(true)}
      >
        {t('snackbar.missing_projects.actions.view')}
      </Button>
      <Button
        variant="text"
        tone="neutral"
        size="sm"
        className="hub-button hub-button--inherit"
        onClick={handleDiscardAll}
      >
        {t('snackbar.missing_projects.actions.discard_all')}
      </Button>
    </>
  );

  return (
    <>
      <SnackbarAlert
        severity="error"
        action={renderActions()}
      >
        {t('snackbar.missing_projects.title', { scenes: missing.length })}
      </SnackbarAlert>
      <MissingProjects
        open={open}
        onClose={handleModal(false)}
      />
    </>
  );
}
