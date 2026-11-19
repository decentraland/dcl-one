// Four page globals repoint the Shop's chains, RPCs and relay: window.__SHOP_CHAIN__, __SHOP_RELAY__, __SHOP_RPC__ and
// __SHOP_ETHEREUM_CHAIN_ID__. Tests and local builds use them. A production build ignores them, so setting one of
// these four on the page (an extension, a name that collides) changes nothing there: production takes those values
// from its build (VITE_SHOP_*). This covers these four only. The node base (window.__CATALYST_BASE__, which Sites
// sets) still feeds the default relay url and the Shop API base (data/catalyst/client.ts catalystBase).

/** The page's override named `name` outside a production build; undefined in one, and where there is no page. */
export function pageOverride<T>(name: string): T | undefined {
  if (import.meta.env?.DEV !== true || typeof window === "undefined") return undefined;
  return (window as unknown as Record<string, T | undefined>)[name];
}
