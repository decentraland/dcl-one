import { useCallback, useMemo } from 'react';
import Text from '@ui/atoms/Text';

import { t } from '/@/modules/store/translation/utils';
import { useDeploy } from '/@/hooks/useDeploy';
import type { Deployment as DeploymentType } from '/@/modules/store/deployment/slice';
import type { Status } from '/@/lib/deploy';
import type { Step } from '/@/components/Step/types';
import { ConnectedSteps } from '/@/components/Step';
import { Loader } from '/@/components/Loader';
import { ExpandMore } from '/@/components/ExpandMore';

import { Modal } from '..';

import type { Props } from './types';

import styles from './DeploymentHistory.module.css';

export function DeploymentHistory({ open, projectPath, onClose }: Props) {
  const { getDeployment, getDeploymentHistory } = useDeploy();
  const activeDeployment = getDeployment(projectPath);
  const history = getDeploymentHistory(projectPath);

  const sortedDeployments = useMemo(() => {
    const allDeployments = activeDeployment ? [activeDeployment, ...history] : history;
    return allDeployments.sort((a, b) => b.createdAt - a.createdAt);
  }, [activeDeployment, history]);

  return (
    <Modal
      open={open}
      title={t('modal.deployment_history.title')}
      onClose={onClose}
      size="medium"
    >
      <div className={styles.container}>
        {sortedDeployments.length === 0 ? (
          <div className={styles.noDeployments}>
            <Text variant="body2">{t('modal.deployment_history.no_deployments')}</Text>
          </div>
        ) : (
          sortedDeployments.map(deployment => (
            <Deployment
              key={deployment.id}
              deployment={deployment}
              isCurrent={activeDeployment?.id === deployment.id}
            />
          ))
        )}
      </div>
    </Modal>
  );
}

const formatDate = (timestamp: number) => {
  const date = new Date(timestamp);
  return date.toLocaleString('en-US', {
    month: '2-digit',
    day: '2-digit',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
};

const getTargetLabel = (deployment: DeploymentType) => {
  if (deployment.info.isWorld) {
    return `${t('modal.deployment_history.world')}: ${deployment.info.title || 'Unknown'}`;
  }
  return `${t('modal.deployment_history.land')}: ${deployment.info.baseParcel || 'Unknown'}`;
};

function Deployment({
  deployment,
  isCurrent,
}: {
  deployment: DeploymentType;
  isCurrent?: boolean;
}) {
  const { componentsStatus, lastUpdated, error } = deployment;
  const { deriveOverallStatus } = useDeploy();
  const overallStatus = deriveOverallStatus(deployment);

  const getStepDescription = useCallback((status: Status) => {
    switch (status) {
      case 'pending':
        return t('modal.publish_project.deploy.deploying.step.loading');
      case 'failed':
        return t('modal.publish_project.deploy.deploying.step.failed');
      default:
        return undefined;
    }
  }, []);

  const steps: Step[] = useMemo(() => {
    const { catalyst, assetBundle } = componentsStatus;
    return [
      {
        bulletText: '1',
        name: t('modal.publish_project.deploy.deploying.step.uploading'),
        description: getStepDescription(catalyst),
        state: catalyst,
      },
      {
        bulletText: '2',
        name: t('modal.publish_project.deploy.deploying.step.converting'),
        description: getStepDescription(assetBundle),
        state: assetBundle,
      },
    ];
  }, [componentsStatus, getStepDescription]);

  const title = useMemo(() => {
    if (overallStatus === 'failed') return t('modal.publish_project.deploy.deploying.failed');
    if (overallStatus === 'complete') return t('modal.publish_project.deploy.success.message');
    return t('modal.publish_project.deploy.deploying.publish');
  }, [overallStatus]);

  return (
    <div className={styles.card}>
      <div className={styles.header}>
        <div className={styles.headerLeft}>
          <div className={styles.title}>
            {overallStatus === 'failed' ? (
              <div className="Warning" />
            ) : overallStatus === 'pending' ? (
              <Loader />
            ) : null}
            <Text variant="h5">{title}</Text>
          </div>
          {error && (
            <span className={styles.error}>
              {error.message}
              {error.cause && (
                <ExpandMore
                  title={t('modal.publish_project.deploy.deploying.errors.details')}
                  text={error.cause}
                />
              )}
            </span>
          )}
          <div className={styles.meta}>
            <span>{formatDate(lastUpdated)}</span>
            <span>{getTargetLabel(deployment)}</span>
          </div>
        </div>
        {isCurrent && (
          <div className={styles.currentBadge}>{t('modal.deployment_history.current')}</div>
        )}
      </div>
      <div className={styles.steps}>
        <ConnectedSteps steps={steps} />
      </div>
    </div>
  );
}
