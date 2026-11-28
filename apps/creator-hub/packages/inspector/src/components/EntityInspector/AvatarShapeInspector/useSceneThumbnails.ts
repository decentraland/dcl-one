import { useEffect, useState } from 'react';

import { useAppSelector } from '../../../redux/hooks';
import { selectAssetCatalog, selectThumbnails } from '../../../redux/app';
import { removeBasePath } from '../../../lib/logic/remove-base-path';
import { transformBinaryToBase64Resource } from '../../../lib/data-layer/host/fs-utils';
import { findAssetThumbnails } from '../../ProjectAssetExplorer/utils';

const NONE: ReadonlyMap<string, string> = new Map();

/** The thumbnail Local Assets shows for each scene file in `paths`, as an image URL keyed by path. */
export function useSceneThumbnails(paths: readonly string[]): ReadonlyMap<string, string> {
  const basePath = useAppSelector(selectAssetCatalog)?.basePath ?? '';
  const thumbnails = useAppSelector(selectThumbnails);
  const [found, setFound] = useState(NONE);
  const key = paths.join('\n');

  useEffect(() => {
    let current = true;
    const sources = paths.map(path => removeBasePath(basePath, path));
    void findAssetThumbnails(sources, thumbnails).then(map => {
      if (!current) return;
      const urls = new Map<string, string>();
      paths.forEach((path, index) => {
        const content = map.get(sources[index]);
        if (content) urls.set(path, transformBinaryToBase64Resource(content));
      });
      setFound(urls);
    });
    return () => {
      current = false;
    };
  }, [key, basePath, thumbnails]);

  return found;
}
