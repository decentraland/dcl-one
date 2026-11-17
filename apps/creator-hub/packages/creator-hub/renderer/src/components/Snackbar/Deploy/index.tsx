import { useCallback, useMemo, useState } from 'react';
import { CloseIcon } from '/@/components/Icons/material';
import Button from '@ui/atoms/HubButton';
import IconButton from '@ui/atoms/IconButton';
import Spinner from '@ui/atoms/HubSpinner';

import { t } from '/@/modules/store/translation/utils';
import { useDeploy } from '/@/hooks/useDeploy';
import { useWorkspace } from '/@/hooks/useWorkspace';

import { PublishProject } from '/@/components/Modals/PublishProject';

import { SnackbarAlert } from '../Alert';

export function Deploy({ path, onClose }: { path: string; onClose: () => void }) {
  const { getDeployment } = useDeploy();
  const deployment = getDeployment(path);

  const sceneTitle = useMemo(() => {
    if (!deployment) return '';
    return deployment.info.title;
  }, [deployment]);

  const props = { path, title: sceneTitle, onClose };

  if (deployment?.status === 'pending') return <Pending {...props} />;
  if (deployment?.status === 'complete') return <Success {...props} />;
  if (deployment?.status === 'failed') return <Error {...props} />;

  return null;
}

function Pending({ path, title, onClose }: { path: string; title: string; onClose: () => void }) {
  const { projects } = useWorkspace();
  const project = projects.find(p => p.path === path);
  const [open, setOpen] = useState(false);

  const openModal = useCallback(() => {
    setOpen(true);
  }, []);

  const closeModal = useCallback(() => {
    setOpen(false);
  }, []);

  const renderActions = useCallback(
    () => (
      <>
        <Button
          variant="text"
          onClick={openModal}
        >
          {t('snackbar.deploy.pending.actions.view')}
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
    [onClose],
  );

  if (!project) return null;

  return (
    <>
      {!open && (
        <SnackbarAlert
          icon={
            <Spinner
              ring="wide"
              role="progressbar"
              size={20}
              color="primary"
            />
          }
          action={renderActions()}
        >
          {t('snackbar.deploy.pending.title', { title })}
        </SnackbarAlert>
      )}
      <PublishProject
        open={open}
        disableGoBack
        project={project}
        initialStep="deploy"
        onClose={closeModal}
      />
    </>
  );
}

function Success({ path, title, onClose }: { path: string; title: string; onClose: () => void }) {
  const { projects } = useWorkspace();
  const project = projects.find(p => p.path === path);
  const [open, setOpen] = useState(false);

  const openModal = useCallback(() => {
    setOpen(true);
  }, []);

  const renderActions = useCallback(
    () => (
      <>
        <Button
          variant="text"
          onClick={openModal}
        >
          {t('snackbar.deploy.success.actions.view')}
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
    [onClose],
  );

  if (!project) return null;

  return (
    <>
      {!open && (
        <SnackbarAlert
          severity="success"
          action={renderActions()}
        >
          {t('snackbar.deploy.success.title', { title })}
        </SnackbarAlert>
      )}
      <PublishProject
        open={open}
        disableGoBack
        project={project}
        initialStep="deploy"
        onClose={onClose}
      />
    </>
  );
}

function Error({ path, title, onClose }: { path: string; title: string; onClose: () => void }) {
  const { projects } = useWorkspace();
  const project = projects.find(p => p.path === path);
  const [open, setOpen] = useState(false);

  const openModal = useCallback(() => {
    setOpen(true);
  }, []);

  const renderActions = useCallback(
    () => (
      <>
        <Button
          variant="text"
          onClick={openModal}
        >
          {t('snackbar.deploy.error.actions.retry')}
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
    [onClose],
  );

  if (!project) return null;

  return (
    <>
      {!open && (
        <SnackbarAlert
          severity="error"
          action={renderActions()}
        >
          {t('snackbar.deploy.error.title', { title: `"${title}"` })}
        </SnackbarAlert>
      )}
      <PublishProject
        open={open}
        disableGoBack
        project={project}
        // initialStep="deploy"
        onClose={onClose}
      />
    </>
  );
}
