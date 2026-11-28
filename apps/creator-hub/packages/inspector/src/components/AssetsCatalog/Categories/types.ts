import type { Asset } from '../../../lib/logic/catalog';

export interface Props {
  assets: Asset[];
  /** Names of the categories whose assets are hidden. */
  collapsed: ReadonlySet<string>;
  onToggle: (category: string) => void;
  onAddToFilesystem?: (asset: Asset) => void;
}
