import { fileURLToPath } from "node:url";
import { flatRoutes } from "@react-router/fs-routes";
import { matchRoutes, type RouteObject } from "react-router";
import { beforeAll, describe, expect, it } from "vitest";

type Entry = { id?: string; path?: string; index?: boolean; file: string; children?: Entry[] };

/** The route tree flat routes builds from the real files, as `src/routes/routes.ts` does. */
let routes: RouteObject[];
beforeAll(async () => {
  (globalThis as { __reactRouterAppDirectory?: string }).__reactRouterAppDirectory = fileURLToPath(
    new URL(".", import.meta.url),
  );
  const tree = (await flatRoutes({ ignoredRouteFiles: ["**/*.test.*", "**/*.spec.*", "**/*.module.css"] })) as Entry[];
  const toObject = (entry: Entry): RouteObject =>
    ({ id: entry.file, path: entry.path, index: entry.index, children: entry.children?.map(toObject) }) as RouteObject;
  routes = tree.map(toObject);
});

/** The file of the deepest route serving `path`, or null when no route serves it (the 404 page). */
const served = (path: string) => matchRoutes(routes, path)?.at(-1)?.route.id ?? null;

describe("Sites paths", () => {
  describe("when a visitor opens an old /landings path", () => {
    it.each([
      "/landings/download",
      "/landings/download/success",
      "/landings/invite-referral",
      "/landings/report-abuse",
      "/landings/cast-not-found",
      "/landings/creator-hub-download",
    ])("should serve no route for %s, so it answers 404", (path) => {
      expect(served(path)).toBeNull();
    });
  });

  describe("when a visitor opens a canonical path", () => {
    it.each([
      ["/download", "routes/download.tsx"],
      ["/download_success", "routes/download_success.tsx"],
      ["/download/creator-hub", "routes/download_.creator-hub.tsx"],
      ["/invite", "routes/invite.($referrer).tsx"],
      ["/invite/0xabc", "routes/invite.($referrer).tsx"],
      ["/report", "routes/report._index.tsx"],
      ["/cast", "routes/cast._index.tsx"],
    ])("should serve %s from %s", (path, file) => {
      expect(served(path)).toBe(file);
    });
  });
});
