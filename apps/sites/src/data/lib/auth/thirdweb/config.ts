export const THIRDWEB_API_BASE = "https://api.thirdweb.com";

export const LOGIN_CHAIN_ID = 1;

declare global {
  interface Window {
    __DCL_PUBLIC__?: {
      thirdwebClientId?: string;
      walletConnectProjectId?: string;
      thirdwebSignProxy?: string;
      docsBase?: string;
    };
  }
}

export function thirdwebClientId(): string {
  if (typeof process !== "undefined" && process.env?.THIRDWEB_CLIENT_ID) {
    return process.env.THIRDWEB_CLIENT_ID;
  }
  if (typeof window !== "undefined") {
    return window.__DCL_PUBLIC__?.thirdwebClientId ?? "";
  }
  return "";
}

export function hasThirdwebClientId(): boolean {
  return thirdwebClientId().length > 0;
}

/** The sign proxy calls thirdweb (a third party) with this secret, so only a mirror node (NODE_POSTURE=mirror)
 *  uses it; an island, the journey stack or a site that names no posture signs nothing, as the binary refuses its
 *  own third-party secrets off mirror. */
export function thirdwebSecretKey(
  env: Record<string, string | undefined> | undefined = typeof process !== "undefined" ? process.env : undefined,
): string {
  const key = env?.THIRDWEB_SECRET_KEY ?? "";
  return key && env?.NODE_POSTURE?.trim() === "mirror" ? key : "";
}
