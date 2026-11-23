import { randomBytes, timingSafeEqual } from "node:crypto";

import { clearVerifiedWalletCookie, parseCookies, serializeVerifiedWalletCookie } from "@/core/lib/telemetry/session";

import { AUTH_METADATA_HEADER } from "./signer";
import { getWalletForToken } from "./thirdweb/index";
import { verifyAuthChainRequest } from "./verify-request.server";

/**
 * `POST /auth/callback`: the only writer of the verified-wallet cookie that page views send as the
 * Segment `userId`. It answers only this site's own JSON posts, and changes no cookie for any other
 * request. A sign-in answers a challenge this browser asked for just before: a one-time nonce kept
 * in an HttpOnly cookie, which an in-app token post repeats in its body and a signed post also signs
 * in its metadata, and which every sign-in answer deletes.
 */

const CHALLENGE_COOKIE = "dcl_wallet_challenge";
const CHALLENGE_MAX_AGE = 5 * 60;
/** 32 random bytes, base64url. */
const CHALLENGE = /^[A-Za-z0-9_-]{43}$/;

type Intent =
  | { kind: "signout" }
  | { kind: "challenge" }
  | { kind: "token"; token: string; challenge: string }
  | { kind: "signed"; challenge: string };

function json(body: unknown, status: number, cookies: string[] = []): Response {
  const headers = new Headers();
  for (const cookie of cookies) headers.append("Set-Cookie", cookie);
  return Response.json(body, { status, headers });
}

/** The origin the browser addressed: the gateway terminates TLS and says so in `X-Forwarded-Proto`. */
function publicOrigin(request: Request): string {
  const url = new URL(request.url);
  const scheme = request.headers.get("x-forwarded-proto");
  if (scheme === "http" || scheme === "https") url.protocol = `${scheme}:`;
  return url.origin;
}

/** Why the request is not this site's own JSON post, or null when it is. */
function untrusted(request: Request): Response | null {
  if (request.method !== "POST") return json({ ok: false }, 405);
  // Browsers send Origin on every POST; Sec-Fetch-Site, where they send it, must agree.
  const site = request.headers.get("sec-fetch-site");
  if (request.headers.get("origin") !== publicOrigin(request) || (site !== null && site !== "same-origin")) {
    return json({ ok: false }, 403);
  }
  const mediaType = request.headers.get("content-type")?.split(";")[0]?.trim().toLowerCase();
  if (mediaType !== "application/json") return json({ ok: false }, 415);
  return null;
}

function parseIntent(raw: string): Intent | null {
  let body: unknown;
  try {
    body = JSON.parse(raw);
  } catch {
    return null;
  }
  if (typeof body !== "object" || body === null || Array.isArray(body)) return null;
  const { signout, challenge, token, ...rest } = body as Record<string, unknown>;
  if (Object.keys(rest).length > 0) return null;
  if (signout === true && challenge === undefined && token === undefined) return { kind: "signout" };
  if (challenge === true && signout === undefined && token === undefined) return { kind: "challenge" };
  if (signout !== undefined || typeof challenge !== "string" || !CHALLENGE.test(challenge)) return null;
  if (token === undefined) return { kind: "signed", challenge };
  return typeof token === "string" && token.length > 0 ? { kind: "token", token, challenge } : null;
}

function challengeCookie(value: string, maxAge: number): string {
  return [
    `${CHALLENGE_COOKIE}=${value}`,
    "Path=/auth/callback",
    `Max-Age=${maxAge}`,
    "SameSite=Strict",
    "HttpOnly",
    "Secure",
  ].join("; ");
}

function sameChallenge(a: string, b: string | undefined): boolean {
  if (b === undefined || a.length !== b.length) return false;
  return timingSafeEqual(Buffer.from(a), Buffer.from(b));
}

/** The challenge the signed post signed in its metadata. */
function signedChallenge(request: Request): string | undefined {
  try {
    const metadata = JSON.parse(request.headers.get(AUTH_METADATA_HEADER) ?? "") as { challenge?: unknown };
    return typeof metadata?.challenge === "string" ? metadata.challenge : undefined;
  } catch {
    return undefined;
  }
}

async function provenWallet(request: Request, intent: Extract<Intent, { kind: "token" | "signed" }>): Promise<string | null> {
  // An in-app sign-in (e-mail, the in-app wallet) proves its wallet with its token; any other (Magic, a
  // browser wallet, WalletConnect) with a request its session signed, its challenge included.
  if (intent.kind === "token") return getWalletForToken(intent.token, request.signal);
  if (!sameChallenge(intent.challenge, signedChallenge(request))) return null;
  const verified = await verifyAuthChainRequest(request);
  return verified.ok ? verified.wallet : null;
}

export async function answerAuthCallback(request: Request): Promise<Response> {
  const refused = untrusted(request);
  if (refused) return refused;
  const intent = parseIntent(await request.text());
  if (!intent) return json({ ok: false }, 400);

  if (intent.kind === "signout") return json({ ok: true }, 200, [clearVerifiedWalletCookie()]);
  if (intent.kind === "challenge") {
    const challenge = randomBytes(32).toString("base64url");
    return json({ ok: true, challenge }, 200, [challengeCookie(challenge, CHALLENGE_MAX_AGE)]);
  }

  // One use only, whatever the answer.
  const consume = challengeCookie("", 0);
  const issued = parseCookies(request.headers.get("cookie"))[CHALLENGE_COOKIE];
  // Not this browser's challenge (another browser, a replay, a sign-in another tab started since): the
  // wallet cookie stays as it is, for whichever sign-in owns the challenge.
  if (!sameChallenge(intent.challenge, issued)) return json({ ok: false }, 401, [consume]);

  const address = await provenWallet(request, intent);
  // A sign-in this browser asked for that does not prove its wallet names nobody, never the account before it.
  if (!address) return json({ ok: false }, 401, [clearVerifiedWalletCookie(), consume]);
  return json({ ok: true, address }, 200, [serializeVerifiedWalletCookie(address), consume]);
}
