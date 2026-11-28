import { useCallback, useEffect, useMemo, useState } from 'react';
import type { PBAvatarShape } from '@dcl/ecs';
import { useDrop } from 'react-dnd';
import { AiOutlinePlus as AddIcon } from 'react-icons/ai';
import { VscClose as RemoveIcon, VscDiscard as ResetIcon } from 'react-icons/vsc';

import { withSdk } from '../../../hoc/withSdk';
import { useHasComponent } from '../../../hooks/sdk/useHasComponent';
import { useComponentValue } from '../../../hooks/sdk/useComponentValue';
import { useRiggedWearableModels } from '../../../hooks/catalog/useRiggedWearableModels';
import { analytics, Event } from '../../../lib/logic/analytics';
import { withAssetDir } from '../../../lib/data-layer/host/fs-utils';
import { DropTypesEnum, type LocalAssetDrop } from '../../../lib/sdk/drag-drop';
import { CoreComponents } from '../../../lib/sdk/components';
import { PLAYER } from '../../../lib/sdk/tree';
import type { WearableCategory } from '../../../lib/logic/wearables';
import {
  BODY_SHAPE_A,
  BODY_SHAPE_B,
  DEFAULT_BODY_SHAPE,
  MAX_WEARABLES,
  getSceneWearableCategory,
  getSceneWearableName,
  isSceneWearablePath,
  isWearableUrn,
  swapBaseOutfit,
} from '../../../lib/logic/wearables';
import { useAppSelector } from '../../../redux/hooks';
import { selectAssetCatalog } from '../../../redux/app';
import type { AssetNodeItem } from '../../ProjectAssetExplorer/types';
import { isWearableAsset } from '../../ProjectAssetExplorer/utils';
import { Block } from '../../Block';
import { Container } from '../../Container';
import { CheckboxField, ColorPicker, InfoTooltip, TextField } from '../../ui';
import { toColor3, toHex } from '../../ui/ColorField/utils';
import { WearablePreview } from '../../WearablePreview';
import type { AnimationItem, WearableItem } from './catalog';
import { resolveWearables } from './catalog';
import { AddWearableModal } from './AddWearableModal';
import { AnimationField } from './AnimationField';
import { CategoryIcon } from './CategoryIcon';
import { useRenameSceneFile } from './useRenameSceneFile';
import { useSceneThumbnails } from './useSceneThumbnails';
import type { Color3 } from './utils';
import {
  DEFAULT_EYE_COLOR,
  DEFAULT_HAIR_COLOR,
  DEFAULT_SKIN_COLOR,
  addWearable,
  getNametagSuffix,
  getPreviewUrns,
  getSlotFileName,
  getWornBySlot,
  nextExpressionTimestamp,
  removeWearableAt,
  toPreviewColor,
  toWearableRows,
} from './utils';
import type { Props } from './types';

import styles from './AvatarShapeInspector.module.css';

const SOURCE_LABELS: Record<WearableItem['source'], string> = {
  scene: 'Scene',
  base: 'Base',
  catalog: 'Catalog',
};

const COLOR_FIELDS = [
  { field: 'skinColor', label: 'Skin', fallback: DEFAULT_SKIN_COLOR },
  { field: 'hairColor', label: 'Hair', fallback: DEFAULT_HAIR_COLOR },
  { field: 'eyeColor', label: 'Eyes', fallback: DEFAULT_EYE_COLOR },
] as const;

type ColorKey = (typeof COLOR_FIELDS)[number]['field'];

const EMPTY_CATALOG = { basePath: '', assets: [] };

function stem(path: string) {
  return (path.split('/').pop() ?? path).replace(/\.glb$/i, '');
}

export default withSdk<Props>(({ sdk, entity, initialOpen = true }) => {
  const { AvatarShape } = sdk.components;
  // On the Player, AvatarShape changes each visitor's own avatar: an unset field keeps their
  // profile, and the fields only an NPC uses are left out.
  const isPlayer = entity === PLAYER;
  const hasAvatarShape = useHasComponent(entity, AvatarShape);
  const [value, setValue] = useComponentValue<PBAvatarShape>(entity, AvatarShape);
  const files = useAppSelector(selectAssetCatalog) ?? EMPTY_CATALOG;
  const renameSceneFile = useRenameSceneFile();
  const [resolved, setResolved] = useState<ReadonlyMap<string, WearableItem>>(new Map());
  const [isAdding, setIsAdding] = useState(false);

  const wearables = value?.wearables ?? [];
  const wearablesKey = wearables.join('\n');
  const urns = useMemo(() => wearables.filter(isWearableUrn), [wearablesKey]);

  useEffect(() => {
    if (!urns.length) return;
    let current = true;
    void resolveWearables(urns).then(items => {
      if (!current) return;
      setResolved(previous => {
        const next = new Map(previous);
        items.forEach(item => item && next.set(item.id, item));
        return next;
      });
    });
    return () => {
      current = false;
    };
  }, [urns]);

  const paths = useMemo(() => files.assets.map($ => $.path), [files]);
  const rigged = useRiggedWearableModels(paths);
  const wearablePaths = useMemo(
    () => [...paths.filter(isSceneWearablePath), ...rigged.keys()],
    [paths, rigged],
  );
  const thumbnails = useSceneThumbnails(wearablePaths);

  const rows = useMemo(
    () =>
      toWearableRows(wearables, resolved).map(row =>
        row.source === 'scene' ? { ...row, thumbnail: thumbnails.get(row.value) } : row,
      ),
    [wearablesKey, resolved, thumbnails],
  );
  const worn = useMemo(() => getWornBySlot(rows), [rows]);

  const sceneWearables = useMemo<WearableItem[]>(() => {
    const named = paths.filter(isSceneWearablePath).map(path => ({
      id: path,
      name: getSceneWearableName(path),
      category: getSceneWearableCategory(path)!,
      source: 'scene' as const,
      thumbnail: thumbnails.get(path),
    }));
    const unnamed = [...rigged].map(([path, model]) => ({
      id: path,
      name: stem(path),
      category: model.suggestedCategory ?? 'upper_body',
      fitsAnySlot: !model.suggestedCategory,
      needsSlotName: true,
      source: 'scene' as const,
      thumbnail: thumbnails.get(path),
    }));
    return [...named, ...unnamed];
  }, [paths, rigged, thumbnails]);
  const sceneAnimations = useMemo<AnimationItem[]>(
    () =>
      paths
        .filter($ => /_emote\.glb$/i.test($))
        .map(path => ({ id: path, name: stem(path).replace(/_emote$/i, ''), source: 'scene' })),
    [paths],
  );

  const categoryOf = useCallback(
    (item: string): WearableCategory | undefined =>
      isWearableUrn(item)
        ? (resolved.get(item)?.category ?? rows.find($ => $.value === item)?.category)
        : getSceneWearableCategory(item),
    [resolved, rows],
  );

  const update = useCallback(
    (changes: Partial<PBAvatarShape>) => setValue({ ...value, ...changes }),
    [value, setValue],
  );

  const handleRemove = useCallback(async () => {
    sdk.operations.removeComponent(entity, AvatarShape);
    await sdk.operations.dispatch();
    analytics.track(Event.REMOVE_COMPONENT, { componentName: CoreComponents.AVATAR_SHAPE });
  }, [sdk, entity, AvatarShape]);

  const putOn = useCallback(
    async (item: Pick<WearableItem, 'id' | 'category' | 'needsSlotName'>) => {
      let id = item.id;
      if (item.needsSlotName) {
        // explorers read a scene wearable's slot from its file name
        const renamed = await renameSceneFile(id, getSlotFileName(id, item.category));
        if (!renamed) return;
        id = renamed;
      }
      update({ wearables: addWearable(wearables, { id, category: item.category }, categoryOf) });
    },
    [renameSceneFile, update, wearables, categoryOf],
  );

  const handleSelect = useCallback(
    (item: Pick<WearableItem, 'id' | 'category' | 'needsSlotName'>) => {
      setIsAdding(false);
      void putOn(item);
    },
    [putOn],
  );

  const [{ isOver, canDrop }, drop] = useDrop(
    () => ({
      accept: [DropTypesEnum.LocalAsset],
      canDrop: ({ value: path, context }: LocalAssetDrop) => {
        const node = context.tree.get(path) as AssetNodeItem | undefined;
        return node?.type === 'asset' && isWearableAsset(node.asset);
      },
      drop: ({ value: path, context }: LocalAssetDrop) => {
        const node = context.tree.get(path) as AssetNodeItem | undefined;
        if (node?.type !== 'asset' || !isWearableAsset(node.asset)) return;
        const file = withAssetDir(node.asset.src);
        const named = getSceneWearableCategory(file);
        if (named) {
          void putOn({ id: file, category: named });
        } else if (node.asset.riggedWearable?.suggestedCategory) {
          void putOn({
            id: file,
            category: node.asset.riggedWearable.suggestedCategory,
            needsSlotName: true,
          });
        } else {
          // a rigged model that names no slot: pick one in the dialog
          setIsAdding(true);
        }
      },
      collect: monitor => ({ isOver: monitor.isOver(), canDrop: monitor.canDrop() }),
    }),
    [putOn],
  );

  if (!hasAvatarShape || !value) return null;

  const bodyShape = value.bodyShape || (isPlayer ? '' : DEFAULT_BODY_SHAPE);
  const previewUrns = getPreviewUrns(rows, bodyShape || DEFAULT_BODY_SHAPE);
  const hasSceneWearables = rows.some($ => $.source === 'scene');
  const bodyOptions = [
    ...(isPlayer ? [{ urn: '', label: 'Profile' }] : []),
    { urn: BODY_SHAPE_A, label: 'Body A' },
    { urn: BODY_SHAPE_B, label: 'Body B' },
  ];

  const setBodyShape = (urn: string) =>
    update({
      bodyShape: urn || undefined,
      // a new NPC wears the base outfit, which differs per body shape
      wearables: urn ? swapBaseOutfit(wearables, urn) : wearables,
    });

  const setColor = (field: ColorKey, color: Color3 | undefined) => update({ [field]: color });

  return (
    <Container
      label="Avatar Shape"
      className="AvatarShape"
      initialOpen={initialOpen}
      rightContent={
        <InfoTooltip
          text={
            isPlayer
              ? "Changes how each visitor's avatar looks while they are in this scene. Only they see the change."
              : 'Draws an avatar on this entity, for example an NPC. Dress it with wearables from the scene, the base avatars or the catalog.'
          }
          link={
            isPlayer
              ? 'https://docs.decentraland.org/creator/sdk7/interactivity/avatars/scene-wearables/'
              : 'https://docs.decentraland.org/creator/sdk7/interactivity/avatars/npc-avatars/'
          }
          type="help"
        />
      }
      component={AvatarShape}
      entity={entity}
      onRemoveContainer={handleRemove}
    >
      {isPlayer ? (
        <p className={styles.callout}>
          Each visitor keeps their own look in every field you leave unset. Only they see the
          change, and only while they are in this scene. To dress the other players too, write
          Avatar Shape on their entities from code.
        </p>
      ) : (
        <>
          <div className={styles.preview}>
            <WearablePreview
              id={`avatar-shape-preview-${entity}`}
              urns={previewUrns}
              bodyShape={bodyShape as React.ComponentProps<typeof WearablePreview>['bodyShape']}
              skin={toPreviewColor(value.skinColor, DEFAULT_SKIN_COLOR)}
              hair={toPreviewColor(value.hairColor, DEFAULT_HAIR_COLOR)}
              eyes={toPreviewColor(value.eyeColor, DEFAULT_EYE_COLOR)}
              disableBackground
              disableDefaultWearables
            />
          </div>
          {hasSceneWearables && (
            <p className={styles.hint}>
              Scene wearables are not in this preview; the viewport shows them.
            </p>
          )}
          <Block>
            <TextField
              label={
                <>
                  Name Tag{' '}
                  <InfoTooltip
                    text="The name shown over the avatar. Explorers add # and the last four characters of the avatar's ID after it."
                    type="help"
                  />
                </>
              }
              value={value.name ?? ''}
              placeholder="NPC"
              rightLabel={<span className={styles.suffix}>{getNametagSuffix(value.id)}</span>}
              onChange={event => update({ name: event.target.value || undefined })}
            />
          </Block>
        </>
      )}

      <Block label="Body shape">
        <div
          className={styles.segmented}
          role="group"
          aria-label="Body shape"
          style={{ '--segments': bodyOptions.length } as React.CSSProperties}
        >
          {bodyOptions.map(option => (
            <button
              key={option.urn}
              type="button"
              className={styles.segment}
              aria-pressed={bodyShape === option.urn}
              onClick={() => setBodyShape(option.urn)}
            >
              {option.label}
            </button>
          ))}
        </div>
      </Block>

      <div className={styles.colors}>
        {COLOR_FIELDS.map(({ field, label, fallback }) => {
          const color = value[field];
          return (
            <div
              key={field}
              className={styles.color}
            >
              <ColorPicker
                label={isPlayer && !color ? `${label} (profile)` : label}
                value={toHex(color ?? fallback)}
                onChange={event => setColor(field, toColor3(event.target.value))}
              />
              {isPlayer && color && (
                <button
                  type="button"
                  className={styles.reset}
                  aria-label={`Use each visitor's own ${label.toLowerCase()} colour`}
                  title="Use the profile colour"
                  onClick={() => setColor(field, undefined)}
                >
                  <ResetIcon aria-hidden="true" />
                </button>
              )}
            </div>
          );
        })}
      </div>

      <div
        ref={drop}
        className={styles.wearables}
        data-drop={canDrop ? (isOver ? 'over' : 'ready') : undefined}
      >
        <div className={styles.wearablesHeader}>
          <span className={styles.sectionTitle}>Wearables</span>
          <span className={styles.count}>{worn.size} of 17 slots</span>
        </div>
        {rows.length > 0 && (
          <ul className={styles.list}>
            {rows.map(row => (
              <li
                key={`${row.index}-${row.value}`}
                className={styles.row}
                data-replaced={!!row.replacedBy}
                data-invalid={row.invalid}
                title={row.value}
              >
                {row.thumbnail ? (
                  <img
                    className={styles.thumb}
                    src={row.thumbnail}
                    alt=""
                  />
                ) : (
                  <span className={styles.thumb}>
                    <CategoryIcon category={row.category} />
                  </span>
                )}
                <span className={styles.rowText}>
                  <span className={styles.rowName}>{row.name}</span>
                  <span className={styles.rowMeta}>
                    {row.categoryLabel}
                    {row.replacedBy && (
                      <span className={styles.warning}> · replaced by {row.replacedBy}</span>
                    )}
                  </span>
                </span>
                {row.source && (
                  <span
                    className={styles.badge}
                    data-source={row.source}
                  >
                    {SOURCE_LABELS[row.source]}
                  </span>
                )}
                <button
                  type="button"
                  className={styles.remove}
                  aria-label={`Remove ${row.name}`}
                  onClick={() => update({ wearables: removeWearableAt(wearables, row.index) })}
                >
                  <RemoveIcon aria-hidden="true" />
                </button>
              </li>
            ))}
          </ul>
        )}
        <button
          type="button"
          className={styles.add}
          onClick={() => setIsAdding(true)}
        >
          <AddIcon aria-hidden="true" />
          Add wearable
        </button>
        {isPlayer && wearables.length > MAX_WEARABLES && (
          <p
            className={styles.warning}
            role="status"
          >
            Explorers use up to {MAX_WEARABLES} wearables on a player. Remove the ones the avatar
            doesn't wear.
          </p>
        )}
        <p className={styles.hint}>
          {isPlayer
            ? "Leave the list empty to keep each visitor's own wearables. A list replaces all of them: the slots it leaves empty stay empty, and the eyes, eyebrows and mouth it leaves out show the default ones."
            : 'Slots you leave empty stay empty, except the eyes, eyebrows and mouth. Drop a wearable from Local Assets here.'}
        </p>
      </div>

      {!isPlayer && (
        <AnimationField
          value={value.expressionTriggerId}
          sceneAnimations={sceneAnimations}
          onChange={emote =>
            update({
              expressionTriggerId: emote,
              expressionTriggerTimestamp: emote
                ? nextExpressionTimestamp(value.expressionTriggerTimestamp)
                : undefined,
            })
          }
          onReplay={() =>
            update({
              expressionTriggerTimestamp: nextExpressionTimestamp(value.expressionTriggerTimestamp),
            })
          }
        />
      )}

      <Block>
        <CheckboxField
          label={
            <>
              Show only wearables{' '}
              <InfoTooltip
                text="Hides the skin, hair and face, so only the wearables show: for costumes, mascots and robots."
                type="help"
              />
            </>
          }
          checked={!!value.showOnlyWearables}
          onChange={event => update({ showOnlyWearables: event.target.checked })}
        />
      </Block>
      {!isPlayer && (
        <Block>
          <CheckboxField
            label={
              <>
                Show talking bubble{' '}
                <InfoTooltip
                  text="Shows the voice chat icon over the avatar, as if it were talking."
                  type="help"
                />
              </>
            }
            checked={!!value.talking}
            onChange={event => update({ talking: event.target.checked })}
          />
        </Block>
      )}

      <AddWearableModal
        isOpen={isAdding}
        worn={worn}
        sceneWearables={sceneWearables}
        onClose={() => setIsAdding(false)}
        onSelect={handleSelect}
      />
    </Container>
  );
});
