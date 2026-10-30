// /docs/*: the docs.decentraland.org mirror from content/docs. Files, Markdown alternates and
// redirects answer in middleware before any render; page URLs render DocsPage.
import { data, redirect } from "react-router";

import {
  docsFileResponse,
  docsManifest,
  docsMarkdownResponse,
  docsPageView,
  resolveDocsRequest,
} from "@/data/lib/docs/content.server";
import DocsPage from "@/features/docs/DocsPage";
import type { Route } from "./+types/docs.$";

export const middleware: Route.MiddlewareFunction[] = [
  async ({ request }, next) => {
    const url = new URL(request.url);
    if (url.pathname.endsWith(".data")) return next();
    const target = resolveDocsRequest(url, request);
    if (target.kind === "redirect") return redirect(target.location, 301);
    if (target.kind === "markdown") return docsMarkdownResponse(target.page);
    if (target.kind === "file") {
      if (request.method !== "GET" && request.method !== "HEAD") return new Response(null, { status: 405, headers: { Allow: "GET, HEAD" } });
      return docsFileResponse(target, request.method);
    }
    if (target.kind === "missing" && /\.[^/]+$/.test(url.pathname)) {
      return new Response("Not found\n", { status: 404, headers: { "Content-Type": "text/plain; charset=utf-8" } });
    }
    return next();
  },
];

export function loader({ request }: Route.LoaderArgs) {
  const url = new URL(request.url);
  url.pathname = url.pathname.replace(/\.data$/, "");
  const target = resolveDocsRequest(url);
  if (target.kind === "redirect") throw redirect(target.location, 301);
  if (target.kind !== "page") throw data(null, { status: 404, statusText: docsManifest() ? "Not Found" : "Docs content missing" });
  return docsPageView(target.page);
}

export function headers() {
  return { Vary: "Accept", "Cache-Control": "public, max-age=300" };
}

export function meta({ loaderData: view }: Route.MetaArgs) {
  if (!view) return [{ title: "Not found | Decentraland Docs" }];
  const title = view.title === "Decentraland Docs" ? view.title : `${view.title} | Decentraland Docs`;
  return [
    { title },
    ...(view.description ? [{ name: "description", content: view.description }] : []),
    { name: "robots", content: "index,follow" },
    { tagName: "link", rel: "icon", href: "/docs/player/images/favicon.png" },
    { tagName: "link", rel: "alternate", type: "text/markdown", href: view.mdUrl },
  ];
}

export default function DocsRoute({ loaderData }: Route.ComponentProps) {
  return <DocsPage view={loaderData} />;
}
