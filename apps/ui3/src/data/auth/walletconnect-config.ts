// Whether this site can offer WalletConnect, apart from walletconnect.ts so a page can ask without loading the signing
// code that module brings.

export function hasWalletConnect(): boolean {
  return walletConnectProjectId().length > 0;
}

/** The explorer is served as a static document, outside the sites root loader. */
export async function loadWalletConnectConfig(): Promise<boolean> {
  if (hasWalletConnect()) return true;
  const response = await fetch("/internal/auth-config");
  if (!response.ok) throw new Error("Couldn't load wallet sign-in configuration.");
  const config: unknown = await response.json();
  if (!config || typeof config !== "object" || !("walletConnectProjectId" in config)
    || typeof config.walletConnectProjectId !== "string") {
    throw new Error("Invalid wallet sign-in configuration.");
  }
  window.__DCL_PUBLIC__ = { ...window.__DCL_PUBLIC__, walletConnectProjectId: config.walletConnectProjectId };
  return hasWalletConnect();
}

export function walletConnectProjectId(): string {
  return (typeof window !== "undefined" && window.__DCL_PUBLIC__?.walletConnectProjectId)
    || import.meta.env?.VITE_DCL_WALLETCONNECT_PROJECT_ID || "";
}
