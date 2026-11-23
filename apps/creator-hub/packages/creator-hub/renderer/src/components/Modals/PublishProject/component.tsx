import { useCallback, useEffect, useMemo, useState } from 'react';
import { analytics } from '#preload';
import { useDispatch } from '#store';
import { fetchENSList } from '/@/modules/store/ens';
import { useAuth } from '/@/hooks/useAuth';
import { ErrorBoundary } from '/@/components/ErrorBoundary';
import { Initial } from './steps/Initial';
import { AlternativeServers } from './steps/AlternativeServers';
import { PublishToWorld } from './steps/PublishToWorld';
import { PublishToLand } from './steps/PublishToLand';
import { Deploy } from './steps/Deploy';
import { PublishErrorFallback } from './PublishErrorFallback';

import type { DeploymentMetadata, Props, Step } from './types';

export function PublishProject({
  open,
  project,
  onClose,
  initialStep = 'initial',
  disableGoBack = false,
}: Omit<Props, 'onStep' | 'deploymentMetadata'>) {
  const dispatch = useDispatch();
  const { wallet } = useAuth();
  const [history, setHistory] = useState<Step[]>([]);
  const [deploymentMetadata, setDeploymentMetadata] = useState<DeploymentMetadata>({});

  // Names can change outside the app (a purchase, a granted deploy permission), so the
  // sign-in-time fetch goes stale. Refresh in the background on every open: consumers only
  // read ens.data and fulfilled merges into it, so the current list stays visible meanwhile.
  useEffect(() => {
    if (open && wallet) {
      dispatch(fetchENSList({ address: wallet }));
    }
  }, [open, wallet, dispatch]);
  const step = useMemo<Step>(
    () => (history.length > 0 ? history[history.length - 1] : initialStep),
    [history, initialStep],
  );

  useEffect(() => {
    if (open) {
      void analytics.track('Publish Modal Opened', {
        project_id: project.id,
        initial_step: initialStep,
        signed_in: !!wallet,
      });
    }
  }, [open]);

  useEffect(() => {
    // A dialog reopened on a later step (the deploy snackbar shows the running publish) is no
    // step of the funnel; Publish Modal Opened names the step it opened on.
    const reopened = step === initialStep && initialStep !== 'initial';
    if (open && !reopened)
      void analytics.track('Publish Step Viewed', { project_id: project.id, step });
  }, [open, step]);

  const handleClose = useCallback(() => {
    void analytics.track('Publish Modal Closed', { project_id: project.id, step });
    setHistory([]);
    onClose();
  }, [onClose, project.id, step]);

  const handleBack = useCallback(() => {
    setHistory(history => (history.length > 0 ? history.slice(0, -1) : []));
  }, []);

  const handleStep: Props['onStep'] = useCallback(
    (newStep, { resetHistory = false, deploymentMetadata } = {}) => {
      setHistory(history => (resetHistory ? [newStep] : [...history, newStep]));
      if (deploymentMetadata) {
        setDeploymentMetadata(prev => ({ ...prev, ...deploymentMetadata }));
      }
    },
    [],
  );

  const previousStep = history.length > 1 ? history[history.length - 2] : undefined;

  const props: Props = {
    open,
    project,
    disableGoBack,
    previousStep,
    deploymentMetadata,
    onClose: handleClose,
    onBack: disableGoBack ? undefined : handleBack,
    onStep: handleStep,
  };

  // Keyed on `open` too: the component stays mounted while closed, so a caught error
  // would otherwise greet the user again on the next open.
  return (
    <ErrorBoundary
      name="publish-modal"
      resetKeys={[open, step]}
      fallback={({ reset }) => (
        <PublishErrorFallback
          open={open}
          onClose={handleClose}
          onRetry={reset}
        />
      )}
    >
      {step === 'initial' && <Initial {...props} />}
      {step === 'alternative-servers' && <AlternativeServers {...props} />}
      {step === 'publish-to-land' && <PublishToLand {...props} />}
      {step === 'publish-to-world' && <PublishToWorld {...props} />}
      {step === 'deploy' && <Deploy {...props} />}
    </ErrorBoundary>
  );
}
