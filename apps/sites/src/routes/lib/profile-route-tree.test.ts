import { fileURLToPath } from "node:url";
import { flatRoutes } from "@react-router/fs-routes";
import { beforeAll, describe, expect, it } from "vitest";

type Entry = { id?: string; path?: string; file: string; children?: Entry[] };

/** The route tree flat routes builds from the real files, as `src/routes/routes.ts` does. */
let tree: Entry[];
beforeAll(async () => {
  (globalThis as { __reactRouterAppDirectory?: string }).__reactRouterAppDirectory = fileURLToPath(
    new URL("..", import.meta.url),
  );
  tree = (await flatRoutes({ ignoredRouteFiles: ["**/*.test.*", "**/*.spec.*", "**/*.module.css"] })) as Entry[];
});

const profileRoutes = () => tree.filter((entry) => entry.file.includes("routes/profile"));

describe("profile route tree", () => {
  it("keeps every profile route at the top level, so a tab page runs only its own loader", () => {
    const nested = profileRoutes().filter((entry) => (entry.children ?? []).length > 0);

    expect(nested.map((entry) => entry.file)).toEqual([]);
  });

  it("serves each profile path from exactly one route", () => {
    const paths = profileRoutes()
      .map((entry) => entry.path)
      .sort();

    expect(paths).toEqual([
      "profile",
      "profile/:address",
      "profile/:address/:tab",
      "profile/accounts/:address",
      "profile/accounts/:address/:tab",
      "profile/me",
      "profile/me/:tab",
    ]);
  });
});
