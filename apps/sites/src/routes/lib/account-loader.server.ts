import { redirect, type RouterContextProvider } from "react-router";

import { sidLoader } from "@/core/lib/telemetry/session";
import { accountHostRedirect } from "@/data/lib/account/confirmation";
import { readWallet } from "@/data/lib/auth/wallet-cookie";
import { loginPath } from "@/features/auth/login/redirect";

/** Account pages show one member's settings: no shared cache may keep them. */
export const ACCOUNT_HEADERS = { "Cache-Control": "private, no-store" } as const;

export type AccountLoaderData = { sid: string };

/**
 * `/account/*`'s gate. Every account read is signed in the browser, which holds the session; the
 * server sees only the wallet cookie, so a reader without one signs in first and comes back here.
 */
export function accountLoader(request: Request, context: Readonly<RouterContextProvider>) {
  const moved = accountHostRedirect(request);
  if (moved) throw redirect(moved);
  if (!readWallet(request)) {
    const url = new URL(request.url);
    throw redirect(loginPath(`${url.pathname}${url.search}`));
  }
  const session = sidLoader(request, context);
  return session.wrap({ sid: session.sid } satisfies AccountLoaderData);
}
