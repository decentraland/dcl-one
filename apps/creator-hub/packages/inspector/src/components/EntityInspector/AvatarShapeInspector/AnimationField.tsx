import { useEffect, useState } from 'react';
import { useDrop } from 'react-dnd';
import { MdOutlineEmojiPeople as EmoteIcon, MdReplay as ReplayIcon } from 'react-icons/md';
import { VscClose as ClearIcon } from 'react-icons/vsc';

import { withAssetDir } from '../../../lib/data-layer/host/fs-utils';
import { DropTypesEnum, type LocalAssetDrop } from '../../../lib/sdk/drag-drop';
import { isWearableUrn } from '../../../lib/logic/wearables';
import type { TreeNode } from '../../ProjectAssetExplorer/ProjectView';
import type { AssetNodeItem } from '../../ProjectAssetExplorer/types';
import { isEmoteAsset } from '../../ProjectAssetExplorer/utils';
import { InfoTooltip } from '../../ui';
import type { AnimationItem } from './catalog';
import { fetchBuiltInAnimations, resolveCatalogEmote } from './catalog';
import { AnimationPickerModal } from './AnimationPickerModal';
import { BUILT_IN_EMOTES } from './utils';

import styles from './AnimationField.module.css';

type Props = {
  /** The current `expressionTriggerId`. */
  value?: string;
  sceneAnimations: readonly AnimationItem[];
  /** Sets the animation and plays it; undefined stops playing any. */
  onChange: (id: string | undefined) => void;
  /** Plays the current animation again. */
  onReplay: () => void;
};

const SOURCE_LABELS: Record<AnimationItem['source'], string> = {
  builtin: 'Built-in',
  scene: 'Scene',
  catalog: 'Catalog',
};

/** The animation `value` names, as far as it is known. */
function useAnimation(value: string | undefined, sceneAnimations: readonly AnimationItem[]) {
  const [item, setItem] = useState<AnimationItem>();
  useEffect(() => {
    setItem(undefined);
    if (!value) return;
    let current = true;
    const scene = sceneAnimations.find($ => $.id === value);
    if (scene) {
      setItem(scene);
    } else if (isWearableUrn(value)) {
      void resolveCatalogEmote(value).then(
        found => current && setItem(found ?? { id: value, name: value, source: 'catalog' }),
      );
    } else if (/\.(glb|gltf)$/i.test(value)) {
      setItem({ id: value, name: value.split('/').pop() ?? value, source: 'scene' });
    } else {
      void fetchBuiltInAnimations(BUILT_IN_EMOTES).then(items => {
        if (!current) return;
        setItem(items.find($ => $.id === value) ?? { id: value, name: value, source: 'builtin' });
      });
    }
    return () => {
      current = false;
    };
  }, [value, sceneAnimations]);
  return item;
}

function isAnimationNode(node: TreeNode): node is AssetNodeItem {
  return node.type === 'asset' && isEmoteAsset((node as AssetNodeItem).asset);
}

/**
 * The animation an avatar plays: a built-in one, a scene `_emote.glb` or a published emote. Takes
 * a drop of an animation from Local Assets.
 */
export function AnimationField({ value, sceneAnimations, onChange, onReplay }: Props) {
  const [isPicking, setIsPicking] = useState(false);
  const item = useAnimation(value, sceneAnimations);

  const [{ isOver, canDrop }, drop] = useDrop(
    () => ({
      accept: [DropTypesEnum.LocalAsset],
      canDrop: ({ value: path, context }: LocalAssetDrop) => {
        const node = context.tree.get(path);
        return !!node && isAnimationNode(node);
      },
      drop: ({ value: path, context }: LocalAssetDrop) => {
        const node = context.tree.get(path);
        if (!node || !isAnimationNode(node)) return;
        const { asset } = node;
        onChange(asset.builtInEmote ? asset.builtInEmote.name : withAssetDir(asset.src));
      },
      collect: monitor => ({ isOver: monitor.isOver(), canDrop: monitor.canDrop() }),
    }),
    [onChange],
  );

  return (
    <div className={styles.root}>
      <span className={styles.label}>
        Play animation{' '}
        <InfoTooltip
          text="The animation the avatar plays when the scene starts: a built-in one, an animation file from the scene (drag one here from Local Assets) or a published emote. Custom animations play once; Play again replays it."
          type="help"
        />
      </span>
      <div
        ref={drop}
        className={styles.field}
        data-drop={canDrop ? (isOver ? 'over' : 'ready') : undefined}
      >
        {item?.thumbnail ? (
          <img
            className={styles.thumb}
            src={item.thumbnail}
            alt=""
          />
        ) : (
          <span className={styles.thumb}>
            <EmoteIcon aria-hidden="true" />
          </span>
        )}
        <button
          type="button"
          className={styles.choose}
          onClick={() => setIsPicking(true)}
          title={value || undefined}
        >
          <span className={styles.name}>{value ? (item?.name ?? value) : 'None'}</span>
          <span className={styles.meta}>
            {value
              ? item
                ? SOURCE_LABELS[item.source]
                : '…'
              : 'Choose, or drop an animation from Local Assets'}
          </span>
        </button>
        {value && (
          <>
            <button
              type="button"
              className={styles.icon}
              onClick={onReplay}
              aria-label="Play again"
              title="Play again"
            >
              <ReplayIcon aria-hidden="true" />
            </button>
            <button
              type="button"
              className={styles.icon}
              onClick={() => onChange(undefined)}
              aria-label="Remove the animation"
              title="Remove the animation"
            >
              <ClearIcon aria-hidden="true" />
            </button>
          </>
        )}
      </div>
      <AnimationPickerModal
        isOpen={isPicking}
        value={value}
        sceneAnimations={sceneAnimations}
        onClose={() => setIsPicking(false)}
        onSelect={picked => {
          onChange(picked.id);
          setIsPicking(false);
        }}
      />
    </div>
  );
}
