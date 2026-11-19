export const CREATOR_HUB_DOWNLOAD = "/download/creator-hub";
export const ACCOUNT_ASSETS = "/profile/me/assets";

// Web Creator Hub pages whose work moved to the desktop app, by first path segment under /create.
const RETIRED_PAGES = new Set([
  "home",
  "templates",
  "scenes",
  "my-scenes",
  "scene-editor",
  "create-project",
  "delete-project",
  "drafts",
  "publish",
  "deploy-world",
  "deploy-alternative",
  "admissions",
  "gate",
  "world-permissions",
  "world-settings",
  "worlds",
  "manage",
  "land",
  "names",
  "metrics",
  "activity",
  "scene-analytics",
  "storage",
  "worlds-storage",
  "learn",
  "settings",
  "data-sources",
  "map",
]);

// The LAND and NAME lists match `/builder/land` and `/builder/names`: what was listed there is the account's assets.
const OWNED_ASSET_PAGES = new Set(["land", "names"]);

/** Where an old link to a retired page goes; `null` for a path that was never a hub page. */
export function retiredHubTarget(head: string | undefined): string | null {
  if (head === undefined || !RETIRED_PAGES.has(head)) return null;
  return OWNED_ASSET_PAGES.has(head) ? ACCOUNT_ASSETS : CREATOR_HUB_DOWNLOAD;
}
