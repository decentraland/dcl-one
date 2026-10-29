import type { AuthIdentity } from "../data/auth/identity";
import type { Eip1193Provider } from "../data/auth/wallet";

export type CreatorHubHost = {
  version: 1;
  identity(): AuthIdentity | null;
  subscribe(listener: (identity: AuthIdentity | null) => void): () => void;
  signIn(): void;
  walletProvider(): Eip1193Provider | null;
  openLink(url: string): void;
};

const HOST_KEY = "__dclCreatorHubHost";
type HostWindow = Window & { [HOST_KEY]?: CreatorHubHost };

export const CREATOR_HUB_HOME = "/create";
export const CREATOR_HUB_SCOPE = [CREATOR_HUB_HOME] as const;

export function isCreatorHubPath(path: string): boolean {
  return /^\/create(?:[/?#]|$)/.test(path);
}

export function installCreatorHubHost(win: Window, host: CreatorHubHost): () => void {
  (win as HostWindow)[HOST_KEY] = host;
  return () => {
    if ((win as HostWindow)[HOST_KEY] === host) delete (win as HostWindow)[HOST_KEY];
  };
}

export function creatorHubHost(win: Window): CreatorHubHost | null {
  try {
    if (win.parent === win) return null;
    const host = (win.parent as HostWindow)[HOST_KEY];
    return host?.version === 1 ? host : null;
  } catch {
    return null;
  }
}

const WALLET_COOKIE = "dcl_wallet";
const WEBSITE_SESSION_KEY = "dcl:auth:identity:v1";

function readWalletCookie(doc: Document): string | null {
  for (const part of doc.cookie.split(";")) {
    const [name, ...value] = part.trim().split("=");
    if (name !== WALLET_COOKIE) continue;
    try {
      return decodeURIComponent(value.join("=")) || null;
    } catch {
      return null;
    }
  }
  return null;
}

function writeWalletCookie(doc: Document, address: string | null, maxAge = 0): void {
  const secure = doc.location?.protocol === "https:" ? "; Secure" : "";
  doc.cookie = address && maxAge > 0
    ? `${WALLET_COOKIE}=${encodeURIComponent(address)}; Path=/; SameSite=Lax; Max-Age=${maxAge}${secure}`
    : `${WALLET_COOKIE}=; Path=/; Max-Age=0${secure}`;
}

function secondsUntil(expiration: unknown): number {
  const at = typeof expiration === "string" ? Date.parse(expiration) : NaN;
  return Number.isFinite(at) ? Math.max(0, Math.floor((at - Date.now()) / 1000)) : 0;
}

function websiteWallet(doc: Document): { address: string; maxAge: number } | null {
  try {
    const stored = JSON.parse(doc.defaultView?.localStorage.getItem(WEBSITE_SESSION_KEY) ?? "null") as { signer?: unknown; expiration?: unknown } | null;
    const maxAge = secondsUntil(stored?.expiration);
    return typeof stored?.signer === "string" && maxAge > 0 ? { address: stored.signer.toLowerCase(), maxAge } : null;
  } catch {
    return null;
  }
}

export function scopeWalletCookie(doc: Document, identity: AuthIdentity | null): () => void {
  const maxAge = secondsUntil(identity?.expiration);
  const next = identity && maxAge > 0 ? identity.signer.toLowerCase() : null;
  if (readWalletCookie(doc) !== next) writeWalletCookie(doc, next, maxAge);
  return () => {
    if (readWalletCookie(doc) !== next) return;
    const website = websiteWallet(doc);
    writeWalletCookie(doc, website?.address ?? null, website?.maxAge);
  };
}
