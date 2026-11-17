import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { SceneParcels } from '@dcl/schemas';
import Text from '@ui/atoms/Text';

import { useSelector } from '#store';

import { config } from '/@/config';
import { t } from '/@/modules/store/translation/utils';
import { selectors as landSelectors } from '/@/modules/store/land';
import { Lands } from '/@/lib/land';
import { useEditor } from '/@/hooks/useEditor';
import { useWorkspace } from '/@/hooks/useWorkspace';
import Button from '@ui/atoms/Button';
import { Atlas } from '/@/components/WorldAtlas/Atlas';
import { useMultiplayerStorage } from '/@/hooks/useMultiplayerStorage';
import { COLORS, type Props, type Coordinate } from '../../types';
import { breaksLandPlaceContinuity, calculateParcels, parseCoords } from '../../utils';
import { PublishModal } from '../../PublishModal';
import { PlaceContinuityWarning } from '../../PlaceContinuityWarning';

import styles from './PublishToLand.module.css';

const CONTENT_SERVER = config.get('PEER_URL');

export function PublishToLand(props: Props) {
  const { project, publishScene } = useEditor();
  const { updateProject, updateSceneJson } = useWorkspace();
  const storage = useMultiplayerStorage(project?.path);
  const tiles = useSelector(state => state.land.tiles);
  const landTiles = useSelector(state => landSelectors.getLandTiles(state.land));
  const [hover, setHover] = useState<Coordinate>({ x: 0, y: 0 });
  const [placement, setPlacement] = useState<Coordinate | null>(null);
  const [initialPlacement, setInitialPlacement] = useState<Coordinate | null>(null);
  const [didAutoPlace, setDidAutoPlace] = useState(false);
  const [isChecking, setIsChecking] = useState(false);
  /** Scene waiting for the user to confirm a deployment that breaks its Place continuity. */
  const [pendingScene, setPendingScene] = useState<SceneParcels | null>(null);
  const [isPublishing, setIsPublishing] = useState(false);
  // The lookup can take up to a minute; closing the modal meanwhile must not publish.
  const isMountedRef = useRef(true);
  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
    };
  }, []);

  if (!project) return null;

  // Memoize the project parcels centered around the hover position
  const projectParcels = useMemo(() => calculateParcels(project, hover), [project, hover]);

  const placedParcels = useMemo(
    () => (placement ? calculateParcels(project, placement) : []),
    [project, placement],
  );

  const publish = useCallback(
    async (scene: SceneParcels) => {
      setIsPublishing(true);
      // Cannot deploy to a LAND with a world configuration
      const sceneUpdates = { scene, worldConfiguration: undefined };
      // Disk first: under Bevy the store update below reloads the editor at the new base, and
      // the reloaded editor reads scene.json, so it must already hold the pick.
      await updateSceneJson(project.path, sceneUpdates);
      updateProject({ ...project, ...sceneUpdates, updatedAt: Date.now() });
      // The deploy step reports a failed publish from the store (publishError).
      publishScene({ target: CONTENT_SERVER, sceneUpdates })?.catch(() => undefined);
      props.onStep('deploy');
    },
    [project, publishScene, updateProject, updateSceneJson, props.onStep],
  );

  const handleNext = useCallback(async () => {
    if (!placement) return;
    const scene: SceneParcels = {
      base: `${placement.x},${placement.y}`,
      parcels: calculateParcels(project, placement).map(({ x, y }) => `${x},${y}`),
    };

    // A redeployment keeps the scene's Place unless it changes both the base parcel and the set
    // of occupied parcels. Look up what is currently deployed where the scene is being placed.
    setIsChecking(true);
    let continuityBroken = false;
    try {
      const deployed = await new Lands().fetchLandPublishedScenes(
        calculateParcels(project, placement).map(({ x, y }): [number, number] => [x, y]),
      );
      continuityBroken = breaksLandPlaceContinuity(
        deployed.map(deployment => ({
          base: deployment.metadata?.scene?.base ?? '',
          parcels: deployment.metadata?.scene?.parcels ?? deployment.pointers ?? [],
        })),
        scene,
      );
    } catch (error) {
      // Not being able to tell must not block publishing.
      console.error('Could not check the Place continuity of the deployment', error);
    } finally {
      if (isMountedRef.current) setIsChecking(false);
    }

    if (!isMountedRef.current) return;
    if (continuityBroken) {
      setPendingScene(scene);
    } else {
      await publish(scene);
    }
  }, [placement, project, publish]);

  const handleConfirmWarning = useCallback(async () => {
    if (pendingScene && !isPublishing) await publish(pendingScene);
  }, [pendingScene, isPublishing, publish]);

  const handleCancelWarning = useCallback(() => {
    setPendingScene(null);
  }, []);

  const handleHover = useCallback((x: number, y: number) => {
    setHover({ x, y });
  }, []);

  const isHighlighted = useCallback(
    (x: number, y: number) =>
      !placement && projectParcels.some(parcel => parcel.x === x && parcel.y === y),
    [placement, projectParcels],
  );

  const isPlaced = useCallback(
    (x: number, y: number) => placedParcels.some(parcel => parcel.x === x && parcel.y === y),
    [placedParcels],
  );

  const isValid = useMemo(() => {
    return hover && projectParcels.every(({ x, y }) => !!landTiles[`${x},${y}`]);
  }, [landTiles, hover, projectParcels]);

  const strokeLayer = useCallback(
    (x: number, y: number) => {
      const placed = isPlaced(x, y);
      if (isHighlighted(x, y) || placed) {
        return {
          color: isValid || placed ? COLORS.selectedStroke : COLORS.indicatorStroke,
          scale: 1.35,
        };
      }
      return null;
    },
    [isHighlighted, isValid, isPlaced],
  );

  const highlightLayer = useCallback(
    (x: number, y: number) => {
      const placed = isPlaced(x, y);
      if (isHighlighted(x, y) || placed) {
        return { color: isValid || placed ? COLORS.selected : COLORS.indicator, scale: 1.15 };
      }
      return null;
    },
    [isHighlighted, isValid, isPlaced],
  );

  const ownedLayer = useCallback(
    (x: number, y: number) => {
      const key = `${x},${y}`;
      return landTiles[key] && landTiles[key].land.owner === tiles[key]?.owner
        ? { color: COLORS.freeParcel }
        : null;
    },
    [tiles, landTiles],
  );

  // One array per change of a layer, so the map paints again only then.
  const layers = useMemo(
    () => [ownedLayer, strokeLayer, highlightLayer],
    [ownedLayer, strokeLayer, highlightLayer],
  );

  const handlePlacement = useCallback(
    (x: number, y: number) => {
      if (!isValid) return;
      setPlacement({ x, y });
    },
    [project, isValid],
  );

  const handleClearPlacement = useCallback(() => {
    setPlacement(null);
  }, []);

  // set initial placement
  useEffect(() => {
    if (!initialPlacement) {
      const { base, parcels } = project.scene;
      // use the base parcel if it's a valid coord
      if (base in landTiles && parcels.every(parcel => parcel in landTiles)) {
        const [x, y] = parseCoords(base);
        setInitialPlacement({ x, y });
      } else {
        // if the base parcel in the scene.json is not valid (ie. it is 0,0) then select the first coord of the available tiles as the initial placement
        const available = Object.keys(landTiles);
        if (available.length > 0) {
          const [x, y] = parseCoords(available[0]);
          setInitialPlacement({ x, y });
        }
      }
    }
  }, [initialPlacement, setInitialPlacement, project, landTiles]);

  // use initial placement if possible
  useEffect(() => {
    if (didAutoPlace) return;
    if (!placement && initialPlacement) {
      const initialPlacementParcels = calculateParcels(project, initialPlacement);
      if (initialPlacementParcels.every(({ x, y }) => !!landTiles[`${x},${y}`])) {
        setPlacement(initialPlacement);
        setDidAutoPlace(true);
      }
    }
  }, [placement, setPlacement, initialPlacement, didAutoPlace, setDidAutoPlace, landTiles]);

  if (pendingScene) {
    return (
      <PublishModal
        title={t('modal.publish_project.land.action')}
        size="large"
        {...props}
        onBack={handleCancelWarning}
      >
        <div className={styles.warning}>
          <PlaceContinuityWarning
            kind="land"
            storage={storage}
          />
          <div className={styles.warningActions}>
            <Button
              variant="outline"
              size="lg"
              fullWidth
              onClick={handleCancelWarning}
              className={`hub-button hub-button--secondary hub-button--outlined ${styles.cancel}`}
            >
              {t('modal.publish_project.place_continuity.cancel')}
            </Button>
            <Button
              size="lg"
              fullWidth
              disabled={isPublishing}
              onClick={handleConfirmWarning}
              className={`hub-button hub-button--primary ${styles.action}`}
            >
              {t('modal.publish_project.place_continuity.confirm')}
            </Button>
          </div>
        </div>
      </PublishModal>
    );
  }

  return (
    <PublishModal
      title={t('modal.publish_project.land.action')}
      size="large"
      {...props}
      className={styles.modal}
    >
      <div data-testid="publish-modal-publish-to-land">
        <div
          className={styles.atlas}
          data-testid="publish-modal-publish-to-land-atlas"
        >
          <Atlas
            tiles={tiles}
            layers={layers}
            onHover={handleHover}
            onClick={handlePlacement}
            withZoomControls
            height={480}
            x={initialPlacement?.x}
            y={initialPlacement?.y}
          />
        </div>
        <div className={styles.footer}>
          <div className={styles.placement}>
            <Text
              variant="body1"
              data-testid="publish-modal-publish-to-land-placement"
            >
              {placement
                ? t('modal.publish_project.land.select_parcel.place_scene', {
                    coords: `${placement.x},${placement.y}`,
                  })
                : t('modal.publish_project.land.select_parcel.select_parcel')}
            </Text>
            {placement && (
              <Button
                variant="text"
                size="sm"
                onClick={handleClearPlacement}
                data-testid="publish-modal-publish-to-land-reset"
                className={`hub-button hub-button--primary ${styles.reset}`}
              >
                {t('modal.publish_project.land.select_parcel.actions.reset')}
              </Button>
            )}
          </div>
          <Button
            size="lg"
            onClick={handleNext}
            disabled={!placement || isChecking || isPublishing}
            data-testid="publish-modal-publish-to-land-action"
            className={`hub-button hub-button--primary ${`${styles.publish} ${styles.action}`}`}
          >
            {isChecking
              ? t('modal.publish_project.place_continuity.checking')
              : t('modal.publish_project.land.select_parcel.actions.publish')}
          </Button>
        </div>
      </div>
    </PublishModal>
  );
}
