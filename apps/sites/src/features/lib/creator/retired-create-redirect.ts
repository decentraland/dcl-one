import { REDIRECTS, buildDestination } from "./builder-redirect";

const RETIRED_WEARABLES = "/create/wearables";

/**
 * Where a URL of the retired `/create/wearables` and `/create/curate` pages now lives. The `/builder/*` routes sent
 * permanent redirects to them, which browsers keep, so they move on permanently too.
 */
export function retiredCreateDestination(url: URL): string {
  if (url.pathname.replace(/\/$/, "") === "/create/curate") return buildDestination(REDIRECTS.curation, url, {});
  const [page, id] = url.pathname.slice(RETIRED_WEARABLES.length).split("/").filter(Boolean);
  if (page === "collections" && id && id !== "new") return buildDestination(REDIRECTS["collection-detail"], url, { id });
  if (page === "items" && id) return buildDestination(REDIRECTS["item-detail"], url, { id });
  if (page === "item-editor") return buildDestination(REDIRECTS["item-editor"], url, {});
  if (page === "publish") return buildDestination(REDIRECTS["publish-collection"], url, {});
  return "/create/collections";
}
