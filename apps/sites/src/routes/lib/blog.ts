import { redirect, type HeadersArgs, type RouterContextProvider } from "react-router";

import type { BlogOutcome } from "@/data/lib/blog/pages";
import type { BlogListPayload } from "@/data/lib/blog/types";
import { pageLoader, type PageDefaults } from "@/core/lib/telemetry/page-loader";

const FALLBACK: PageDefaults = { variant: "index_grid" };

/**
 * A blog loader's response: the payload with its status (the view renders a 404 or 503 page from it),
 * or a redirect. A 503 is never cached; nothing about the upstream is in it.
 */
export async function blogResponse<T>(
  request: Request,
  context: Readonly<RouterContextProvider>,
  load: Promise<BlogOutcome<T>>,
) {
  const { sid, wrap } = await pageLoader(request, context, FALLBACK);
  const outcome = await load;
  if ("location" in outcome) throw redirect(outcome.location, outcome.status);
  const response = wrap({ sid, ...outcome.payload }, { status: outcome.status });
  if (outcome.status === 503) {
    const headers = new Headers(response.init?.headers);
    headers.set("Cache-Control", "no-store");
    headers.set("Retry-After", "300");
    response.init = { ...response.init, headers };
  }
  return response;
}

/** Document headers: the loader's Cache-Control and Retry-After (set on a 503) win over the parent's. */
export function blogHeaders({ loaderHeaders, parentHeaders }: HeadersArgs): Headers {
  const headers = new Headers(parentHeaders);
  for (const name of ["Cache-Control", "Retry-After"]) {
    const value = loaderHeaders.get(name);
    if (value) headers.set(name, value);
  }
  return headers;
}

const MISSING_TITLES = { post: "Post not found", category: "Category not found", author: "Author not found" } as const;

/** A list page's title and description; a 404 or 503 names itself. */
export function blogListMeta(data: BlogListPayload | undefined) {
  if (!data || data.state === "unavailable") return [{ title: "Blog unavailable | Decentraland Blog" }];
  if (data.state === "not-found") return [{ title: `${MISSING_TITLES[data.kind]} | Decentraland Blog` }];
  return [
    { title: `${data.title} | Decentraland Blog` },
    ...(data.subtitle ? [{ name: "description", content: data.subtitle }] : []),
  ];
}
