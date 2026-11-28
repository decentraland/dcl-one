import { useMemo } from 'react';

import type { AssetCatalogResponse } from '../../tooling-entrypoint';
import { buildAssetTree } from '../../components/ProjectAssetExplorer/utils';
import { removeBasePath } from '../../lib/logic/remove-base-path';
import type { RiggedModel } from '../../lib/logic/wearable-models';

/* istanbul ignore next */
export const useAssetTree = (
  files: AssetCatalogResponse,
  riggedWearables: ReadonlyMap<string, RiggedModel> = new Map(),
) => {
  const tree = useMemo(() => {
    // the tree names files without the catalog's base path
    const rigged = new Map(
      [...riggedWearables].map(([path, model]) => [removeBasePath(files.basePath, path), model]),
    );
    return buildAssetTree(
      files.assets.map(item => removeBasePath(files.basePath, item.path)),
      rigged,
    );
  }, [files, riggedWearables]);
  return { tree };
};
