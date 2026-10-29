import { PassThrough } from "node:stream";

import type { EntryContext } from "react-router";
import { createReadableStreamFromReadable } from "@react-router/node";
import { ServerRouter } from "react-router";
import { isbot } from "isbot";
import type { RenderToPipeableStreamOptions } from "react-dom/server";
import { renderToPipeableStream } from "react-dom/server";

import { markdownResponse, wantsMarkdown } from "@/data/lib/agent/markdown";
import { renderAgentMarkdown } from "@/data/lib/agent/dispatch.server";
import { ensureSid } from "@/core/lib/telemetry/session";
import { track } from "@/core/lib/telemetry/track";

import { applyFrameIsolation } from "./lib/frame-isolation";

export const streamTimeout = 5_000;

export default function handleRequest(
  request: Request,
  responseStatusCode: number,
  responseHeaders: Headers,
  routerContext: EntryContext,
  _loadContext: unknown,
) {
  const pathname = new URL(request.url).pathname;
  // The Explorer's native Shop runtime descriptor: /shop-native, and /marketplace-native for Explorer
  // builds that still ask for the old name (the same runtime).
  if (pathname === "/shop-native" || pathname === "/marketplace-native") {
    const runtime = routerContext.manifest.routes["routes/shop-native"];
    if (!runtime) return Response.json({ error: "Shop runtime unavailable" }, { status: 503 });
    const styles = [...new Set(Object.values(routerContext.manifest.routes)
      .filter(route => route && (route.id === "routes/shop" || route.id === "routes/shop-native"))
      .flatMap(route => route?.css ?? []))];
    const hmr = routerContext.manifest.hmr;
    // Explorer can be a built document while sites runs through Vite. Its
    // imported routes need the same preamble normally installed by <Scripts>.
    const module = hmr ? "/src/routes/shop/native-dev.browser.ts" : runtime.module;
    return Response.json({ version: 1, module, styles }, { headers: { "Cache-Control": "no-store" } });
  }
  if (wantsMarkdown(request)) {
    const hit = renderAgentMarkdown(routerContext);
    if (hit) return markdownResponse(hit.md, { status: hit.status });
  }
  applyFrameIsolation(request, responseHeaders);

  if (responseStatusCode === 404) {
    try {
      const { sid } = ensureSid(request);
      const u = new URL(request.url);
      track(
        "page_not_found",
        {
          path: u.pathname + u.search,
          referrer: request.headers.get("referer") ?? "",
          ua: request.headers.get("user-agent") ?? "",
        },
        { sid, story: "sites-error" },
      );
    } catch {
    }
  }

  return new Promise((resolve, reject) => {
    let shellRendered = false;
    const userAgent = request.headers.get("user-agent");

    const readyOption: keyof RenderToPipeableStreamOptions =
      (userAgent && isbot(userAgent)) || routerContext.isSpaMode
        ? "onAllReady"
        : "onShellReady";

    const { pipe, abort } = renderToPipeableStream(
      <ServerRouter context={routerContext} url={request.url} />,
      {
        [readyOption]() {
          shellRendered = true;
          const body = new PassThrough();
          const stream = createReadableStreamFromReadable(body);

          responseHeaders.set("Content-Type", "text/html");

          resolve(
            new Response(stream, {
              headers: responseHeaders,
              status: responseStatusCode,
            }),
          );

          pipe(body);
        },
        onShellError(error: unknown) {
          reject(error);
        },
        onError(error: unknown) {
          responseStatusCode = 500;
          if (shellRendered) {
            console.error(error);
          }
        },
      },
    );

    setTimeout(abort, streamTimeout + 1_000);
  });
}
