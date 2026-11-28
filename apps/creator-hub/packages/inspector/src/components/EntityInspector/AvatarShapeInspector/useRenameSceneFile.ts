import { useCallback, useMemo } from 'react';

import { useAppSelector } from '../../../redux/hooks';
import { selectAssetCatalog, selectThumbnails } from '../../../redux/app';
import { useAssetTree } from '../../../hooks/catalog/useAssetTree';
import { addBasePath } from '../../../lib/logic/add-base-path';
import { removeBasePath } from '../../../lib/logic/remove-base-path';
import { generateAssetTree } from '../../ProjectAssetExplorer/tree';
import type { AssetNodeFolder } from '../../ProjectAssetExplorer/types';
import { useAssetOperations } from '../../ProjectAssetExplorer/useAssetOperations';

const EMPTY_CATALOG = { basePath: '', assets: [] };

/**
 * Renames a scene file in place, the way Local Assets does: every scene reference to it follows.
 * Takes and returns paths as the asset catalog lists them; undefined when the rename failed (the
 * reason is shown as a notification).
 */
export function useRenameSceneFile() {
  const files = useAppSelector(selectAssetCatalog) ?? EMPTY_CATALOG;
  const thumbnails = useAppSelector(selectThumbnails);
  const { tree: root } = useAssetTree(files);
  const tree = useMemo(() => {
    const folders = root.children.filter(item => item.type === 'folder') as AssetNodeFolder[];
    return generateAssetTree(folders, new Set()).tree;
  }, [root]);
  const { rename } = useAssetOperations({ tree, thumbnails, virtualFolders: [] });

  return useCallback(
    async (path: string, newName: string): Promise<string | undefined> => {
      const renamed = await rename(removeBasePath(files.basePath, path), newName);
      return renamed ? addBasePath(files.basePath, renamed) : undefined;
    },
    [rename, files.basePath],
  );
}
