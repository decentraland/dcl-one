import { fileURLToPath } from "node:url";
import { flatRoutes } from "@react-router/fs-routes";
import { RouterContextProvider } from "react-router";
import { describe, expect, it } from "vitest";

import { sessionMiddleware } from "@/core/lib/telemetry/session";

import { loader } from "./robots[.]txt";

describe("when a crawler requests /robots.txt", () => {
  it("should allow every path as plain text", async () => {
    const response = loader();

    expect(response.status).toBe(200);
    expect(response.headers.get("Content-Type")).toBe(
      "text/plain; charset=utf-8",
    );
    expect(await response.text()).toBe("User-agent: *\nAllow: /\n");
  });

  it("should be publicly cacheable and carry no session cookie", async () => {
    const response = (await sessionMiddleware(
      {
        request: new Request("https://sites.example/robots.txt"),
        context: new RouterContextProvider(),
      } as unknown as Parameters<typeof sessionMiddleware>[0],
      async () => loader(),
    )) as Response;

    expect(response.headers.get("Cache-Control")).toBe("public, max-age=3600");
    expect(response.headers.get("Set-Cookie")).toBeNull();
  });

  it("should name no sitemap, since none is served", async () => {
    expect(await loader().text()).not.toMatch(/sitemap/i);
  });
});

describe("when the route table is built from the route files", () => {
  it("should serve this file at /robots.txt", async () => {
    (
      globalThis as { __reactRouterAppDirectory?: string }
    ).__reactRouterAppDirectory = fileURLToPath(new URL("..", import.meta.url));
    const tree = (await flatRoutes({
      ignoredRouteFiles: ["**/*.test.*", "**/*.spec.*", "**/*.module.css"],
    })) as { path?: string; file: string }[];

    expect(
      tree
        .filter((entry) => entry.path === "robots.txt")
        .map((entry) => entry.file),
    ).toEqual(["routes/robots[.]txt.ts"]);
  });
});
