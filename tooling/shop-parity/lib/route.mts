// Browser-side routing of a capture: every request that leaves the side's own origin is answered
// from the recording (pictures by placeholder, telemetry blocked); nothing reaches the network.
import type { BrowserContext, Route } from "playwright-core";
import type { Recording } from "./recording.mts";
import { imageSeed, placeholderFor } from "./servers.mts";
import { fromNodePath, ORIGINS, resolve } from "./services.mts";

export type SideLog = { blocked: string[]; unknown: string[]; images: number };

const CORS = { "access-control-allow-origin": "*", "access-control-allow-headers": "*", "access-control-allow-methods": "*", "cache-control": "no-store" };

/** The wearable preview iframe: a blank document; its box is a declared 3D mask on both sides. */
const BLANK_PREVIEW = "<!doctype html><html><body style=\"margin:0;background:transparent\"></body></html>";

export async function routeSide(context: BrowserContext, recording: Recording, side: string, origin: string): Promise<SideLog> {
  const log: SideLog = { blocked: [], unknown: [], images: 0 };
  // `route` does not see WebSockets: close every one before it connects, and list it as blocked.
  await context.routeWebSocket(/.*/, (ws) => {
    log.blocked.push(`ws:${new URL(ws.url()).host}`);
    ws.close().catch(() => {});
  });
  await context.route("**/*", async (route: Route) => {
    const request = route.request();
    const url = new URL(request.url());
    const method = request.method().toUpperCase();
    const r = resolve(url, method, origin, request.resourceType());
    try {
      if (r.kind === "own") return await route.continue();
      if (method === "OPTIONS") return await route.fulfill({ status: 204, headers: CORS });
      if (r.kind === "blocked") {
        log.blocked.push(url.host);
        return await route.fulfill({ status: 204, headers: CORS, body: "" });
      }
      if (r.kind === "image") {
        log.images++;
        if (recording.live) {
          const own = url.origin === origin ? fromNodePath("GET", url.pathname) : null;
          const real = own ? `${ORIGINS[own.service] ?? ORIGINS.peer}${own.path}` : url.href;
          await recording.learnImage(imageSeed(url), real);
        }
        return await route.fulfill({ status: 200, headers: { ...CORS, "content-type": "image/png" }, body: placeholderFor(recording, url) });
      }
      if (r.kind === "unknown") {
        log.unknown.push(`${method} ${url.host}${url.pathname}`);
        return await route.fulfill({ status: 404, headers: CORS, body: "" });
      }
      if (r.service === "wearable-preview") return await route.fulfill({ status: 200, headers: { ...CORS, "content-type": "text/html" }, body: BLANK_PREVIEW });
      const answer = await recording.answer({
        method,
        service: r.service,
        path: r.path,
        search: url.search,
        body: request.postDataBuffer() ?? undefined,
        headers: await request.allHeaders(),
        side,
      });
      return await route.fulfill({ status: answer.status, headers: { ...CORS, "content-type": answer.contentType }, body: answer.body });
    } catch (err) {
      if (/closed|disposed/i.test(String(err))) return;
      try {
        await route.fulfill({ status: 502, headers: CORS, body: String(err) });
      } catch {
        // the page went away
      }
    }
  });
  return log;
}
