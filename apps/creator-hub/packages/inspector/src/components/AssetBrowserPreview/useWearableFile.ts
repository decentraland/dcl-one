import { useEffect, useState } from 'react';

import type { WearableCategory } from '../../lib/logic/wearables';
import { getSceneWearableCategory } from '../../lib/logic/wearables';
import { inspectRiggedModel } from '../../lib/logic/wearable-models';

export type WearableFile = {
  /** The slot the file name gives. */
  category?: WearableCategory;
  /** The slot the mesh names suggest, for a rigged model without one in its name. */
  suggestedCategory?: WearableCategory;
};

/**
 * Whether a model file is a wearable: named `name_category.glb`, or rigged to the avatar
 * armature. `'checking'` while a rigged-model check reads the file; undefined for other models.
 */
export function useWearableFile(file: File | undefined): WearableFile | 'checking' | undefined {
  const named = file ? getSceneWearableCategory(file.name) : undefined;
  const [rigged, setRigged] = useState<{ file: File; model?: WearableFile }>();

  useEffect(() => {
    if (!file || named || !file.name.toLowerCase().endsWith('.glb')) return;
    let current = true;
    void file
      .arrayBuffer()
      .then(buffer => inspectRiggedModel(new Uint8Array(buffer)))
      .catch(() => undefined)
      .then(model => current && setRigged({ file, model }));
    return () => {
      current = false;
    };
  }, [file, named]);

  if (!file) return undefined;
  if (named) return { category: named };
  if (!file.name.toLowerCase().endsWith('.glb')) return undefined;
  if (rigged?.file !== file) return 'checking';
  return rigged.model;
}
