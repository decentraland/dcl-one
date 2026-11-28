import { useEffect, useMemo, useState } from 'react';

import { getDataLayerInterface } from '../../redux/data-layer';
import type { RiggedModel } from '../../lib/logic/wearable-models';
import { inspectRiggedModel, isWearableCandidate } from '../../lib/logic/wearable-models';

const MAX_PARALLEL_READS = 3;
const results = new Map<string, Promise<RiggedModel | undefined>>();
const queue: (() => void)[] = [];
let active = 0;

function pump() {
  while (active < MAX_PARALLEL_READS && queue.length) {
    active++;
    queue.shift()!();
  }
}

/** The scene model at `path` (as the asset catalog lists it) when it is a rigged wearable. */
export function detectRiggedModel(path: string): Promise<RiggedModel | undefined> {
  let result = results.get(path);
  if (!result) {
    result = new Promise(resolve => {
      queue.push(() => {
        const dataLayer = getDataLayerInterface();
        const read = dataLayer
          ? dataLayer.getAssetData({ path }).then(({ data }) => inspectRiggedModel(data))
          : Promise.resolve(undefined);
        void read
          .catch(() => {
            // A file that could not be read is asked again next time.
            results.delete(path);
            return undefined;
          })
          .then(resolve)
          .finally(() => {
            active--;
            pump();
          });
      });
      pump();
    });
    results.set(path, result);
  }
  return result;
}

/**
 * The models among `paths` that are rigged wearables without a `name_category.glb` name, keyed by
 * path. Each file is read once per session; the map fills in as the reads finish.
 */
export function useRiggedWearableModels(
  paths: readonly string[],
): ReadonlyMap<string, RiggedModel> {
  const key = paths.join('\n');
  const candidates = useMemo(() => paths.filter(isWearableCandidate), [key]);
  const [found, setFound] = useState<ReadonlyMap<string, RiggedModel>>(new Map());

  useEffect(() => {
    let current = true;
    for (const path of candidates) {
      void detectRiggedModel(path).then(model => {
        if (!current || !model) return;
        setFound(previous => (previous.has(path) ? previous : new Map(previous).set(path, model)));
      });
    }
    return () => {
      current = false;
    };
  }, [candidates]);

  return useMemo(() => {
    const present = new Map<string, RiggedModel>();
    for (const path of candidates) {
      const model = found.get(path);
      if (model) present.set(path, model);
    }
    return present;
  }, [candidates, found]);
}
