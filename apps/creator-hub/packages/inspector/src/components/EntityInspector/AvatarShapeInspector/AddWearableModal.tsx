import { useCallback, useEffect, useId, useMemo, useState } from 'react';
import { VscClose as CloseIcon, VscSearch as SearchIcon } from 'react-icons/vsc';

import type { WearableCategory } from '../../../lib/logic/wearables';
import {
  WEARABLE_CATEGORIES,
  WEARABLE_CATEGORY_GROUPS,
  getCategoryLabel,
  getSceneWearableCategory,
  isSceneWearableCategory,
  isWearableUrn,
} from '../../../lib/logic/wearables';
import { Modal } from '../../Modal';
import { Button } from '../../Button';
import type { WearableItem } from './catalog';
import { CategoryIcon } from './CategoryIcon';
import type { PickerResult } from './PickerResults';
import { PickerResults } from './PickerResults';
import { useWearableSearch } from './useWearableSearch';
import type { WearableRow } from './utils';
import { getSlotFileName, hasDefaultWearable } from './utils';

import styles from './AddWearableModal.module.css';

export type Props = {
  isOpen: boolean;
  initialCategory?: WearableCategory;
  /** What the avatar wears in each slot now. */
  worn: ReadonlyMap<WearableCategory, WearableRow>;
  /** Scene files named `name_category.glb`, and rigged models whose name gives no slot. */
  sceneWearables: readonly WearableItem[];
  onClose: () => void;
  onSelect: (item: Pick<WearableItem, 'id' | 'category' | 'needsSlotName'>) => void;
};

type Result = PickerResult & { item: WearableItem };

function toResult(item: WearableItem, slot: WearableCategory): Result {
  return {
    id: item.id,
    name: item.name,
    thumbnail: item.thumbnail,
    icon: <CategoryIcon category={item.category} />,
    meta: item.needsSlotName
      ? `Renames to ${getSlotFileName(item.id, slot)}`
      : item.source === 'scene'
        ? item.id.split('/').pop()
        : item.rarity,
    item: { ...item, category: slot },
  };
}

export function AddWearableModal({
  isOpen,
  initialCategory = 'upper_body',
  worn,
  sceneWearables,
  onClose,
  onSelect,
}: Props) {
  const [category, setCategory] = useState<WearableCategory>(initialCategory);
  const [search, setSearch] = useState('');
  const [manual, setManual] = useState('');
  const titleId = useId();
  const searchId = useId();
  const manualId = useId();
  const manualErrorId = useId();

  useEffect(() => {
    if (!isOpen) return;
    setCategory(initialCategory);
    setSearch('');
    setManual('');
  }, [isOpen, initialCategory]);

  const results = useWearableSearch(isOpen, category, search, sceneWearables);
  const label = getCategoryLabel(category);
  const toResults = useCallback(
    (items: readonly WearableItem[]) => items.map($ => toResult($, category)),
    [category],
  );
  const scene = useMemo(() => toResults(results.scene), [results.scene, toResults]);
  const base = useMemo(() => toResults(results.base.items), [results.base.items, toResults]);
  const catalog = useMemo(
    () => toResults(results.catalog.items),
    [results.catalog.items, toResults],
  );

  const manualValue = manual.trim();
  const manualCategory = isWearableUrn(manualValue)
    ? category
    : getSceneWearableCategory(manualValue);
  const manualError =
    manualValue && !manualCategory
      ? 'Enter a wearable URN, or the path of a scene file named name_category.glb, from the project folder (assets/…)'
      : undefined;

  const handleManualAdd = useCallback(() => {
    if (!manualValue || !manualCategory) return;
    onSelect({ id: manualValue, category: manualCategory });
  }, [manualValue, manualCategory, onSelect]);

  const handleSelect = useCallback((result: Result) => onSelect(result.item), [onSelect]);

  return (
    <Modal
      isOpen={isOpen}
      onRequestClose={onClose}
      aria={{ labelledby: titleId }}
    >
      <div className={styles.content}>
        <header className={styles.header}>
          <h2
            id={titleId}
            className={styles.title}
          >
            Add wearable
          </h2>
          <button
            type="button"
            className={styles.close}
            onClick={onClose}
            aria-label="Close"
          >
            <CloseIcon aria-hidden="true" />
          </button>
        </header>

        <div className={styles.slots}>
          {WEARABLE_CATEGORY_GROUPS.map(group => (
            <section
              key={group.id}
              className={styles.group}
              aria-label={group.label}
            >
              <h3 className={styles.groupTitle}>{group.label}</h3>
              <div className={styles.cards}>
                {WEARABLE_CATEGORIES.filter($ => $.group === group.id).map(slot => {
                  const current = worn.get(slot.id);
                  const state = current
                    ? current.name
                    : hasDefaultWearable(slot.id)
                      ? 'Default'
                      : 'Empty';
                  return (
                    <button
                      key={slot.id}
                      type="button"
                      className={styles.card}
                      aria-pressed={slot.id === category}
                      onClick={() => setCategory(slot.id)}
                    >
                      <CategoryIcon
                        category={slot.id}
                        className={styles.cardIcon}
                      />
                      <span className={styles.cardLabel}>{slot.label}</span>
                      <span
                        className={current ? styles.cardWorn : styles.cardState}
                        title={state}
                      >
                        {state}
                      </span>
                    </button>
                  );
                })}
              </div>
            </section>
          ))}
        </div>

        <div className={styles.search}>
          <label
            htmlFor={searchId}
            className={styles.visuallyHidden}
          >
            Search {label} wearables by name
          </label>
          <SearchIcon
            className={styles.searchIcon}
            aria-hidden="true"
          />
          <input
            id={searchId}
            type="search"
            className={styles.searchInput}
            placeholder={`Search ${label.toLowerCase()} by name`}
            value={search}
            onChange={event => setSearch(event.target.value)}
            autoComplete="off"
          />
        </div>

        <div
          className={styles.results}
          aria-live="polite"
        >
          <PickerResults
            title="This scene"
            items={scene}
            empty={
              isSceneWearableCategory(category)
                ? `No ${label.toLowerCase()} models in the scene. A wearable is a GLB rigged to the avatar skeleton; import one into Local Assets.`
                : `${label} is a texture, not a model, so a scene file can't fill it. Choose one from the base avatars or the catalog.`
            }
            onSelect={handleSelect}
          />
          <PickerResults
            title="Base avatars"
            items={base}
            loading={results.base.loading}
            error={results.base.error}
            empty="No base wearables match."
            onSelect={handleSelect}
          />
          <PickerResults
            title="Decentraland catalog"
            items={catalog}
            loading={results.catalog.loading}
            error={results.catalog.error}
            empty="No published wearables match."
            onSelect={handleSelect}
          />
        </div>

        <div className={styles.manual}>
          <label
            htmlFor={manualId}
            className={styles.manualLabel}
          >
            URN or scene file
          </label>
          <input
            id={manualId}
            className={styles.manualInput}
            placeholder="urn:decentraland:… or assets/…/name_category.glb"
            value={manual}
            onChange={event => setManual(event.target.value)}
            onKeyDown={event => event.key === 'Enter' && handleManualAdd()}
            aria-invalid={!!manualError}
            aria-describedby={manualError ? manualErrorId : undefined}
          />
          <Button
            variant="secondary"
            size="sm"
            disabled={!manualValue || !!manualError}
            onClick={handleManualAdd}
          >
            Add
          </Button>
          {manualError && (
            <p
              id={manualErrorId}
              className={styles.manualError}
            >
              {manualError}
            </p>
          )}
        </div>
      </div>
    </Modal>
  );
}
