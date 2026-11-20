import type { RouterContextProvider } from "react-router";
import { redirect } from "react-router";

import { ownedSessionCookies, requestSession } from "@/core/lib/telemetry/session";
import { track } from "@/core/lib/telemetry/track";

export const REDIRECT_EVENT = "creator_builder_redirect";

export const REDIRECT_STORY = "creator-hub/integration-redirect-item-publish-curate";

const PERMANENT = 308;

export type RedirectRule = {
  from: string;
  to: string;
  carry: readonly string[];
  param?: { name: string };
};

export const REDIRECTS = {
  "item-editor": {
    from: "item-editor",
    to: "/create/collections/editor",
    carry: ["collection", "item", "reviewing"],
  },
  "publish-collection": {
    from: "publish-collection",
    to: "/create/collections/:collection",
    carry: [],
    param: { name: "collection" },
  },
  curation: {
    from: "curation",
    to: "/create/curation",
    carry: ["status", "type", "assignee"],
  },
  "collection-detail": {
    from: "collection-detail",
    to: "/create/collections/:id",
    carry: [],
    param: { name: "id" },
  },
  // The editor needs the item's collection, which an item link does not carry.
  "item-detail": {
    from: "item-detail",
    to: "/create/collections",
    carry: [],
  },
} as const satisfies Record<string, RedirectRule>;

export type RedirectKey = keyof typeof REDIRECTS;

export function buildDestination(
  rule: RedirectRule,
  url: URL,
  params: Record<string, string | undefined>,
): string {
  const out = new URLSearchParams();

  for (const key of rule.carry) {
    for (const v of url.searchParams.getAll(key)) if (v !== "") out.append(key, v);
  }

  let to = rule.to;
  if (rule.param) {
    // The legacy page takes it in the path or, like publish-collection, in the query.
    const raw = params[rule.param.name] ?? url.searchParams.get(rule.param.name);
    to = raw != null && raw !== "" ? to.replace(`:${rule.param.name}`, encodeURIComponent(raw)) : "/create/collections";
  }

  const qs = out.toString();
  return qs ? `${to}?${qs}` : to;
}

export function builderRedirect(
  key: RedirectKey,
  request: Request,
  context: Readonly<RouterContextProvider>,
  params: Record<string, string | undefined> = {},
): Response {
  const rule = REDIRECTS[key];
  const url = new URL(request.url);
  const to = buildDestination(rule, url, params);

  const session = requestSession(request, context);
  const { sid } = session;

  track(
    REDIRECT_EVENT,
    {
      from: rule.from,
      to,
      fromPath: url.pathname + url.search,
    },
    { sid, story: REDIRECT_STORY },
  );

  const headers = new Headers();
  headers.set("Location", to);
  // Behind the root middleware the cookie is set once on the response; on its own this sets it.
  for (const [name, value] of ownedSessionCookies(request, session) ?? []) headers.append(name, value);

  return redirect(to, { status: PERMANENT, headers });
}
