import { type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Link } from 'react-router';
import { worldsManagementUrl } from '/shared/urls';
import { config } from '/@/config';
import { cx } from '/@/lib/cx';
import Checkbox from '@ui/atoms/Checkbox';
import Text from '@ui/atoms/Text';

import { analytics, misc } from '#preload';
import { useDispatch, useSelector } from '#store';

import { type File, type Info, type Status } from '/@/lib/deploy';

import { useAuth } from '/@/hooks/useAuth';
import { useWorkspace } from '/@/hooks/useWorkspace';
import { useEditor } from '/@/hooks/useEditor';
import { useSnackbar } from '/@/hooks/useSnackbar';
import { useDeploy } from '/@/hooks/useDeploy';
import { useCounter } from '/@/hooks/useCounter';
import { useSceneSizeLimit } from '/@/hooks/useSceneSizeLimit';

import { actions as deploymentActions, type Deployment } from '/@/modules/store/deployment/slice';
import { isFinalPhase } from '/@/modules/store/deployment/deploy-api';
import {
  getInvalidFiles,
  getTotalSize,
  MAX_FILE_SIZE_BYTES,
} from '/@/modules/store/deployment/utils';
import {
  actions as managementActions,
  selectors as managementSelectors,
} from '/@/modules/store/management/slice';

import { t } from '/@/modules/store/translation/utils';
import { HELP_FAQ_URL, getSceneJumpInDeeplink, getSceneJumpInUrl } from '/@/modules/utils';
import { getProjectWorldName } from '/@/lib/project-world';
import { formatSize } from '/@/modules/file';

import { PublishModal } from '/@/components/Modals/PublishProject/PublishModal';
import { ConnectedSteps } from '/@/components/Step';
import Button from '@ui/atoms/Button';
import { Loader } from '/@/components/Loader';
import { ExpandMore } from '/@/components/ExpandMore';
import { ReportIssue } from '/@/components/Modals/ReportIssue';
import { ExternalLink } from '/@/components/ExternalLink';
import { CheckBoxIcon, CheckBoxOutlineBlankIcon } from '@ui/atoms/icons-material';

import type { Step } from '/@/components/Step/types';
import type { Props } from '/@/components/Modals/PublishProject/types';
import { ProjectStepWrapper } from '/@/components/Modals/PublishProject/ProjectStepWrapper';
import { parseCoords } from '/@/components/Modals/PublishProject/utils';

import { DeployProgress, type DeployCancel } from './DeployProgress';
import styles from './Deploy.module.css';

import './styles.css';

const MAX_FILE_PATH_LENGTH = 50;

function getPath(filename: string) {
  return filename.length > MAX_FILE_PATH_LENGTH
    ? `${filename.slice(0, MAX_FILE_PATH_LENGTH / 2)}...${filename.slice(
        -(MAX_FILE_PATH_LENGTH / 2),
      )}`
    : filename;
}

export function Deploy(props: Props) {
  const { project, previousStep, onStep, onBack, onClose, deploymentMetadata } = props;
  const { signOut, wallet } = useAuth();
  const dispatch = useDispatch();
  const { updateProjectInfo } = useWorkspace();
  const { loadingPublish, publishError } = useEditor();
  const { getDeployment, executeDeployment } = useDeploy();
  const { pushCustom } = useSnackbar();
  const worldScenes = useSelector(managementSelectors.getWorldScenes);
  const names = useSelector(state => state.ens.data);
  const [showWarning, setShowWarning] = useState(false);
  const [skipWarning, setSkipWarning] = useState(project.info.skipPublishWarning ?? false);
  const [undeployStatus, setUndeployStatus] = useState<Status>('idle');
  const [reportOpen, setReportOpen] = useState(false);
  const deployment = getDeployment(project.path);
  const isWorld = previousStep === 'publish-to-world' || !!deployment?.info.isWorld;
  const worldName = deploymentMetadata?.worldName || getProjectWorldName(project);
  const ownsWorld =
    !!wallet &&
    !!worldName &&
    names[worldName]?.nftOwnerAddress?.toLowerCase() === wallet.toLowerCase();

  /** True if the project needs to undeploy existing world scenes before deploying the new one.
   * True when deploying to a single-scene world and there are existing scenes in the world that do not overlap with the project parcels. */
  const needsUndeploy = useMemo(() => {
    const projectParcelsSet = new Set(project.scene.parcels);
    return (
      isWorld &&
      deploymentMetadata?.isMultiScene === false &&
      worldScenes.length > 0 &&
      worldScenes.some(scene => scene.parcels.every(parcel => !projectParcelsSet.has(parcel)))
    );
  }, [isWorld, deploymentMetadata?.isMultiScene, worldScenes, project.scene.parcels]);

  const worldsUrl = config.get('WORLDS_CONTENT_SERVER_URL');
  const managesCanonicalWorlds = worldsManagementUrl(worldsUrl) !== worldsUrl;
  // An independent .dev upload cannot replace an entire .org World atomically.
  // Require an explicit removal through Manage for layouts that need a whole-World delete.
  const requiresManualRemoval = needsUndeploy && managesCanonicalWorlds;
  /** Publishing empties the world first: the same condition `handlePublish` undeploys on. */
  const willEmptyWorld = needsUndeploy && !managesCanonicalWorlds && !!wallet && !!worldName;

  // Only the idle view shows the limit. A deployment whose publish server never answered
  // (initializeDeployment rejected, e.g. offline) is failed and carries no parcels.
  const { limit: sceneSizeLimit, isLoading: isSceneSizeLimitLoading } = useSceneSizeLimit(
    deployment?.status === 'idle'
      ? {
          isWorld,
          worldName,
          parcels: deployment.info.parcels,
          replacesWorld: willEmptyWorld,
          deploymentId: deployment.id,
        }
      : undefined,
  );

  /** True if any of the project parcels overlap with any of the existing world scenes parcels */
  const isReplacingWorldContent: boolean = useMemo(() => {
    if (!isWorld) return false;
    const projectParcels = project.scene.parcels;
    const worldScenesParcels = worldScenes.map(scene => scene.parcels).flat();
    return projectParcels.some(parcel => worldScenesParcels.includes(parcel));
  }, [isWorld, worldScenes, project.scene.parcels]);

  const handlePublish = useCallback(async () => {
    if (requiresManualRemoval) return;
    setShowWarning(false);
    updateProjectInfo(project.path, { skipPublishWarning: skipWarning }); // write skip warning flag

    if (willEmptyWorld && wallet && worldName) {
      setUndeployStatus('pending');
      try {
        await dispatch(
          managementActions.unpublishEntireWorld({
            worldName,
          }),
        ).unwrap();
        setUndeployStatus('complete');
        await dispatch(managementActions.fetchWorldScenes({ worldName }));
      } catch {
        setUndeployStatus('failed');
        return; // Stop here if undeploy fails
      }
    }

    executeDeployment(project.path);
  }, [
    skipWarning,
    project,
    requiresManualRemoval,
    willEmptyWorld,
    worldName,
    wallet,
    executeDeployment,
    updateProjectInfo,
  ]);

  const handleBack = useCallback(() => {
    setShowWarning(false);
    setSkipWarning(false);
  }, []);

  const canCancel =
    !!deployment?.supportsDeployResource &&
    deployment.status === 'pending' &&
    !!deployment.phase &&
    !isFinalPhase(deployment.phase);

  const handleCancel = useCallback(
    async () =>
      (await dispatch(deploymentActions.cancelDeployment(project.path)).unwrap()).rejection,
    [dispatch, project.path],
  );

  const cancel = useMemo<DeployCancel>(() => {
    if (!canCancel) return { kind: 'hidden' };
    if (!deployment.hasCancelToken) return { kind: 'unsupported' };
    if (deployment.cancelRequested) return { kind: 'cancelling' };
    return { kind: 'available', onCancel: handleCancel };
  }, [canCancel, deployment?.hasCancelToken, deployment?.cancelRequested, handleCancel]);

  // A cancelled deployment leaves the CLI unable to deploy again, so leave this step: the
  // previous step starts a fresh one. Only on the transition, as a deployment cancelled
  // earlier is still in the store when this step mounts again.
  const previousStatus = useRef(deployment?.status);
  useEffect(() => {
    const wasPending = previousStatus.current === 'pending';
    previousStatus.current = deployment?.status;
    if (!wasPending || deployment?.status !== 'cancelled') return;
    if (previousStep && onBack) {
      onBack();
    } else {
      onClose();
    }
  }, [deployment?.status, previousStep, onBack, onClose]);

  const jumpInLocation = useMemo(() => {
    const [x, y] = parseCoords(deployment?.info.baseParcel ?? project.scene.base);
    const world = deployment?.info.isWorld && worldName ? worldName : undefined;
    return { world, x, y };
  }, [deployment, project, worldName]);

  /** Web link shown and copied for sharing: it opens for people without the desktop client. */
  const shareUrl = useMemo(() => getSceneJumpInUrl(jumpInLocation), [jumpInLocation]);

  /** Jump In opens the installed desktop client, or the web link when there is none. */
  const handleJumpIn = useCallback(async () => {
    const opened = await misc
      .openDeepLink(getSceneJumpInDeeplink(jumpInLocation))
      .catch(() => false);
    if (!opened) void misc.openExternal(shareUrl);
    void analytics.track('Jump In Published Scene', {
      project_id: project.id,
      is_world: !!jumpInLocation.world,
      opened_in: opened ? 'client' : 'web',
    });
  }, [jumpInLocation, shareUrl, project.id]);

  const handleReportIssue = useCallback(() => setReportOpen(true), []);

  const handleDeployRetry = useCallback(() => {
    onBack && onBack();
  }, [onBack]);

  const handleGoToSignIn = useCallback(() => {
    signOut();
    onStep('initial', { resetHistory: true });
  }, []);

  const handleClose = useCallback(() => {
    // push snackbar notification if deploy is still pending when closing the modal
    if (deployment?.status === 'pending') {
      pushCustom({ type: 'deploy', path: project.path }, { duration: 0, requestId: project.path });
    }
    props.onClose();
  }, [deployment, pushCustom, project.path]);

  const title = useMemo(() => {
    if (!deployment && loadingPublish) {
      return t('modal.publish_project.deploy.loading');
    }

    switch (previousStep) {
      case 'publish-to-world':
        return t('modal.publish_project.deploy.world');
      case 'publish-to-land':
        return t('modal.publish_project.deploy.land');
      case 'alternative-servers':
        return t('modal.publish_project.deploy.server');
      default:
        return isWorld
          ? t('modal.publish_project.deploy.world')
          : t('modal.publish_project.deploy.land');
    }
  }, [deployment, loadingPublish, previousStep, isWorld]);

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
    const stepsList: Step[] = [];
    let stepNumber = 1;

    if ((needsUndeploy && !managesCanonicalWorlds) || undeployStatus !== 'idle') {
      stepsList.push({
        bulletText: stepNumber++,
        name: t('modal.publish_project.deploy.deploying.step.unpublishing'),
        description: getStepDescription(undeployStatus),
        state: undeployStatus,
        testId: 'publish-modal-deploy-step-unpublishing',
      });
    }

    if (!deployment) return stepsList;

    const { catalyst, assetBundle } = deployment.componentsStatus;
    stepsList.push({
      bulletText: stepNumber++,
      name: t('modal.publish_project.deploy.deploying.step.uploading'),
      description: getStepDescription(catalyst),
      state: catalyst,
      testId: 'publish-modal-deploy-step-uploading',
    });
    stepsList.push({
      bulletText: stepNumber++,
      name: t('modal.publish_project.deploy.deploying.step.converting'),
      description: getStepDescription(assetBundle),
      state: assetBundle,
      testId: 'publish-modal-deploy-step-converting',
    });

    return stepsList;
  }, [
    deployment?.componentsStatus,
    getStepDescription,
    needsUndeploy,
    managesCanonicalWorlds,
    undeployStatus,
  ]);

  const hasError =
    publishError || !deployment || deployment.status === 'failed' || undeployStatus === 'failed';

  return (
    <PublishModal
      title={title}
      size="large"
      {...props}
      onClose={handleClose}
      onBack={props.disableGoBack || deployment?.status === 'complete' ? undefined : onBack}
    >
      <div
        className="Deploy"
        data-testid="publish-modal-deploy"
      >
        {showWarning ? (
          <div
            className="publish-warning"
            data-testid="publish-modal-deploy-warning"
          >
            <div className="content">
              <div className="Warning" />
              <div className="message">
                {t('modal.publish_project.deploy.warning.message_replacing_world_content', {
                  ul: (child: string) => <ul>{child}</ul>,
                  li: (child: string) => <li>{child}</li>,
                })}
              </div>
            </div>
            <div className="actions">
              <Checkbox
                className="dont-show-again"
                icon={<CheckBoxOutlineBlankIcon />}
                checkedIcon={<CheckBoxIcon />}
                checked={skipWarning}
                onChange={setSkipWarning}
              >
                {t('modal.publish_project.deploy.warning.checkbox')}
              </Checkbox>
              <span className="buttons">
                <Button
                  variant="outline"
                  className="hub-button hub-button--secondary hub-button--outlined"
                  onClick={handleBack}
                >
                  {t('modal.publish_project.deploy.warning.back')}
                </Button>
                <Button
                  variant="text"
                  className="hub-button hub-button--primary"
                  data-testid="publish-modal-deploy-warning-continue-button"
                  onClick={handlePublish}
                >
                  {t('modal.publish_project.deploy.warning.continue')}
                </Button>
              </span>
            </div>
          </div>
        ) : (
          <ProjectStepWrapper
            isWorld={isWorld}
            project={project}
            name={worldName}
            className="scene"
          >
            {loadingPublish ? (
              <div className="header Loading">
                <Loader />
                <Text variant="h5">{t('modal.publish_project.deploy.deploying.publish')}</Text>
              </div>
            ) : hasError ? (
              <Error
                errorMessage={deployment?.error?.message}
                errorCause={publishError || deployment?.error?.cause}
                errorType={
                  publishError
                    ? 'code_error'
                    : deployment?.error?.name === 'INVALID_IDENTITY'
                      ? 'identity_error'
                      : 'deployment_error'
                }
                steps={steps}
                onRetry={handleDeployRetry}
                onReportIssue={handleReportIssue}
                goToSignIn={handleGoToSignIn}
              />
            ) : (
              <>
                {deployment.status === 'idle' && undeployStatus === 'idle' && (
                  <Idle
                    files={deployment.files}
                    error={deployment.error}
                    publishBlockedReason={
                      requiresManualRemoval
                        ? t(
                            'modal.publish_project.deploy.deploying.errors.canonical_removal_required',
                          )
                        : undefined
                    }
                    sceneSizeLimit={sceneSizeLimit}
                    isSceneSizeLimitLoading={isSceneSizeLimitLoading}
                    onClick={() =>
                      skipWarning || !(isReplacingWorldContent || needsUndeploy)
                        ? handlePublish()
                        : setShowWarning(true)
                    }
                  />
                )}
                {(deployment.status === 'pending' || undeployStatus === 'pending') && (
                  <Deploying steps={steps}>
                    <DeployProgress
                      key={deployment.id}
                      upload={deployment.upload}
                      cancel={cancel}
                    />
                  </Deploying>
                )}
                {deployment.status === 'complete' && (
                  <Success
                    info={deployment.info}
                    url={shareUrl}
                    worldName={deployment.info.isWorld && ownsWorld ? worldName : undefined}
                    onClick={handleJumpIn}
                    onClose={props.onClose}
                  />
                )}
              </>
            )}
          </ProjectStepWrapper>
        )}
      </div>
      <ReportIssue
        open={reportOpen}
        source="publish-failure"
        associatedEventId={deployment?.error?.eventId}
        onClose={() => setReportOpen(false)}
      />
    </PublishModal>
  );
}

type IdleProps = {
  files: File[];
  error?: Deployment['error'];
  publishBlockedReason?: string;
  sceneSizeLimit: number | null;
  isSceneSizeLimitLoading?: boolean;
  onClick: () => void;
};

export function Idle({
  files,
  error,
  publishBlockedReason,
  sceneSizeLimit,
  isSceneSizeLimitLoading = false,
  onClick,
}: IdleProps) {
  const invalidFiles = getInvalidFiles(files);
  const totalSize = getTotalSize(files);
  const exceedsSceneSize = sceneSizeLimit !== null && totalSize > sceneSizeLimit;
  const errorMessage =
    publishBlockedReason ??
    (invalidFiles.length > 0
      ? t('modal.publish_project.deploy.deploying.errors.max_file_size_exceeded', {
          maxFileSizeInMb: MAX_FILE_SIZE_BYTES / 1e6,
        })
      : exceedsSceneSize
        ? t('modal.publish_project.deploy.deploying.errors.max_scene_size_exceeded')
        : error?.message);

  return (
    <div
      className="files"
      data-testid="publish-modal-deploy-publish-button"
    >
      <div className="filters">
        <div className="count">
          {t('modal.publish_project.deploy.files.count', { count: files.length })}
        </div>
        <div className={cx('size', { invalid: exceedsSceneSize })}>
          {sceneSizeLimit === null
            ? t('modal.publish_project.deploy.files.size', {
                size: formatSize(totalSize),
                b: (child: string) => <b>{child}</b>,
              })
            : t('modal.publish_project.deploy.files.size_with_limit', {
                size: formatSize(totalSize),
                limit: formatSize(sceneSizeLimit),
                b: (child: string) => <b>{child}</b>,
              })}
          {isSceneSizeLimitLoading && (
            <span
              className="limit-loading"
              role="status"
            >
              {t('modal.publish_project.deploy.files.checking_limit')}
            </span>
          )}
        </div>
      </div>
      <div className="list">
        {files.map(file => (
          <div
            className={cx('file', { invalid: file.size > MAX_FILE_SIZE_BYTES })}
            key={file.name}
          >
            <div
              className="filename"
              title={file.name}
            >
              {getPath(file.name)}
            </div>
            <div className="size">{formatSize(file.size)}</div>
          </div>
        ))}
      </div>
      <div className="actions">
        <p className="error">{errorMessage}</p>
        <Button
          variant="text"
          size="lg"
          className="hub-button hub-button--primary"
          onClick={onClick}
          disabled={
            !!publishBlockedReason || !!error || exceedsSceneSize || isSceneSizeLimitLoading
          }
        >
          {t('modal.publish_project.deploy.files.publish')}
          <i className="deploy-icon" />
        </Button>
      </div>
    </div>
  );
}

type DeployingProps = {
  steps: Step[];
  children?: ReactNode;
};

function Deploying({ steps, children }: DeployingProps) {
  return (
    <div
      className="Deploying"
      data-testid="publish-modal-deploy-deploying"
    >
      <div className="header">
        <Loader />
        <Text variant="h5">{t('modal.publish_project.deploy.deploying.publish')}</Text>
      </div>
      <ConnectedSteps steps={steps} />
      {children}
    </div>
  );
}

type ErrorProps = {
  errorMessage?: string;
  errorCause?: string;
  errorType: 'code_error' | 'identity_error' | 'deployment_error';
  steps: Step[];
  onRetry: () => void;
  onReportIssue: () => void;
  goToSignIn: () => void;
};

function Error({
  errorMessage,
  errorCause,
  errorType,
  steps,
  onRetry,
  onReportIssue,
  goToSignIn,
}: ErrorProps) {
  const { count, start } = useCounter(5, { onComplete: goToSignIn });

  useEffect(() => {
    if (errorType === 'identity_error') {
      start(5);
    }
  }, [errorType, start]);

  const renderErrorMessage = useCallback(
    (errorMessage: string | undefined) => {
      switch (errorType) {
        case 'code_error':
          return t('modal.publish_project.deploy.deploying.errors.code_error');
        case 'identity_error':
          return `${errorMessage} ${t('modal.publish_project.deploy.deploying.redirect.sign_in', { seconds: count })}`;
        default:
          return errorMessage || t('modal.publish_project.deploy.deploying.errors.failed');
      }
    },
    [count, errorType],
  );

  return (
    <div
      className="Error"
      data-testid="publish-modal-deploy-error"
    >
      <div className="header">
        <div className="Warning" />
        <Text variant="h5">{t('modal.publish_project.deploy.deploying.failed')}</Text>
      </div>
      <p className="description">{renderErrorMessage(errorMessage)}</p>
      {errorCause && (
        <ExpandMore
          title={t('modal.publish_project.deploy.deploying.errors.details')}
          text={errorCause}
        />
      )}
      {errorType !== 'code_error' && <ConnectedSteps steps={steps} />}
      <div className="actions">
        <ExternalLink
          className={styles.helpCenterLink}
          href={HELP_FAQ_URL}
        >
          {t('modal.report_issue.help_center')}
        </ExternalLink>
        <Button
          variant="outline"
          size="lg"
          className="hub-button hub-button--secondary hub-button--outlined"
          onClick={onReportIssue}
        >
          {t('modal.publish_project.deploy.deploying.actions.report_issue')}
        </Button>
        <Button
          variant="text"
          size="lg"
          className="hub-button hub-button--primary"
          data-testid="publish-modal-deploy-error-retry"
          onClick={onRetry}
        >
          {t('modal.publish_project.deploy.deploying.actions.retry')}
        </Button>
      </div>
    </div>
  );
}

type SuccessProps = {
  info: Info;
  url: string;
  worldName?: string;
  onClick: () => void;
  onClose: () => void;
};

/** The publish result: the share URL, Jump In and, for a World, a link to its Settings tab. */
export function Success({ info, url, worldName, onClick, onClose }: SuccessProps) {
  return (
    <div
      className="Success"
      data-testid="publish-modal-deploy-success"
    >
      <div className="content">
        <i className="success-icon" />
        <div className="message">{t('modal.publish_project.deploy.success.message')}</div>
        <JumpUrl
          info={info}
          url={url}
        />
        {worldName && (
          <Link
            className={styles.settingsLink}
            to={`/manage/world/${encodeURIComponent(worldName)}/settings`}
            onClick={onClose}
          >
            {t('modal.publish_project.deploy.success.world_settings')}
          </Link>
        )}
        <div className="indexing-note">
          {t('modal.publish_project.deploy.success.indexing_note')}
        </div>
      </div>
      <div className="actions">
        <Button
          variant="text"
          size="lg"
          className="hub-button hub-button--primary"
          onClick={onClick}
        >
          {t('modal.publish_project.deploy.success.jump_in')}
          <i className="jump-in-icon" />
        </Button>
      </div>
    </div>
  );
}

function JumpUrl({ info, url }: { info: Info; url: string }) {
  return (
    <div className="jump-in-url">
      <label>
        {t('modal.publish_project.deploy.success.url', {
          target: info.isWorld
            ? t('modal.publish_project.deploy.success.world')
            : t('modal.publish_project.deploy.success.land'),
        })}
      </label>
      <div className="url">
        {url}
        <i
          className="copy-icon"
          onClick={() => misc.copyToClipboard(url)}
        />
      </div>
    </div>
  );
}
