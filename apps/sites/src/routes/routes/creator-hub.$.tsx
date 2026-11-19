import { redirect } from "react-router";

import { ownedSessionCookies, requestSession } from "@/core/lib/telemetry/session";
import { track } from "@/core/lib/telemetry/track";
import { retiredHubTarget } from "@/features/lib/creator/retired-hub";

import type { Route } from "./+types/creator-hub.$";

/** `/creator-hub/*` is the retired spelling of `/create/*`: one prefix. Pages that moved to the
    desktop app go to where that work lives now; everything else keeps its path and query. */
const RENAMED: Record<string, string> = {
  "multiplayer-test": "preview",
};

export function createPathFor(url: URL, splat: string | undefined): string {
  const path = (splat ?? "").replace(/^\/+|\/+$/g, "");
  const [head, ...rest] = path ? path.split("/") : [];
  const retired = retiredHubTarget(head);
  if (retired) return retired;
  const mapped = head ? [RENAMED[head] ?? head, ...rest].join("/") : "";
  return `/create${mapped ? `/${mapped}` : ""}${url.search}`;
}

function moved({ request, params, context }: Route.LoaderArgs | Route.ActionArgs) {
  const session = requestSession(request, context);
  const { sid } = session;
  // Behind the root middleware the cookie is set once on the response; only a direct call sets it here.
  const cookies = ownedSessionCookies(request, session);
  const url = new URL(request.url);
  const to = createPathFor(url, params["*"]);
  track("creator_builder_redirect", { from: url.pathname, to }, { sid });
  return redirect(to, {
    status: 308,
    headers: cookies,
  });
}

export const loader = moved;
export const action = moved;

export default function CreatorHubMoved() {
  return null;
}
