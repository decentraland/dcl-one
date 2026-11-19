export const CREATOR_HUB_DOWNLOAD = "/download/creator-hub";

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

export function isRetiredHubPage(head: string | undefined): boolean {
  return head !== undefined && RETIRED_PAGES.has(head);
}
