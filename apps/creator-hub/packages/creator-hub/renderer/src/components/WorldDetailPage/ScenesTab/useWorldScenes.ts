import { useCallback, useEffect, useMemo, useState } from 'react';

import { useDispatch, useSelector } from '#store';
import { useAuth } from '/@/hooks/useAuth';
import { WorldPermissionName, WorldPermissionType } from '/@/lib/worlds';
import {
  fetchParcelsPermission,
  fetchWorldPermissions,
  fetchWorldScenes,
  unpublishWorldScene,
} from '/@/modules/store/management/slice';
import { isWorldWideDeployer, type LandScene } from '/@/modules/store/management/utils';

import type { RowMode } from '../../LandDetailPage/ScenesTab/SceneRow';
import { scenesOnParcels, toWorldScenes } from './worldScenes';

/** The World's scenes (all of them, and the rows the account may see), their deployers, how to unpublish one, and how to read them again after a failure. */
export function useWorldScenes(worldName: string, mode: RowMode) {
  const dispatch = useDispatch();
  const { wallet } = useAuth();
  const [attempt, setAttempt] = useState(0);
  const [answered, setAnswered] = useState<string>();
  const settings = useSelector(state => state.management.worldSettings);
  const permissions = useSelector(state => state.management.worldPermissions);
  const all = useMemo(
    () => (settings.worldName === worldName ? settings.scenes : []),
    [settings.worldName, settings.scenes, worldName],
  );

  // The store keeps the scenes and the status of the last visit, and a refresh after an unpublish
  // reads as loading, so the first render waits for the answer to this tab's own request.
  const request = `${attempt}:${worldName}`;
  useEffect(() => {
    let latest = true;
    void dispatch(fetchWorldScenes({ worldName })).finally(() => latest && setAnswered(request));
    return () => {
      latest = false;
    };
  }, [dispatch, worldName, request]);

  useEffect(() => {
    if (mode === 'view') void dispatch(fetchWorldPermissions({ worldName }));
  }, [dispatch, worldName, mode, attempt]);

  const summary = permissions.worldName === worldName ? permissions.summary : undefined;
  const deployment = wallet
    ? (summary?.[wallet] ?? summary?.[wallet.toLowerCase()])?.find(
        entry => entry.permission === 'deployment',
      )
    : undefined;
  const listedDeployer =
    permissions.permissions?.deployment.type === WorldPermissionType.AllowList &&
    permissions.permissions.deployment.wallets.some(
      address => address.toLowerCase() === wallet?.toLowerCase(),
    );
  const worldWide = !!summary && !!wallet && isWorldWideDeployer(summary, wallet);
  const parcels = wallet ? permissions.parcels[wallet] : undefined;
  const parcelsWanted = mode === 'view' && !!deployment && !worldWide;

  useEffect(() => {
    if (parcelsWanted && wallet)
      void dispatch(
        fetchParcelsPermission({
          worldName,
          permissionName: WorldPermissionName.Deployment,
          walletAddress: wallet,
        }),
      );
  }, [dispatch, worldName, wallet, parcelsWanted, attempt]);

  const allowed: 'all' | Set<string> | undefined =
    mode === 'manage' || worldWide || (listedDeployer && !deployment)
      ? 'all'
      : parcels?.status === 'succeeded'
        ? new Set(parcels.parcels)
        : undefined;
  const sameWorld = settings.worldName === worldName;
  // A failed scenes request leaves an empty list behind: it must not read as a World with no scenes.
  const failed =
    (sameWorld && settings.scenesStatus === 'failed') ||
    (mode === 'view' &&
      permissions.worldName === worldName &&
      (permissions.status === 'failed' || (parcelsWanted && parcels?.status === 'failed')));
  const ready =
    answered === request &&
    sameWorld &&
    (mode === 'manage' ||
      failed ||
      (permissions.worldName === worldName &&
        permissions.status === 'succeeded' &&
        (!parcelsWanted || !!allowed)));

  const rows = useMemo(() => toWorldScenes(all), [all]);
  const scenes = allowed ? scenesOnParcels(rows, allowed) : [];
  const deployers = useMemo(
    () => Object.fromEntries(all.map(scene => [scene.entityId, scene.deployer])),
    [all],
  );

  const unpublishScene = useCallback(
    (scene: LandScene) =>
      dispatch(
        unpublishWorldScene({ worldName, sceneCoord: scene.parcels[0], entityId: scene.id }),
      ).unwrap(),
    [dispatch, worldName],
  );

  const retry = useCallback(() => setAttempt(count => count + 1), []);

  return { all, scenes, deployers, ready, failed, retry, unpublishScene };
}
