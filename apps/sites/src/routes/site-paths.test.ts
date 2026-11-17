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
      "/landings/home",
      "/landings/cast-watch",
      "/landings/cast-stream",
      "/landings/event-schedule",
      "/landings/invite-referral",
      "/landings/report-abuse",
      "/landings/cast-not-found",
      "/landings/creator-hub-download",
      "/landings/event-subscriptions",
      "/landings/rsvp-event",
      "/landings/submit-hangout",
      "/landings/community-detail",
      "/landings/create-community",
    ])("should serve no route for %s, so it answers 404", (path) => {
      expect(served(path)).toBeNull();
    });
  });

  describe("when a visitor opens a community by id outside /social", () => {
    it.each(["/communities/c-1", "/communities/427638bd-35c2-4592-bee3-7a33d0883d8a"])(
      "should serve no route for %s, so it answers 404",
      (path) => {
        expect(served(path)).toBeNull();
      },
    );

    it("should serve the community page at /social/communities/:id", () => {
      expect(served("/social/communities/c-1")).toBe("routes/social_.communities.$id.tsx");
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
      ["/cast/w/0,0", "routes/cast.w.$location.tsx"],
      ["/cast/s/key", "routes/cast.s.$token.tsx"],
      ["/", "routes/_index.tsx"],
      ["/events/subscriptions", "routes/events_.subscriptions.tsx"],
      ["/events/ev-1/rsvp", "routes/events_.$id.rsvp.tsx"],
      ["/events/new-event", "routes/events_.new-event.tsx"],
      ["/events/edit-event/ev-1", "routes/events_.edit-event.$id.tsx"],
      ["/communities/new", "routes/communities.new.tsx"],
    ])("should serve %s from %s", (path, file) => {
      expect(served(path)).toBe(file);
    });
  });
});
