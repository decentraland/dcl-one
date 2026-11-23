import { useEffect, useRef } from "react";
import { useRevalidator, useRouteLoaderData } from "react-router";

import { dropTrackedUser, restoreTrackedUser } from "@/core/lib/telemetry/tracked-user";

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

/** What a caller learns from `/auth/callback`'s answer; its body is never read. */
export type AuthCallbackAnswer = { ok: boolean; status: number };

/**
 * Posts to `/auth/callback`, which sets or clears the verified-wallet cookie on every answer it
 * gives; a post that got an answer marks the session changed, one that failed on the way (offline,
 * aborted) does not, since no cookie changed. Resolves to the answer's status, or null.
 */
export async function postAuthCallback(
  body: { token: string } | { signout: true },
  opts: { keepalive?: boolean; revalidate?: boolean } = {},
): Promise<AuthCallbackAnswer | null> {
  return post(body, {}, opts);
}

/**
 * Sets the verified-wallet cookie for a sign-in that holds no in-app token (Magic, a browser wallet,
 * WalletConnect): the session signs the post, and `/auth/callback` verifies its auth chain and keeps
 * the wallet that signed it. Resolves as `postAuthCallback` does; null as well when the session could
 * not sign (expired), which posts nothing.
 */
export async function confirmSignIn(
  identity: AuthIdentity,
  opts: { revalidate?: boolean } = {},
): Promise<AuthCallbackAnswer | null> {
  let headers: Record<string, string>;
  try {
    const { signRequest } = await import("./signer");
    headers = (await signRequest(identity, "POST", AUTH_CALLBACK_PATH)).headers;
  } catch {
    return null;
  }
  return post({}, headers, opts);
}

/**
 * The wallet moved the session to another account without a sign-in page: page views leave the
 * account out until the server names the new one, then root's data reloads in place. A wrong account
 * is worse than none, so when the new one cannot be proven (its session could not sign, the post
 * never arrived) the cookie naming the previous one is cleared; a refusal clears it on its own.
 */
export async function switchSignedInAccount(identity: AuthIdentity): Promise<AuthCallbackAnswer | null> {
  signOutAnswered = 0;
  dropTrackedUser();
  const answer = await confirmSignIn(identity, { revalidate: true });
  if (answer === null) await postAuthCallback({ signout: true }, { revalidate: true });
  return answer;
}

async function post(
  body: { token: string } | { signout: true } | Record<string, never>,
  signed: Record<string, string>,
  opts: { keepalive?: boolean; revalidate?: boolean },
): Promise<AuthCallbackAnswer | null> {
  let response: Response;
  try {
    response = await fetch(AUTH_CALLBACK_PATH, {
      method: "POST",
      headers: { ...signed, "Content-Type": "application/json" },
      body: JSON.stringify(body),
      ...(opts.keepalive ? { keepalive: true } : {}),
    });
  } catch {
    // A sign-out that never reached the server keeps the account left out (the visitor asked to sign
    // out) and the reload beginSignOut asked for.
    return null;
  }
  void response.body?.cancel().catch(() => {});
  const answer = { ok: response.ok, status: response.status };
  markSessionChanged({ revalidate: opts.revalidate });
  // Only a successful answer settles the change: after a failed one (a 5xx, a gateway error) the
  // cookie may be unchanged, so a sign-out keeps the account left out and a sign-in restores nothing.
  if (!answer.ok) return answer;
  if ("signout" in body) signOutAnswered = generation;
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
