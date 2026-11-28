import { useEffect, useId, useMemo, useState } from 'react';
import { MdOutlineEmojiPeople as EmoteIcon } from 'react-icons/md';
import { VscClose as CloseIcon, VscSearch as SearchIcon } from 'react-icons/vsc';

import { Modal } from '../../Modal';
import type { AnimationItem } from './catalog';
import { fetchBuiltInAnimations, searchCatalogEmotes } from './catalog';
import type { PickerResult } from './PickerResults';
import { PickerResults } from './PickerResults';
import { BUILT_IN_EMOTES } from './utils';

import styles from './AddWearableModal.module.css';

const SEARCH_DEBOUNCE_MS = 300;

type Props = {
  isOpen: boolean;
  /** The current `expressionTriggerId`. */
  value?: string;
  /** The scene's `_emote.glb` files. */
  sceneAnimations: readonly AnimationItem[];
  onClose: () => void;
  onSelect: (item: AnimationItem) => void;
};

type Result = PickerResult & { item: AnimationItem };

function matches(item: AnimationItem, search: string) {
  const term = search.trim().toLowerCase();
  return !term || item.name.toLowerCase().includes(term) || item.id.toLowerCase().includes(term);
}

export function AnimationPickerModal({ isOpen, value, sceneAnimations, onClose, onSelect }: Props) {
  const [search, setSearch] = useState('');
  const [builtIn, setBuiltIn] = useState<AnimationItem[]>([]);
  const [catalog, setCatalog] = useState<{
    items: AnimationItem[];
    loading: boolean;
    error?: string;
  }>({ items: [], loading: true });
  const titleId = useId();
  const searchId = useId();

  useEffect(() => {
    if (isOpen) setSearch('');
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) return;
    let current = true;
    void fetchBuiltInAnimations(BUILT_IN_EMOTES).then(items => current && setBuiltIn(items));
    return () => {
      current = false;
    };
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) return;
    const controller = new AbortController();
    setCatalog(previous => ({ ...previous, loading: true, error: undefined }));
    const timer = setTimeout(() => {
      searchCatalogEmotes(search, controller.signal)
        .then(items => setCatalog({ items, loading: false }))
        .catch(error => {
          if (controller.signal.aborted) return;
          setCatalog({ items: [], loading: false, error: String(error?.message ?? error) });
        });
    }, SEARCH_DEBOUNCE_MS);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [isOpen, search]);

  // The catalog is searched on the server; the other lists are filtered here.
  const toResults = (
    items: readonly AnimationItem[],
    meta?: (item: AnimationItem) => string,
    filter = true,
  ) =>
    items
      .filter($ => !filter || matches($, search))
      .map(
        (item): Result => ({
          id: item.id,
          name: item.name,
          thumbnail: item.thumbnail,
          icon: <EmoteIcon />,
          meta: meta?.(item),
          selected: item.id === value,
          item,
        }),
      );

  const builtInResults = useMemo(() => toResults(builtIn), [builtIn, search, value]);
  const sceneResults = useMemo(
    () => toResults(sceneAnimations, item => item.id.split('/').pop() ?? item.id),
    [sceneAnimations, search, value],
  );
  const catalogResults = useMemo(
    () => toResults(catalog.items, item => item.rarity ?? '', false),
    [catalog.items, value],
  );

  const handleSelect = (result: Result) => onSelect(result.item);

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
            Choose an animation
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
        <div className={styles.search}>
          <label
            htmlFor={searchId}
            className={styles.visuallyHidden}
          >
            Search animations by name
          </label>
          <SearchIcon
            className={styles.searchIcon}
            aria-hidden="true"
          />
          <input
            id={searchId}
            type="search"
            className={styles.searchInput}
            placeholder="Search animations by name"
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
            items={sceneResults}
            empty="No animation files in the scene. An animation file is a GLB named name_emote.glb, like the ones Import Assets makes."
            onSelect={handleSelect}
          />
          <PickerResults
            title="Built-in"
            items={builtInResults}
            loading={!builtIn.length}
            empty="No built-in animations match."
            onSelect={handleSelect}
          />
          <PickerResults
            title="Decentraland catalog"
            items={catalogResults}
            loading={catalog.loading}
            error={catalog.error}
            empty="No published emotes match."
            onSelect={handleSelect}
          />
        </div>
      </div>
    </Modal>
  );
}
