// from: shop@94584a5c:app/src/lib/searchClear.ts
// Where clearing the search box navigates: on the results page it drops `q` and keeps every other
// filter; anywhere else it stays put.

import { SHOP_BASE } from "../../routes";

const RESULTS = `${SHOP_BASE}/items`;

export function clearedSearchUrl(pathname: string, search: string): string | null {
  if (pathname !== RESULTS) return null;
  const params = new URLSearchParams(search);
  if (!params.has("q")) return null;
  params.delete("q");
  const rest = params.toString();
  return rest ? `${RESULTS}?${rest}` : RESULTS;
}
