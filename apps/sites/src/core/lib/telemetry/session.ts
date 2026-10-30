import { data } from "react-router";

const SID_COOKIE = "sid";
const SID_MAX_AGE = 60 * 60 * 24 * 365;
const WALLET_COOKIE = "dcl_wallet_verified";
const WALLET_MAX_AGE = 60 * 60 * 24 * 30;
const ADDRESS = /^0x[0-9a-f]{40}$/;

export function parseCookies(header: string | null | undefined): Record<string, string> {
  const out: Record<string, string> = {};
  if (!header) return out;
  for (const part of header.split(";")) {
    const eq = part.indexOf("=");
    if (eq === -1) continue;
    const key = part.slice(0, eq).trim();
    if (!key) continue;
    const value = part.slice(eq + 1).trim();
    try {
      out[key] = decodeURIComponent(value);
    } catch {
      out[key] = value;
    }
  }
  return out;
}

export function readSid(request: Request): string | null {
  const sid = parseCookies(request.headers.get("cookie"))[SID_COOKIE];
  return sid && sid.length > 0 ? sid : null;
}

function sidSharedParent(): string | undefined {
  const raw = typeof process !== "undefined" ? process.env?.SID_COOKIE_DOMAIN : undefined;
  const parent = raw?.trim().replace(/^\./, "").toLowerCase();
  return parent ? parent : undefined;
}

export function sharedSidDomain(request: Request): string | undefined {
  const parent = sidSharedParent();
  if (!parent) return undefined;
  let hostname: string;
  try {
    hostname = new URL(request.url).hostname;
  } catch {
    return undefined;
  }
  return hostname === parent || hostname.endsWith(`.${parent}`) ? parent : undefined;
}

export function hasSplitSidCookie(request: Request): boolean {
  const header = request.headers.get("cookie");
  if (!header) return false;
  let seen = 0;
  for (const part of header.split(";")) {
    const eq = part.indexOf("=");
    if (eq !== -1 && part.slice(0, eq).trim() === SID_COOKIE) seen += 1;
  }
  return seen > 1;
}

export function createSid(): string {
  try {
    if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
      return crypto.randomUUID();
    }
  } catch {
  }
  let sid = "";
  for (let i = 0; i < 32; i++) sid += Math.floor(Math.random() * 16).toString(16);
  return sid;
}

export function ensureSid(request: Request): { sid: string; created: boolean } {
  const existing = readSid(request);
  if (existing) return { sid: existing, created: false };
  return { sid: createSid(), created: true };
}

export function serializeSidCookie(
  sid: string,
  opts?: { domain?: string },
): string {
  return [
    `${SID_COOKIE}=${encodeURIComponent(sid)}`,
    "Path=/",
    `Max-Age=${SID_MAX_AGE}`,
    "SameSite=Lax",
    "HttpOnly",
    "Secure",
    ...(opts?.domain ? [`Domain=${opts.domain}`] : []),
  ].join("; ");
}

export function expireSidCookie(opts?: { domain?: string }): string {
  return [
    `${SID_COOKIE}=`,
    "Path=/",
    "Max-Age=0",
    "SameSite=Lax",
    "HttpOnly",
    "Secure",
    ...(opts?.domain ? [`Domain=${opts.domain}`] : []),
  ].join("; ");
}

export function readVerifiedWallet(request: Request): string | null {
  const wallet = parseCookies(request.headers.get("cookie"))[WALLET_COOKIE];
  return wallet && ADDRESS.test(wallet) ? wallet : null;
}

export function serializeVerifiedWalletCookie(address: string): string {
  return [
    `${WALLET_COOKIE}=${encodeURIComponent(address.toLowerCase())}`,
    "Path=/",
    `Max-Age=${WALLET_MAX_AGE}`,
    "SameSite=Lax",
    "HttpOnly",
    "Secure",
  ].join("; ");
}

export function clearVerifiedWalletCookie(): string {
  return [
    `${WALLET_COOKIE}=`,
    "Path=/",
    "Max-Age=0",
    "SameSite=Lax",
    "HttpOnly",
    "Secure",
  ].join("; ");
}

export function sidLoader(request: Request) {
  const { sid, created } = ensureSid(request);
  const wallet = readVerifiedWallet(request);
  const domain = sharedSidDomain(request);
  let headers: Headers | undefined;
  if (created) {
    headers = new Headers({ "Set-Cookie": serializeSidCookie(sid, { domain }) });
  } else if (domain && hasSplitSidCookie(request)) {
    headers = new Headers();
    headers.append("Set-Cookie", serializeSidCookie(sid, { domain }));
    headers.append("Set-Cookie", expireSidCookie());
  }
  return {
    sid,
    wallet,
    userKey: wallet ?? sid,
    created,
    wrap: <T,>(payload: T, init?: { status?: number }) =>
      data(payload, { status: init?.status, headers }),
  };
}