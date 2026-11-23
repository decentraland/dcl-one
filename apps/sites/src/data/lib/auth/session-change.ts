import { useEffect, useRef } from "react";
import { useRevalidator, useRouteLoaderData } from "react-router";

import { dropTrackedUser, restoreTrackedUser } from "@/core/lib/telemetry/tracked-user";

import { isIdentityExpired } from "./expiry";
import type { AuthIdentity } from "./types";

/**
 * The verified-wallet cookie (`dcl_wallet_verified`) changes only through `POST /auth/callback`, a
 * plain fetch the router never sees, so root's data (the session's verified wallet, which page views
 * send as `userId`) would stay as it was. A completed post marks the session changed; root's
 * shouldRevalidate reloads its data while a change has not reached it.
 *
 * The mark is cleared when root's data reloads after a shouldRevalidate that asked for it, not inside
 * shouldRevalidate: React Router also calls it to decide what a `prefetch="intent"` link preloads
 * (react-router `dom/ssr/links.js`, getNewMatchesForLinks), so hovering a link would otherwise use
 * the mark up without any navigation.
 */

export const AUTH_CALLBACK_PATH = "/auth/callback";

let generation = 0;
/** The change root's data has not taken yet; 0 for none. */
let pending = 0;
/** The change a root reload was last asked for. */
let requested = 0;
let revalidateRoot: (() => void) | null = null;
/** A reload was asked for before the router could take it (the page had not hydrated yet). */
let reloadBeforeMount = false;
/** The change carrying the server's answer to a sign-out; 0 while none has answered. */
let signOutAnswered = 0;

/**
 * Marks the session changed, and reloads the page's data now when `revalidate` is set; asked before
 * the router is mounted, the reload runs as soon as it is.
 */
export function markSessionChanged(opts: { revalidate?: boolean } = {}): void {
  generation += 1;
  pending = generation;
  if (!opts.revalidate) return;
  if (revalidateRoot) revalidateRoot();
  else reloadBeforeMount = true;
}

/** For root's shouldRevalidate: true while a change has not reached root's data. */
export function sessionChangePending(): boolean {
  if (!pending) return false;
  requested = pending;
  return true;
}

/** Root's data reloaded: the change a reload was asked for has reached it. */
export function sessionChangeApplied(): void {
  // Root's data read after the sign-out's answer has no verified wallet: it takes over from the drop.
  if (signOutAnswered && requested >= signOutAnswered) {
    signOutAnswered = 0;
    restoreTrackedUser();
  }
  if (pending && requested === pending) pending = 0;
}

/** Test-only: forgets every change, request and pending reload, as a fresh document starts. */
export function resetSessionChangeForTests(): void {
  generation = 0;
  pending = 0;
  requested = 0;
  revalidateRoot = null;
  reloadBeforeMount = false;
  signOutAnswered = 0;
  resetAuthCallbackQueueForTests();
}

/**
 * A sign-out starts: page views leave the account out from now on, without waiting for the server,
 * and root's data is read again on the next navigation. Call it before posting the sign-out.
 */
export function beginSignOut(): void {
  signOutAnswered = 0;
  dropTrackedUser();
  markSessionChanged();
}

/**
 * Another tab signed the account out: it posted the sign-out, which cleared the verified-wallet
 * cookie this tab shares. Page views here leave the account out from now on, and root's data is
 * read again in place.
 */
export function signedOutElsewhere(): void {
  beginSignOut();
  markSessionChanged({ revalidate: true });
}

/** What a caller learns from `/auth/callback`'s answer. */
export type AuthCallbackAnswer = { ok: boolean; status: number };

/** What the verified-wallet cookie should end up saying: a sign-in to prove, or nobody. */
type Intent = { token: string } | { identity: AuthIdentity } | { signout: true };
type PostOptions = { keepalive?: boolean; revalidate?: boolean; clearOnFailure?: boolean };

/** Tabs on this origin share the cookie: one post at a time across them. */
const COOKIE_LOCK = "dcl:auth:verified-wallet";
/** The latest intent of this document; only its answer moves the tracked user. */
let latestIntent = 0;
let queue: Promise<unknown> = Promise.resolve();

/**
 * Posts to `/auth/callback`, which sets or clears the verified-wallet cookie on every answer it
 * gives; a post that got an answer marks the session changed, one that failed on the way (offline,
 * aborted) does not, since no cookie changed. Resolves to the answer's status, or null.
 *
 * Posts run one at a time, in the order they were asked for, so the cookie ends with the last intent;
 * an earlier answer that arrives while a later intent is queued does not restore or settle anything.
 */
export function postAuthCallback(
  body: { token: string } | { signout: true },
  opts: { keepalive?: boolean; revalidate?: boolean } = {},
): Promise<AuthCallbackAnswer | null> {
  return enqueue(body, opts);
}

/**
 * Sets the verified-wallet cookie for a sign-in that holds no in-app token (Magic, a browser wallet,
 * WalletConnect): the session signs the post, challenge included, and `/auth/callback` verifies its
 * auth chain and keeps the wallet that signed it. Resolves as `postAuthCallback` does; null as well
 * when the session could not sign (expired), which posts nothing.
 */
export function confirmSignIn(
  identity: AuthIdentity,
  opts: { revalidate?: boolean } = {},
): Promise<AuthCallbackAnswer | null> {
  return enqueue({ identity }, opts);
}

/**
 * The wallet suspended the session (it locked, or moved to an account with no usable session): the
 * account is not signed in here any more, so page views leave it out at once and its cookie is
 * cleared. Coming back, it is proven again (`switchSignedInAccount`).
 */
export function suspendSignedInAccount(): void {
  beginSignOut();
  void enqueue({ signout: true }, { keepalive: true, revalidate: true });
}

/**
 * The wallet moved the session to another account without a sign-in page: page views leave the
 * account out until the server names the new one, then root's data reloads in place. A wrong account
 * is worse than none, so when the new one is not proven (refused, its session could not sign, the
 * post never arrived) and nothing newer was asked for since, the cookie is cleared.
 */
export function switchSignedInAccount(identity: AuthIdentity): Promise<AuthCallbackAnswer | null> {
  signOutAnswered = 0;
  dropTrackedUser();
  return enqueue({ identity }, { revalidate: true, clearOnFailure: true });
}

/** Forgets queued intents, as a fresh document starts (resetSessionChangeForTests). */
function resetAuthCallbackQueueForTests(): void {
  latestIntent = 0;
  queue = Promise.resolve();
}

function enqueue(intent: Intent, opts: PostOptions): Promise<AuthCallbackAnswer | null> {
  const mine = ++latestIntent;
  const run = async () => {
    const answer = await send(intent, mine, opts);
    if (opts.clearOnFailure && !answer?.ok && mine === latestIntent) {
      await send({ signout: true }, mine, { revalidate: opts.revalidate });
    }
    return answer;
  };
  const locks = typeof navigator !== "undefined" ? navigator.locks : undefined;
  const turn = () => (locks ? locks.request(COOKIE_LOCK, run) : run());
  const result = queue.then(turn, turn);
  queue = result.catch(() => {});
  return result;
}

async function call(body: unknown, opts: { keepalive?: boolean }, headers: Record<string, string> = {}): Promise<Response | null> {
  try {
    return await fetch(AUTH_CALLBACK_PATH, {
      method: "POST",
      headers: { ...headers, "Content-Type": "application/json" },
      body: JSON.stringify(body),
      ...(opts.keepalive ? { keepalive: true } : {}),
    });
  } catch {
    return null;
  }
}

/** The one-time challenge a sign-in answers (an HttpOnly cookie of this browser holds its twin). */
async function challenge(): Promise<string | AuthCallbackAnswer | null> {
  const response = await call({ challenge: true }, {});
  if (!response) return null;
  if (!response.ok) {
    void response.body?.cancel().catch(() => {});
    return { ok: false, status: response.status };
  }
  try {
    const body = (await response.json()) as { challenge?: unknown };
    return typeof body.challenge === "string" ? body.challenge : { ok: false, status: response.status };
  } catch {
    return { ok: false, status: response.status };
  }
}

async function send(intent: Intent, mine: number, opts: PostOptions): Promise<AuthCallbackAnswer | null> {
  let response: Response | null;
  if ("signout" in intent) {
    response = await call({ signout: true }, opts);
  } else {
    if ("identity" in intent && isIdentityExpired(intent.identity)) return null;
    // Asking for a challenge changes no wallet cookie: a refusal here settles nothing.
    const nonce = await challenge();
    if (typeof nonce !== "string") return nonce;
    if ("token" in intent) {
      response = await call({ token: intent.token, challenge: nonce }, opts);
    } else {
      let headers: Record<string, string>;
      try {
        const { signRequest } = await import("./signer");
        headers = (await signRequest(intent.identity, "POST", AUTH_CALLBACK_PATH, { challenge: nonce })).headers;
      } catch {
        return null;
      }
      response = await call({ challenge: nonce }, opts, headers);
    }
  }
  // A sign-out that never reached the server keeps the account left out (the visitor asked to sign
  // out) and the reload beginSignOut asked for.
  if (!response) return null;
  void response.body?.cancel().catch(() => {});
  const answer = { ok: response.ok, status: response.status };
  markSessionChanged({ revalidate: opts.revalidate });
  // Only a successful answer to the latest intent settles the change: after a failed one (a 5xx, a
  // gateway error) the cookie may be unchanged, and an older one is about to be overwritten.
  if (!answer.ok || mine !== latestIntent) return answer;
  if ("signout" in intent) signOutAnswered = generation;
  else {
    // A sign-in replaces whatever sign-out was in progress.
    signOutAnswered = 0;
    restoreTrackedUser();
  }
  return answer;
}

/**
 * Mounted once, inside the router: lets a session change reload the page's data in place, and marks
 * the change applied when root's data reloads. Mounted again after the root error page gave way
 * (`App` unmounts while it shows), it applies a change a reload was asked for: root's data in hand
 * came from a navigation that ran the root loader while the change was pending.
 */
export function useSessionChangeRevalidation(): void {
  const revalidator = useRevalidator();
  const latest = useRef(revalidator);
  latest.current = revalidator;
  useEffect(() => {
    const revalidate = () => void latest.current.revalidate();
    revalidateRoot = revalidate;
    if (requested) sessionChangeApplied();
    if (reloadBeforeMount) {
      reloadBeforeMount = false;
      revalidate();
    }
    return () => {
      if (revalidateRoot === revalidate) revalidateRoot = null;
    };
  }, []);
  const rootData = useRouteLoaderData("root");
  const seen = useRef(rootData);
  useEffect(() => {
    if (seen.current === rootData) return;
    seen.current = rootData;
    sessionChangeApplied();
  }, [rootData]);
}
