import type { Eip1193Provider } from "./eip1193";

export type WalletEvents = {
  onAccountsChanged?(account: string | null): void;
  onChainChanged?(): void;
};

/** Follows the wallet's own events; returns the unsubscribe. */
export function subscribeWallet(provider: Eip1193Provider | null, events: WalletEvents): () => void {
  if (!provider) return () => {};
  let on: Eip1193Provider["on"];
  let removeListener: Eip1193Provider["removeListener"];
  try {
    // Brave Android can throw from its injected proxy when reading an event method.
    ({ on, removeListener } = provider);
  } catch {
    console.warn("[shop/wallet] provider event methods unreadable; skipping subscription");
    return () => {};
  }
  if (typeof on !== "function" || typeof removeListener !== "function") return () => {};
  const accounts = (...args: unknown[]) => {
    const list = (args[0] as string[] | undefined) ?? [];
    events.onAccountsChanged?.(list[0]?.toLowerCase() ?? null);
  };
  const chain = () => events.onChainChanged?.();
  on.call(provider, "accountsChanged", accounts);
  on.call(provider, "chainChanged", chain);
  return () => {
    removeListener.call(provider, "accountsChanged", accounts);
    removeListener.call(provider, "chainChanged", chain);
  };
}
