// Where the Shop's chain reads, relays and settles may be repointed from the page (window.__SHOP_CHAIN__,
// __SHOP_RELAY__, __SHOP_RPC__, __SHOP_ETHEREUM_CHAIN_ID__) by tests and local builds only. A production build
// ignores those globals, so a script on the page that sets one (an extension, a name that collides) can't repoint
// the relay, an RPC or the settlement chain: production takes them from its build (VITE_SHOP_*) and its node.

/** The page's override named `name` outside a production build; undefined in one, and where there is no page. */
export function pageOverride<T>(name: string): T | undefined {
  if (import.meta.env?.DEV !== true || typeof window === "undefined") return undefined;
  return (window as unknown as Record<string, T | undefined>)[name];
}
