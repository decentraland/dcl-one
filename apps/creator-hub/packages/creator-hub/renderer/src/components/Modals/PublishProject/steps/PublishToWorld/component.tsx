import { useCallback, useEffect, useId, useMemo, useRef, useState } from 'react';
import {
  AddIcon,
  ChevronRightIcon as ArrowForwardIcon,
  LayersIcon as MultiSceneIcon,
  GridOnIcon as ParcelsIcon,
  GridViewOutlinedIcon as OutlinedParcelsIcon,
  LocationOnIcon as LocationIcon,
} from '@ui/atoms/icons-material';
import type { SceneParcels, WorldConfiguration } from '@dcl/schemas';
import { Select, SelectOption } from '@ui/atoms/Select';
import Text from '@ui/atoms/Text';
import Toggle from '@ui/atoms/Toggle';

import type { Project } from '/shared/types/projects';
import FallbackThumbnail from '/assets/images/scene-thumbnail-fallback.png';
import { misc } from '#preload';
import { useDispatch, useSelector } from '#store';

import { config } from '/@/config';
import { formatWorldSize, getBaseParcel, getWorldDimensions } from '/@/modules/world';
import { t } from '/@/modules/store/translation/utils';
import { fetchENSList } from '/@/modules/store/ens';
import { ENSProvider } from '/@/modules/store/ens/types';
import { getEnsProvider } from '/@/modules/store/ens/utils';
import type { ParcelsPermission, WorldSettingsState } from '/@/modules/store/management';
import {
  actions as managementActions,
  hasWorldWidePermission,
  selectors as managementSelectors,
} from '/@/modules/store/management';
import { addBase64ImagePrefix } from '/@/modules/image';
import { useEditor } from '/@/hooks/useEditor';
import { useWorkspace } from '/@/hooks/useWorkspace';
import { useMultiplayerStorage } from '/@/hooks/useMultiplayerStorage';
import { useAuth } from '/@/hooks/useAuth';
import { WorldPermissionName, type WorldScene } from '/@/lib/worlds';
import { coordsToId } from '/@/lib/land';

import EmptyWorldSVG from '/assets/images/empty-deploy-to-world.svg';
import LogoDCLSVG from '/assets/images/logo-dcl.svg';
import LogoENSSVG from '/assets/images/logo-ens.svg';

import { Row } from '/@/components/Row';
import Button from '@ui/atoms/Button';
import { Loader } from '/@/components/Loader';
import { Image } from '/@/components/Image';
import { type AtlasScene, WorldAtlas, WorldAtlasColors } from '/@/components/WorldAtlas';
import { PublishModal } from '../../PublishModal';
import { ProjectStepWrapper } from '../../ProjectStepWrapper';
import { COLORS, type Props, type Coordinate } from '../../types';
import {
  calculateParcels,
  countOverlappedWorldScenes,
  getWorldPlaceContinuityBreak,
  parseCoords,
} from '../../utils';
import { PlaceContinuityWarning } from '../../PlaceContinuityWarning';

import './styles.css';

const WORLDS_CONTENT_SERVER = config.get('WORLDS_CONTENT_SERVER_URL');

enum Step {
  SELECTION = 'selection',
  LOCATION = 'location',
  CONFIRM_OVERWRITE = 'confirm-overwrite',
}

export function PublishToWorld(props: Props) {
  const { project, publishScene } = useEditor();
  const { updateSceneJson, updateProject } = useWorkspace();
  const { wallet } = useAuth();
  const dispatch = useDispatch();
  const names = useSelector(state => state.ens.data);
  const [name, setName] = useState<string>('');
  /** The owner's or a world-wide collaborator's own pick on the switch; null keeps the default. */
  const [multiSceneChoice, setMultiSceneChoice] = useState<boolean | null>(null);
  const worldSettings = useSelector(managementSelectors.getWorldSettings);
  const worldPermissions = useSelector(state =>
    managementSelectors.getParcelsStateForAddress(state, wallet || ''),
  );
  const [step, setStep] = useState<Step>(Step.SELECTION);
  /** Step the confirmation was reached from, so going back returns to it. */
  const [confirmOrigin, setConfirmOrigin] = useState<Step>(Step.LOCATION);
  /**
   * Parcels of the deployment awaiting confirmation. Kept apart from the store project: the
   * inspector re-sends its own (older) scene metadata after scene.json is written, which would
   * make the confirmation describe the wrong parcels.
   */
  const [pendingScene, setPendingScene] = useState<SceneParcels | null>(null);

  /**
   * The NAME list has four outcomes and they are not interchangeable. Deriving everything
   * from `names` being empty made "still loading", "the subgraph failed" and "you own no
   * NAMEs" render the same screen — so a slow load looked like an empty account, and a
   * `.zone` subgraph outage looked like one too. `status` is what tells them apart.
   */
  const namesStatus = useSelector(state => state.ens.status);
  const hasNames = Object.keys(names).length > 0;
  const isLoadingNames = !hasNames && (namesStatus === 'idle' || namesStatus === 'loading');
  const namesFailed = !hasNames && namesStatus === 'failed';
  const emptyNames = !hasNames && namesStatus === 'succeeded';

  const handleRetryNames = useCallback(() => {
    if (wallet) dispatch(fetchENSList({ address: wallet }));
  }, [dispatch, wallet]);

  const isOwner: boolean = useMemo(() => {
    if (!name || !wallet) return false;
    return names[name]?.nftOwnerAddress?.toLocaleLowerCase() === wallet.toLocaleLowerCase();
  }, [names, name, wallet]);

  const hasWorldWidePermissions: boolean = useMemo(() => {
    return isOwner || hasWorldWidePermission(worldPermissions);
  }, [isOwner, worldPermissions]);

  // Derived, not synced by an effect: re-picking the world already in the store changes neither
  // the permissions nor the scene count, so an effect keyed on them never re-ran and the reset
  // to single scene stuck (a restricted collaborator, whose switch is locked, saw the
  // "lose its Places listing" warning instead of the parcel picker).
  // A restricted collaborator always has to choose a location.
  const isMultiSceneEnabled =
    !hasWorldWidePermissions || (multiSceneChoice ?? worldSettings.scenes.length > 1);

  const handleBack = useCallback(() => {
    if (step === Step.CONFIRM_OVERWRITE) {
      setStep(confirmOrigin);
    } else if (step === Step.LOCATION) {
      setStep(Step.SELECTION);
    } else {
      props.onBack?.();
    }
  }, [step, confirmOrigin, props.onBack]);

  const loadWorld = useCallback(
    (worldName: string) => {
      dispatch(managementActions.fetchWorldSettings({ worldName }));
      dispatch(managementActions.fetchWorldScenes({ worldName }));
      dispatch(
        managementActions.fetchParcelsPermission({
          worldName,
          permissionName: WorldPermissionName.Deployment,
          walletAddress: wallet || '',
        }),
      );
    },
    [wallet],
  );

  const handleChangeName = useCallback(
    (worldName: string) => {
      setMultiSceneChoice(null);
      setName(worldName);
      loadWorld(worldName);
    },
    [loadWorld],
  );

  const handleRetryLoadWorld = useCallback(() => loadWorld(name), [loadWorld, name]);

  const worldLoadFailed =
    !!name &&
    (worldSettings.status === 'failed' ||
      worldSettings.scenesStatus === 'failed' ||
      (!isOwner && worldPermissions?.status === 'failed'));

  // Single vs multi scene is decided from the scenes and the permissions, so the next step
  // waits for both instead of deciding (and warning) on what is still loading.
  const worldLoading =
    !!name &&
    (worldSettings.status === 'loading' ||
      worldSettings.scenesStatus === 'loading' ||
      (!isOwner && worldPermissions?.status === 'loading'));

  // Every choice made across the steps (world name, then location), so the publish can
  // put all of them in the scene.json it deploys rather than only the last step's.
  const chosenUpdates = useRef<Partial<Project>>({});

  const handleUpdateProject = useCallback(
    async (projectUpdates: Partial<Project>) => {
      if (!project) return;
      chosenUpdates.current = { ...chosenUpdates.current, ...projectUpdates };
      await updateSceneJson(project.path, projectUpdates);
      updateProject({ ...project, ...projectUpdates, updatedAt: Date.now() });
    },
    [project, updateSceneJson, updateProject],
  );

  const handlePublish = useCallback(
    async (projectUpdates?: Partial<Project>) => {
      if (projectUpdates) await handleUpdateProject(projectUpdates);
      const { worldConfiguration, scene } = chosenUpdates.current;
      // The deploy step reports a failed publish from the store (publishError).
      publishScene({
        targetContent: WORLDS_CONTENT_SERVER,
        sceneUpdates: {
          ...(worldConfiguration && { worldConfiguration }),
          ...(scene && { scene }),
        },
      })?.catch(() => undefined);
      props.onStep('deploy', { deploymentMetadata: { isMultiScene: isMultiSceneEnabled } });
    },
    [isMultiSceneEnabled, props.onStep, publishScene, handleUpdateProject],
  );

  const handleSelectLocation = useCallback(
    async (projectUpdates: Partial<Project>) => {
      if (projectUpdates) await handleUpdateProject(projectUpdates);
      setStep(Step.LOCATION);
    },
    [handleUpdateProject],
  );

  const handleShowConfirmation = useCallback(
    async (projectUpdates: Partial<Project>, origin: Step = Step.LOCATION) => {
      setPendingScene(projectUpdates.scene ?? null);
      if (projectUpdates) await handleUpdateProject(projectUpdates);
      setConfirmOrigin(origin);
      setStep(Step.CONFIRM_OVERWRITE);
    },
    [handleUpdateProject],
  );

  useEffect(() => {
    // Initialize the name and world settings when the project is loaded
    // We use handleChangeName to also fetch the world settings, scenes and permissions
    if (project?.worldConfiguration?.name && names[project.worldConfiguration.name]) {
      handleChangeName(project.worldConfiguration.name);
    }
  }, []);

  return (
    <PublishModal
      title={t('modal.publish_project.worlds.select_world.title')}
      data-testid="publish-modal-publish-to-world"
      size="large"
      {...props}
      onBack={handleBack}
    >
      {isLoadingNames ? (
        <LoadingNames />
      ) : namesFailed ? (
        <NamesError onRetry={handleRetryNames} />
      ) : emptyNames ? (
        <EmptyNames />
      ) : hasNames && project ? (
        <ProjectStepWrapper
          isWorld
          name={name}
          project={project}
        >
          {worldLoadFailed && (
            <div className="WorldLoadError">
              <Text
                variant="body2"
                tone="error"
              >
                {t('modal.publish_project.worlds.select_world.load_error')}
              </Text>
              <Button
                variant="outline"
                size="sm"
                className="hub-button hub-button--secondary hub-button--outlined"
                onClick={handleRetryLoadWorld}
              >
                {t('modal.publish_project.worlds.select_world.actions.retry')}
              </Button>
            </div>
          )}
          {step === Step.SELECTION && (
            <SelectWorld
              name={name}
              project={project}
              worldSettings={worldSettings}
              isMultiSceneEnabled={isMultiSceneEnabled}
              onChangeMultiScene={setMultiSceneChoice}
              hasWorldWidePermissions={hasWorldWidePermissions}
              worldLoadFailed={worldLoadFailed}
              worldLoading={worldLoading}
              onChangeName={handleChangeName}
              onSelectLocation={handleSelectLocation}
              onShowConfirmation={updates => handleShowConfirmation(updates, Step.SELECTION)}
              onPublish={handlePublish}
            />
          )}
          {step === Step.LOCATION && (
            <SelectLocation
              project={project}
              worldScenes={worldSettings.scenes}
              worldPermissions={worldPermissions}
              hasWorldWidePermissions={hasWorldWidePermissions}
              onShowConfirmation={handleShowConfirmation}
              onPublish={handlePublish}
            />
          )}
          {step === Step.CONFIRM_OVERWRITE && (
            <ConfirmOverwrite
              parcels={pendingScene?.parcels ?? project.scene.parcels}
              worldScenes={worldSettings.scenes}
              isMultiScene={isMultiSceneEnabled}
              onCancel={handleBack}
              onConfirm={() => handlePublish()}
            />
          )}
        </ProjectStepWrapper>
      ) : null}
    </PublishModal>
  );
}

function SelectWorld({
  name,
  project,
  worldSettings,
  hasWorldWidePermissions,
  worldLoadFailed,
  worldLoading,
  isMultiSceneEnabled,
  onChangeMultiScene,
  onChangeName,
  onSelectLocation,
  onShowConfirmation,
  onPublish,
}: {
  name: string;
  project: Project;
  worldSettings: WorldSettingsState;
  hasWorldWidePermissions: boolean;
  worldLoadFailed: boolean;
  worldLoading: boolean;
  isMultiSceneEnabled: boolean;
  onChangeMultiScene: (isMultiSceneEnabled: boolean) => void;
  onChangeName: (name: string) => void;
  onSelectLocation: (projectUpdates: Partial<Project>) => void;
  onShowConfirmation: (projectUpdates: Partial<Project>) => void;
  onPublish: (projectUpdates: Partial<Project>) => Promise<void>;
}) {
  const names = useSelector(state => state.ens.data);
  const descriptionId = useId();
  const multiSceneLabelId = useId();
  // A list opened with the pointer shows no active row until a key is pressed.
  const [menuKeys, setMenuKeys] = useState(false);
  const menuClassName = menuKeys
    ? 'SelectWorld-SelectMenu'
    : 'SelectWorld-SelectMenu SelectWorld-SelectMenu-Pointer';
  const handleMenuKeyDown = useCallback(() => setMenuKeys(true), []);
  const handleMenuPointerDown = useCallback(() => setMenuKeys(false), []);
  const handleMenuOpenChange = useCallback((open: boolean) => {
    if (!open) setMenuKeys(false);
  }, []);
  const [ensProvider, setENSProvider] = useState(
    project.worldConfiguration?.name
      ? getEnsProvider(project.worldConfiguration?.name)
      : ENSProvider.DCL,
  );

  const worldSize = useMemo(() => {
    const { width, height } = getWorldDimensions(worldSettings.scenes || []);
    return width !== 0 && height !== 0
      ? formatWorldSize({ width, height })
      : formatWorldSize({ width: project.layout.cols, height: project.layout.rows });
  }, [worldSettings.scenes]);

  const listNames = useMemo(() => {
    const _names = [];
    for (const ens in names) {
      if (names[ens].provider === ensProvider) {
        _names.push(names[ens].subdomain);
      }
    }
    return _names;
  }, [names, ensProvider]);

  const handleClaimNewName = useCallback(() => {
    if (ensProvider === ENSProvider.DCL) {
      misc.openExternal('https://decentraland.org/marketplace/names/claim');
    } else {
      misc.openExternal('https://ens.domains');
    }
  }, [ensProvider]);

  const handleChangeSelectProvider = useCallback(
    (provider: ENSProvider) => {
      setENSProvider(provider);
      onChangeName('');
    },
    [onChangeName],
  );

  const handleChangeSelectName = useCallback(
    (worldName: string) => {
      if (!worldName) return;
      if (worldName === CLAIM_NAME_VALUE) return handleClaimNewName();
      onChangeName(worldName);
    },
    [onChangeName, handleClaimNewName],
  );

  const handleNext = useCallback(async () => {
    const worldConfiguration: WorldConfiguration = { ...project.worldConfiguration, name };

    if (isMultiSceneEnabled && worldSettings.scenes.length > 0) {
      onSelectLocation({ worldConfiguration });
    } else {
      // In single scene mode, ensure the project base parcel is 0,0.
      const scene: SceneParcels = {
        base: coordsToId(0, 0),
        parcels: calculateParcels(project, { x: 0, y: 0 }).map(parcel =>
          coordsToId(parcel.x, parcel.y),
        ),
      };
      if (getWorldPlaceContinuityBreak(worldSettings.scenes, scene.parcels, isMultiSceneEnabled)) {
        onShowConfirmation({ worldConfiguration, scene });
      } else {
        onPublish({ worldConfiguration, scene });
      }
    }
  }, [
    project,
    name,
    isMultiSceneEnabled,
    worldSettings.scenes,
    onPublish,
    onSelectLocation,
    onShowConfirmation,
  ]);

  // Only an in-flight thumbnail save blocks publishing (so the deploy doesn't
  // race the file write). `status` stays undefined when no thumbnail was ever
  // captured — the Bevy renderer's screenshot is best-effort and may never
  // dispatch the save — and a failed save is likewise no reason to block:
  // the thumbnail is decorative, the deploy works without it.
  const projectIsReady = project.status !== 'loading';
  const claimLabel =
    ensProvider === ENSProvider.DCL
      ? t('modal.publish_project.worlds.select_world.claim_new_name')
      : t('modal.publish_project.worlds.select_world.claim_new_ens_domain');

  return (
    <div
      className="SelectWorld"
      data-testid="publish-modal-publish-to-world-select-world"
    >
      <div
        className="selection"
        onKeyDown={handleMenuKeyDown}
        onPointerDown={handleMenuPointerDown}
      >
        <Text
          id={descriptionId}
          variant="h6"
          className="StepDescription"
        >
          {t('modal.publish_project.worlds.select_world.description')}
        </Text>
        <Row>
          <div
            className="SelectWorld-ENSProvider"
            data-testid="publish-modal-publish-to-world-select-world-ens-provider"
          >
            <Select
              variant="outlined"
              fullWidth
              placement="bottom"
              menuClassName={menuClassName}
              onOpenChange={handleMenuOpenChange}
              icon={selectArrow}
              aria-label={t('modal.publish_project.worlds.select_world.domain_type_label')}
              aria-describedby={descriptionId}
              value={ensProvider}
              onChange={handleChangeSelectProvider}
              renderValue={provider => <ENSProviderLabel provider={provider} />}
            >
              {ENS_PROVIDERS.map(provider => (
                <SelectOption
                  key={provider}
                  value={provider}
                  label={t(`modal.publish_project.worlds.select_world.ens_providers.${provider}`)}
                >
                  <span
                    className="SelectWorld-ENSProvider-Option"
                    data-testid={`publish-modal-publish-to-world-select-world-ens-provider-${provider}`}
                  >
                    <ENSProviderLabel provider={provider} />
                  </span>
                </SelectOption>
              ))}
            </Select>
          </div>
          <div
            className="SelectWorld-WorldName"
            data-testid="publish-modal-publish-to-world-select-world-select"
          >
            <Select
              variant="outlined"
              fullWidth
              placement="bottom"
              menuClassName={menuClassName}
              onOpenChange={handleMenuOpenChange}
              icon={selectArrow}
              aria-label={t('modal.publish_project.worlds.select_world.world_name_label')}
              aria-describedby={descriptionId}
              value={name}
              onChange={handleChangeSelectName}
              disabled={listNames.length === 0}
              placeholder={<em>{t('modal.publish_project.worlds.select_world.placeholder')}</em>}
              endAdornment={worldSettings.status === 'loading' ? <Loader size={20} /> : null}
              renderValue={selected => selected}
            >
              <SelectOption
                key="placeholder"
                value=""
                disabled
                className={name ? undefined : 'SelectWorld-NamePlaceholder'}
              >
                <em>{t('modal.publish_project.worlds.select_world.placeholder')}</em>
              </SelectOption>
              {listNames.map((_world: string) => (
                <SelectOption
                  key={_world}
                  value={_world}
                  label={_world}
                >
                  <span
                    data-testid={`publish-modal-publish-to-world-select-world-select-item-${_world.toLowerCase()}`}
                  >
                    {_world}
                  </span>
                </SelectOption>
              ))}
              <SelectOption
                key="claim"
                value={CLAIM_NAME_VALUE}
                label={claimLabel}
                className="SelectWorld-ClaimName"
              >
                <AddIcon />
                {claimLabel}
              </SelectOption>
            </Select>
          </div>
        </Row>
      </div>
      {name && (
        <div className="AdvancedSettings">
          <Row>
            <MultiSceneIcon />
            <div className="AdvancedSettingsHeaderTexts">
              <Text id={multiSceneLabelId}>
                {t('modal.publish_project.worlds.select_world.multi_scene.title')}
              </Text>
              <Text
                variant="body2"
                className="AdvancedSettingsDescription"
              >
                {t('modal.publish_project.worlds.select_world.multi_scene.description')}
              </Text>
            </div>
            <Toggle
              checked={isMultiSceneEnabled}
              onChange={onChangeMultiScene}
              disabled={!hasWorldWidePermissions}
              ariaLabelledBy={multiSceneLabelId}
            />
          </Row>
          {isMultiSceneEnabled && worldSettings.status !== 'loading' && (
            <div className="WorldInfo">
              <div className="WorldThumbnail">
                <Image
                  src={
                    worldSettings.settings?.thumbnail ||
                    addBase64ImagePrefix(project.thumbnail) ||
                    ''
                  }
                  alt={worldSettings.settings?.title || ''}
                  fallbackSrc={FallbackThumbnail}
                />
              </div>
              <div className="WorldInfoContent">
                <Text className="CurrentWorldLabel">
                  {t('modal.publish_project.worlds.select_world.multi_scene.current_world')}
                </Text>
                <Text
                  variant="h6"
                  className="WorldTitle"
                >
                  {worldSettings.settings?.title || project.title}
                </Text>
                <Row className="WorldMetadata">
                  <InfoItem
                    icon={<MultiSceneIcon />}
                    label={t('modal.publish_project.worlds.select_world.multi_scene.scenes_count', {
                      count: worldSettings.scenes?.length || 1,
                    })}
                  />
                  <InfoItem
                    icon={<ParcelsIcon />}
                    label={worldSize}
                  />
                </Row>
              </div>
            </div>
          )}
        </div>
      )}
      <div className="actions">
        <Button
          variant="text"
          size="lg"
          className="hub-button hub-button--primary"
          data-testid="publish-modal-publish-to-world-select-world-action"
          onClick={handleNext}
          disabled={!projectIsReady || !name || worldLoadFailed || worldLoading}
          endIcon={<ArrowForwardIcon />}
        >
          {isMultiSceneEnabled && worldSettings.scenes.length > 0
            ? t('modal.publish_project.worlds.select_world.actions.select_location')
            : t('modal.publish_project.worlds.select_world.actions.review')}
        </Button>
      </div>
    </div>
  );
}

const ENS_PROVIDERS = [ENSProvider.DCL, ENSProvider.ENS];
// The value of the claim row of the name select. A World name has no colon.
const CLAIM_NAME_VALUE = ':claim';

const selectArrow = (
  <svg
    className="SelectWorld-SelectArrow"
    viewBox="0 0 24 24"
    focusable="false"
    aria-hidden="true"
  >
    <path d="M7 10l5 5 5-5z" />
  </svg>
);
const ENS_PROVIDER_LOGOS = { [ENSProvider.DCL]: LogoDCLSVG, [ENSProvider.ENS]: LogoENSSVG };

function ENSProviderLabel({ provider }: { provider: ENSProvider }) {
  return (
    <>
      <img
        className="SelectWorld-ENSProvider-Img"
        src={ENS_PROVIDER_LOGOS[provider]}
      />
      {t(`modal.publish_project.worlds.select_world.ens_providers.${provider}`)}
    </>
  );
}

function InfoItem({ icon, label }: { icon: React.ReactNode; label: string }) {
  if (!label) return null;
  return (
    <div className="WorldInfoItem">
      {icon}
      <Text variant="body2">{label}</Text>
    </div>
  );
}

function SelectLocation({
  worldScenes,
  worldPermissions,
  project,
  hasWorldWidePermissions,
  onPublish,
  onShowConfirmation,
}: {
  worldScenes: WorldScene[];
  worldPermissions?: ParcelsPermission;
  project: Project;
  hasWorldWidePermissions: boolean;
  onPublish: (projectUpdates: Partial<Project>) => void;
  onShowConfirmation: (projectUpdates: Partial<Project>) => void;
}) {
  const permissionsSet = useMemo(
    () => new Set(worldPermissions?.parcels ?? []),
    [worldPermissions?.parcels],
  );
  const [hover, setHover] = useState<Coordinate | null>(null);
  const [placement, setPlacement] = useState<Coordinate | null>(getInitialPlacement);

  const noParcelsFit: boolean = useMemo(() => {
    // TODO: in the future we should make this more accurate by checking if the project parcels shape fits in the available parcels shape.
    return (
      !placement && permissionsSet.size > 0 && permissionsSet.size < project.scene.parcels?.length
    );
  }, [placement, permissionsSet, project.scene.parcels]);

  function getInitialPlacement(): Coordinate | null {
    if (project.scene.base) {
      // If project has a base parcel, check if the user has permission to deploy into it
      // before setting the initial placement.
      const [x, y] = parseCoords(project.scene.base);
      if (
        hasWorldWidePermissions ||
        calculateParcels(project, { x, y }).every(parcel =>
          permissionsSet.has(coordsToId(parcel.x, parcel.y)),
        )
      ) {
        return { x, y };
      }
    }
    if (hasWorldWidePermissions && !worldScenes.length) {
      return { x: 0, y: 0 };
    }
    return null;
  }

  const atlasInitialCenter: Coordinate | null = useMemo(() => {
    if (placement) {
      return placement;
    } else if (worldPermissions?.parcels.length) {
      // If the user has parcel-specific permissions, center the atlas on the first parcel available.
      const [x, y] = parseCoords(worldPermissions.parcels[0]);
      return { x, y };
    }
    return null;
  }, []); // Just calculate the initial center once when the component mounts.

  const sceneParcelsSet = useMemo(
    () => new Set(worldScenes.flatMap(scene => scene.parcels ?? [])),
    [worldScenes],
  );

  // Memoize the project parcels centered around the hover position only if not placed
  const hoverProjectParcels = useMemo(
    () =>
      new Set(
        hover ? calculateParcels(project, hover).map(parcel => coordsToId(parcel.x, parcel.y)) : [],
      ),
    [project, hover, placement],
  );

  const placedProjectParcels = useMemo(
    () =>
      new Set(
        placement
          ? calculateParcels(project, placement).map(parcel => coordsToId(parcel.x, parcel.y))
          : [],
      ),
    [project, placement],
  );

  /** World scenes list with the placement parcel added. */
  const newWorldScenes: AtlasScene[] = useMemo(() => {
    if (!placement) return worldScenes;
    return [...worldScenes, { parcels: Array.from(placedProjectParcels) }];
  }, [worldScenes, placement, placedProjectParcels]);

  const isValidHoverPlacement = useMemo(() => {
    return (
      hover &&
      (hasWorldWidePermissions ||
        Array.from(hoverProjectParcels).every(parcel => worldPermissions?.parcels.includes(parcel)))
    );
  }, [hasWorldWidePermissions, worldPermissions, hover, hoverProjectParcels]);

  const isHoveredParcel = useCallback(
    (x: number, y: number) => hoverProjectParcels.has(coordsToId(x, y)),
    [hoverProjectParcels],
  );

  const isPlacedParcel = useCallback(
    (x: number, y: number) => placedProjectParcels.has(coordsToId(x, y)),
    [placedProjectParcels],
  );

  const isOverlappingScenes = useMemo(() => {
    return Array.from(placedProjectParcels).some(parcel => sceneParcelsSet.has(parcel));
  }, [placedProjectParcels, sceneParcelsSet]);

  /** Parcels where the user has or not deployment permission if they are collaborators. */
  const permissionsLayer = useCallback(
    (x: number, y: number) =>
      hasWorldWidePermissions || worldPermissions?.parcels.includes(coordsToId(x, y))
        ? null // Available parcel color is applied by default.
        : { color: WorldAtlasColors.noPermissionParcel, scale: 1.1 }, // Darken the parcel to make it "not available".
    [hasWorldWidePermissions, worldPermissions],
  );

  const placedStrokeLayer = useCallback(
    (x: number, y: number) =>
      isPlacedParcel(x, y) ? { color: COLORS.selectedStroke, scale: 1.35 } : null,
    [isPlacedParcel],
  );

  const placedHighlightLayer = useCallback(
    (x: number, y: number) =>
      isPlacedParcel(x, y) ? { color: COLORS.selected, scale: 1.15 } : null,
    [isPlacedParcel],
  );

  const hoverLayer = useCallback(
    (x: number, y: number) => {
      return isHoveredParcel(x, y)
        ? { color: isValidHoverPlacement ? COLORS.selected : COLORS.indicator, scale: 1.1 }
        : null;
    },
    [isHoveredParcel, isValidHoverPlacement],
  );

  const handleHover = useCallback((x: number, y: number) => {
    setHover({ x, y });
  }, []);

  const handleSelectParcel = useCallback(
    (x: number, y: number) => {
      if (isValidHoverPlacement) {
        setPlacement({ x, y });
      }
    },
    [isValidHoverPlacement],
  );

  const handleReset = useCallback(() => {
    setPlacement(null);
    setHover(null);
  }, []);

  const handleNext = useCallback(async () => {
    if (!placement) return;
    const scene: SceneParcels = {
      base: coordsToId(placement.x, placement.y),
      parcels: Array.from(placedProjectParcels),
    };
    // Multi-scene mode: only an overlap can break the Place continuity, and an overlap already
    // goes through the confirmation.
    if (isOverlappingScenes) {
      onShowConfirmation({ scene });
    } else {
      onPublish({ scene });
    }
  }, [placement, placedProjectParcels, isOverlappingScenes, onShowConfirmation, onPublish]);

  return (
    <div className="SelectLocation">
      <Text
        variant="h6"
        className="StepDescription"
      >
        {t('modal.publish_project.worlds.select_world.location.description')}
      </Text>
      <WorldAtlas
        layers={[permissionsLayer, placedStrokeLayer, placedHighlightLayer, hoverLayer]}
        onHover={handleHover}
        onClick={handleSelectParcel}
        worldScenes={newWorldScenes}
        colorsOverride={!hasWorldWidePermissions ? { availableParcel: '#43404a' } : undefined}
        floatingContent={
          permissionsSet.size > 0 && (
            <div className="AvailableParcelsCount">
              <OutlinedParcelsIcon />
              <Text variant="body2">
                {t('modal.publish_project.worlds.select_world.location.available_parcels_count', {
                  count: permissionsSet.size,
                })}
              </Text>
            </div>
          )
        }
        height={350}
        x={atlasInitialCenter?.x}
        y={atlasInitialCenter?.y}
      />
      <div className="actions">
        {noParcelsFit && (
          <Text
            variant="body2"
            tone="error"
          >
            {t('modal.publish_project.worlds.select_world.location.no_parcels_fit_error')}
          </Text>
        )}

        {placement && (
          <Button
            variant="outline"
            size="lg"
            className="hub-button hub-button--secondary hub-button--outlined"
            onClick={handleReset}
          >
            {t('modal.publish_project.worlds.select_world.actions.reset')}
          </Button>
        )}
        {placement && project.scene.parcels?.length > 0 && (
          <Text className="SelectedParcelInfo">
            {t('modal.publish_project.worlds.select_world.location.parcels_place', {
              count: project.scene.parcels.length,
            })}
            <LocationIcon />
            {`${placement.x}, ${placement.y}`}
          </Text>
        )}
        <Button
          variant="text"
          size="lg"
          className="hub-button hub-button--primary"
          onClick={handleNext}
          disabled={!placement || noParcelsFit}
          endIcon={<ArrowForwardIcon />}
        >
          {t('modal.publish_project.worlds.select_world.actions.review')}
        </Button>
      </div>
    </div>
  );
}

function ConfirmOverwrite({
  parcels,
  worldScenes,
  isMultiScene,
  onCancel,
  onConfirm,
}: {
  parcels: string[];
  worldScenes: WorldScene[];
  isMultiScene: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const { project } = useEditor();
  const storage = useMultiplayerStorage(project?.path);
  const replacedScenes = useMemo(() => {
    return worldScenes.filter(scene => {
      const sceneParcels = new Set(scene.parcels);
      return parcels.some(parcel => sceneParcels.has(parcel));
    });
  }, [worldScenes, parcels]);

  const overlap = useMemo(
    () => countOverlappedWorldScenes(worldScenes, parcels),
    [worldScenes, parcels],
  );
  const continuityBreak = getWorldPlaceContinuityBreak(worldScenes, parcels, isMultiScene);
  const hasReplacedScenes = replacedScenes.length > 0;

  return (
    <div className="ConfirmOverwrite">
      {hasReplacedScenes ? (
        <>
          <Text variant="h5">
            {t('modal.publish_project.worlds.select_world.confirm_overwrite.title')}
          </Text>
          <Text variant="body2">
            {t('modal.publish_project.worlds.select_world.confirm_overwrite.scenes_count', {
              count: replacedScenes.length,
              b: (child: string) => <b>{child}</b>,
            })}
          </Text>
        </>
      ) : null}
      <ul className="ScenesList">
        {replacedScenes.map((scene, index) => (
          <li key={scene.entityId}>
            <Text variant="body2">
              <b>
                {scene.entity.metadata?.display?.title ||
                  t(
                    'modal.publish_project.worlds.select_world.confirm_overwrite.scene_title_placeholder',
                    { index: index + 1 },
                  )}{' '}
              </b>
              <span>({getBaseParcel(scene.parcels || [])?.join(', ') || ''})</span>
            </Text>
          </li>
        ))}
      </ul>
      {hasReplacedScenes && (
        <Text variant="body2">
          {t('modal.publish_project.worlds.select_world.confirm_overwrite.description')}
        </Text>
      )}
      {continuityBreak === 'merge' && (
        <PlaceContinuityWarning
          kind="world-merge"
          overlap={overlap}
          storage={storage}
        />
      )}
      {continuityBreak === 'replace-world' && (
        <PlaceContinuityWarning
          kind="world-replace"
          storage={storage}
        />
      )}
      <div className="actions">
        <Button
          variant="text"
          size="lg"
          className="hub-button hub-button--secondary"
          fullWidth
          onClick={onCancel}
        >
          {t('modal.publish_project.worlds.select_world.actions.cancel')}
        </Button>
        <Button
          variant="text"
          size="lg"
          className="hub-button hub-button--primary"
          fullWidth
          onClick={onConfirm}
        >
          {t('modal.publish_project.worlds.select_world.actions.confirm')}
        </Button>
      </div>
    </div>
  );
}

/** Shown while the NAME list is still being fetched, so a slow load is not mistaken for an
 *  account that owns nothing. */
function LoadingNames() {
  return (
    <div
      className="LoadingNames"
      data-testid="publish-modal-names-loading"
    >
      <Loader />
      <Text
        variant="h6"
        align="center"
      >
        {t('modal.publish_project.worlds.loading_names.title')}
      </Text>
    </div>
  );
}

/** Shown when the NAME lookup failed (typically a subgraph outage). Without this the user
 *  saw "you have no Worlds" and had no way to retry. */
function NamesError({ onRetry }: { onRetry: () => void }) {
  return (
    <div
      className="NamesError"
      data-testid="publish-modal-names-error"
    >
      <Text
        variant="h6"
        align="center"
      >
        {t('modal.publish_project.worlds.names_error.title')}
      </Text>
      <Text
        variant="body2"
        align="center"
      >
        {t('modal.publish_project.worlds.names_error.description')}
      </Text>
      <Button
        variant="text"
        className="hub-button hub-button--primary"
        data-testid="publish-modal-names-error-retry"
        onClick={onRetry}
      >
        {t('modal.publish_project.worlds.names_error.action')}
      </Button>
    </div>
  );
}

function EmptyNames() {
  const handleClick = useCallback(() => {
    misc.openExternal('https://decentraland.org/marketplace/names/claim');
  }, []);

  const handleClickLearnMore = useCallback(() => {
    misc.openExternal(
      'https://docs.decentraland.org/creator/worlds/about/#worlds-from-decentraland-names',
    );
  }, []);

  return (
    <div
      className="EmptyNames"
      data-testid="publish-modal-empty-names"
    >
      <Text
        variant="h6"
        align="center"
      >
        {t('modal.publish_project.worlds.empty_names.title')}
      </Text>
      <img
        className="thumbnail"
        src={EmptyWorldSVG}
      />
      <Text
        variant="body2"
        align="center"
      >
        {t('modal.publish_project.worlds.empty_names.description', {
          b: (child: string) => <b>{child}</b>,
          br: () => <br />,
        })}
      </Text>
      <div className="actions">
        <Button
          variant="text"
          size="sm"
          className="hub-button hub-button--primary"
          onClick={handleClick}
        >
          {t('modal.publish_project.worlds.empty_names.action')}
        </Button>
        <Button
          variant="text"
          size="sm"
          className="hub-button hub-button--secondary"
          onClick={handleClickLearnMore}
        >
          {t('option_box.learn_more')}
        </Button>
      </div>
    </div>
  );
}
