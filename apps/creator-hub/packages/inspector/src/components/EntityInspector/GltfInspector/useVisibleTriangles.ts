import { useEffect, useState } from 'react';

import { getDataLayerInterface } from '../../../redux/data-layer';
import { selectAssetCatalog } from '../../../redux/app';
import { useAppSelector } from '../../../redux/hooks';
import { countVisibleTriangles } from './utils';

// The data layer has no partial read, so counting means fetching the whole model. Each model is
// read once per asset catalog: the catalog is fetched again whenever the scene's files change,
// which is when a count can go stale.
let countedFor: unknown;
const counts = new Map<string, Promise<number | null>>();

export function readVisibleTriangles(src: string, catalog: unknown): Promise<number | null> {
  if (countedFor !== catalog) {
    counts.clear();
    countedFor = catalog;
  }
  let count = counts.get(src);
  if (!count) {
    const dataLayer = getDataLayerInterface();
    count = dataLayer
      ? dataLayer.getFile({ path: src }).then(({ content }) => countVisibleTriangles(content))
      : Promise.resolve(null);
    counts.set(src, count);
    // A failed read is not kept, so selecting the entity again retries it.
    count.catch(() => {
      if (counts.get(src) === count) counts.delete(src);
    });
  }
  return count;
}

/** Triangle count of a model's visible meshes, or null while unknown. */
export function useVisibleTriangles(src: string | undefined, enabled: boolean): number | null {
  const catalog = useAppSelector(selectAssetCatalog);
  const [triangles, setTriangles] = useState<number | null>(null);

  useEffect(() => {
    setTriangles(null);
    if (!src || !enabled) return;
    let cancelled = false;
    readVisibleTriangles(src, catalog)
      .then(count => {
        if (!cancelled) setTriangles(count);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [src, enabled, catalog]);

  return triangles;
}
