import { useState } from 'react';
import { useNavigate } from 'react-router';
import Toggle from '@ui/atoms/Toggle';

import { useSelector } from '#store';
import { toMB } from '/@/modules/file';
import { getCurrentLocale, t } from '/@/modules/store/translation/utils';
import { getWorldDimensions } from '/@/modules/world';
import { WorldAtlas } from '/@/components/WorldAtlas';

import Button from '@ui/atoms/Button';
import { LayersIcon, LayoutOutlineIcon } from '../../Icons';
import type { RowMode } from '../../LandDetailPage/ScenesTab/SceneRow';
import { ScenesView } from '../../LandDetailPage/ScenesTab/ScenesView';
import { Meta } from '../../LandDetailPage/ScenesTab/ScenesTab.styled';
import { Loader } from '../../Loader';
import { EmptyState } from '../../ManagePage/EmptyState';
import { InfoTip } from '../../ManagePage/InfoTip';
import { WorldsYourStorageModal } from '../../Modals/WorldsYourStorageModal';
import { useWorldScenes } from './useWorldScenes';
import { usedBytes } from './worldScenes';

import styles from './ScenesTab.module.css';

type Props = { worldName: string; mode: RowMode };

/** The World's scenes: the owner manages all of them, a deploy collaborator sees the ones on their parcels. */
export function ScenesTab({ worldName, mode }: Props) {
  const navigate = useNavigate();
  const { all, scenes, deployers, ready, failed, retry, unpublishScene } = useWorldScenes(
    worldName,
    mode,
  );
  const storageStats = useSelector(state => state.management.storageStats);
  const accountHoldings = useSelector(state => state.management.accountHoldings);
  const [map, setMap] = useState(false);
  const [storageOpen, setStorageOpen] = useState(false);

  if (!ready) return <Loader size={70} />;
  if (failed)
    return (
      <EmptyState
        title={t('manage.errors.fetch.title')}
        description={t('manage.errors.fetch.description')}
        action={{ label: t('manage.errors.fetch.retry'), variant: 'tertiary', onClick: retry }}
      />
    );

  const isOwner = mode === 'manage';
  const { width, height } = getWorldDimensions(all);
  const used = getCurrentLocale().formatNumber(toMB(usedBytes(all)), {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });

  return (
    <>
      <ScenesView
        scenes={scenes}
        deployers={deployers}
        mode={mode}
        placeName={worldName}
        source="world"
        unpublishScene={unpublishScene}
        noticeFor={targets =>
          targets.length === all.length
            ? t('world_detail.scenes.last_scene', { world: worldName })
            : undefined
        }
        stats={
          <>
            <Meta>
              <LayersIcon />
              {t('manage.cards.scenes_count', { count: scenes.length })}
            </Meta>
            {width > 0 && (
              <Meta>
                <LayoutOutlineIcon />
                {t('world_detail.scenes.parcels_shape', { w: width, h: height })}
              </Meta>
            )}
            {isOwner && (
              <Meta>
                {t('world_detail.scenes.used', { size: `${used} MB` })}
                <button
                  type="button"
                  className={styles.storageLink}
                  onClick={() => setStorageOpen(true)}
                >
                  {t('world_detail.scenes.manage_storage')}
                </button>
              </Meta>
            )}
          </>
        }
        controls={
          <>
            {isOwner && (
              <span className={styles.multiScene}>
                <Toggle
                  className={styles.switch}
                  checked={all.length > 1}
                  disabled
                  ariaLabel={t('world_detail.scenes.multi_scene_title')}
                />
                {t('world_detail.scenes.multi_scene_title')}
                <InfoTip label={t('world_detail.scenes.multi_scene_info')}>
                  {t('world_detail.scenes.multi_scene_description')}
                </InfoTip>
              </span>
            )}
            <Button
              size="sm"
              className="hub-button hub-button--secondary"
              startIcon={<LayoutOutlineIcon />}
              aria-pressed={map}
              onClick={() => setMap(open => !open)}
            >
              {t('world_detail.scenes.world_map')}
            </Button>
          </>
        }
        alternate={map ? <WorldAtlas worldScenes={all} /> : undefined}
        empty={
          isOwner ? (
            <EmptyState
              title={t('world_detail.scenes.empty')}
              action={{
                label: t('manage.cards.no_scene.view_scenes'),
                variant: 'secondary',
                onClick: () => navigate('/scenes'),
              }}
            />
          ) : (
            <EmptyState title={t('world_detail.scenes.empty_filtered')} />
          )
        }
      />
      {storageStats && (
        <WorldsYourStorageModal
          open={storageOpen}
          stats={storageStats}
          accountHoldings={accountHoldings}
          onClose={() => setStorageOpen(false)}
        />
      )}
    </>
  );
}
