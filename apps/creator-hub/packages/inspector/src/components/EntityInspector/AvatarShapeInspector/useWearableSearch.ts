import { useEffect, useMemo, useState } from 'react';

import type { WearableCategory } from '../../../lib/logic/wearables';
import type { WearableItem } from './catalog';
import { fetchBaseWearables, searchCatalogWearables } from './catalog';

const SEARCH_DEBOUNCE_MS = 300;

type Remote = {
  items: WearableItem[];
  loading: boolean;
  error?: string;
};

export type WearableSearch = {
  scene: WearableItem[];
  base: Remote;
  catalog: Remote;
};

export function matchesName(item: WearableItem, search: string): boolean {
  const term = search.trim().toLowerCase();
  return !term || item.name.toLowerCase().includes(term);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * The wearables of one slot whose name matches `search`, from the scene's files, the base avatars
 * and the published catalog. The catalog is searched on the server, after a pause in typing.
 */
export function useWearableSearch(
  enabled: boolean,
  category: WearableCategory,
  search: string,
  sceneWearables: readonly WearableItem[],
): WearableSearch {
  const [allBase, setAllBase] = useState<Remote>({ items: [], loading: true });
  const [catalog, setCatalog] = useState<Remote>({ items: [], loading: true });

  useEffect(() => {
    if (!enabled) return;
    let current = true;
    setAllBase(previous => ({ ...previous, loading: true, error: undefined }));
    fetchBaseWearables()
      .then(items => current && setAllBase({ items, loading: false }))
      .catch(
        error => current && setAllBase({ items: [], loading: false, error: errorMessage(error) }),
      );
    return () => {
      current = false;
    };
  }, [enabled]);

  useEffect(() => {
    if (!enabled) return;
    const controller = new AbortController();
    setCatalog(previous => ({ ...previous, loading: true, error: undefined }));
    const timer = setTimeout(() => {
      searchCatalogWearables(category, search, controller.signal)
        .then(items => setCatalog({ items, loading: false }))
        .catch(error => {
          if (controller.signal.aborted) return;
          setCatalog({ items: [], loading: false, error: errorMessage(error) });
        });
    }, SEARCH_DEBOUNCE_MS);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [enabled, category, search]);

  const scene = useMemo(
    () =>
      sceneWearables.filter(
        $ => ($.category === category || $.fitsAnySlot) && matchesName($, search),
      ),
    [sceneWearables, category, search],
  );
  const base = useMemo(
    () => ({
      ...allBase,
      items: allBase.items.filter($ => $.category === category && matchesName($, search)),
    }),
    [allBase, category, search],
  );

  return { scene, base, catalog };
}
