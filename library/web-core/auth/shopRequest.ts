import type { AuthIdentity } from "./identity";
import { signAuthChain } from "./signerCore";

type BrowserShopApiMode = "canonical" | "legacy";

/** The API mode: the override, else the page's `window.__SHOP_API_MODE__`, else `VITE_SHOP_API_MODE`; legacy only when one says so. */
export function shopApiMode(override?: BrowserShopApiMode): BrowserShopApiMode {
  if (override) return override;
  const fromWindow = typeof window !== "undefined" ? (window as Window & { __SHOP_API_MODE__?: string }).__SHOP_API_MODE__ : undefined;
  const fromEnv = typeof import.meta !== "undefined" ? (import.meta.env?.VITE_SHOP_API_MODE as string | undefined) : undefined;
  const value = fromWindow ?? fromEnv;
  return value === "legacy" ? "legacy" : "canonical";
}

/** A transport over fetch. */
export function fetchTransport(fetchImpl: typeof fetch = (...args) => fetch(...args)) {
  return async (req: Pick<RequestInit, "method" | "headers" | "body" | "signal"> & { url: string }) => {
    const res = await fetchImpl(req.url, {
      method: req.method,
      headers: req.headers,
      body: req.body,
      signal: req.signal,
    });
    return { status: res.status, body: res.status === 204 ? "" : await res.text() };
  };
}

function expired(expiration: string, now: number): boolean {
  const at = Date.parse(expiration);
  return !Number.isFinite(at) || at <= now;
}

/** Signs with a web identity (sites' session, or the host's identity in the native contract). */
export function identitySigner(identity: AuthIdentity | null, now: () => number = Date.now) {
  if (!identity || expired(identity.expiration, now())) return null;
  return {
    address: identity.signer.toLowerCase(),
    async sign(method: string, path: string, metadata: Record<string, unknown>) {
      if (expired(identity.expiration, now())) throw new Error("The sign-in has expired. Sign in again to continue.");
      const { headers } = await signAuthChain(
        identity.ephemeral.privateKey,
        identity.authChain,
        method,
        path,
        String(now()),
        JSON.stringify(metadata),
      );
      return headers;
    },
  };
}
