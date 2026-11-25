import { getDomain } from './fetch-utils';

export const getContentUrl = () => `https://builder-items.decentraland.${getDomain()}`;
export const getRewardsServerUrl = () => `https://rewards.decentraland.${getDomain()}`;

let itemAssets: string | undefined;

/** The scene folder of the item's own files; they deploy with the scene. An item with no script names none. */
export function setItemFolder(folder: string | undefined) {
  itemAssets = folder && `${folder}/assets`;
}

/** Where the panel's pictures are: in the item's own folder, or else in the bucket of the catalog items. */
export const getAssetsUrl = () => itemAssets ?? `${getContentUrl()}/admin_toolkit/assets`;
