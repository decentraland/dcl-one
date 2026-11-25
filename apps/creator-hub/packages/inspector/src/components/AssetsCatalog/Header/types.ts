import type { AssetPack } from '../../../lib/logic/catalog';

export interface Props {
  selectedTheme: AssetPack | undefined;
  search: string;
  /** The categories of the assets in scope, with how many assets each holds. */
  categories: { name: string; count: number }[];
  selectedCategory: string | undefined;
  onChangeCategory: (value?: string) => void;
  onChangeTheme: (value?: AssetPack) => void;
  onSearch: (value: string) => void;
}
