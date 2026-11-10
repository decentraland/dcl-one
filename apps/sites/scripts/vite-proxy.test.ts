import { IncomingMessage, ServerResponse } from "node:http";
import { Socket } from "node:net";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { nodeProxy, proxyContext, sitesApiRoutes } from "../vite.config";

describe("when matching the development gateway proxy", () => {
  let routes: ReturnType<typeof nodeProxy>;
  let matching: (url: string) => string | undefined;

  beforeEach(() => {
    routes = nodeProxy();
    matching = (url) =>
      Object.keys(routes).find((context) => new RegExp(context).test(url));
  });

  it.each([
    "/credits",
    "/credits?wallet=guest",
    "/credits/balance?cursor=1",
    "/about",
    "/about?realm=one",
    "/content/",
    "/content/contents/hash?download=1",
    "/contents/hash",
    "/api/places?limit=1",
    "/places/api/places",
    "/events/api/events",
    "/shop-api/orders",
    "/world-storage/users/me",
    "/auth-api/session",
    "/media/convert?src=a",
    "/_play/session",
    "/world-manifest.json",
    "/WorldManifest.json?cache=1",
  ])("should retain backend routing for %s", (url) => {
    expect(matching(url)).toBeDefined();
  });

  it.each([
    "/credits-terms",
    "/credits-terms?next=/credits",
    "/credits-extra",
    "/credits%2Dterms",
    "/about-us",
    "/content",
    "/content-policy",
    "/api",
    "/places/apiary/places",
    "/shop-apiary",
    "/world-storage-docs",
    "/rpc-example",
    "/social-rpc-guide",
    "/world-manifestXjson",
    "/world-manifest.jsonld",
  ])("should leave the lookalike page %s to Sites", (url) => {
    expect(matching(url)).toBeUndefined();
  });

  it("should escape literal regex punctuation in prefix names", () => {
    expect(
      new RegExp(proxyContext("/a+b.test")).test("/a+b.test?value=1"),
    ).toBe(true);
    expect(new RegExp(proxyContext("/a+b.test")).test("/aaabXtest")).toBe(
      false,
    );
  });

  it.each(["/rpc", "/social-rpc"])(
    "should retain websocket forwarding for %s",
    (prefix) => {
      expect(routes[proxyContext(prefix)]?.ws).toBe(true);
      expect(matching(prefix + "?transport=websocket")).toBe(
        proxyContext(prefix),
      );
      expect(matching(prefix + "/room")).toBe(proxyContext(prefix));
    },
  );

  it("should retain gateway targets and origin rewriting for every rule", () => {
    expect(
      Object.values(routes).every(
        (options) => options.target && options.changeOrigin,
      ),
    ).toBe(true);
    expect(
      Object.keys(routes).every((context) => context.startsWith("^")),
    ).toBe(true);
  });
});

describe("when a Sites resource route shares the gateway API namespace", () => {
  let options: ReturnType<typeof nodeProxy>[string];
  let request: IncomingMessage;
  let response: ServerResponse;

  beforeEach(() => {
    options = nodeProxy()[proxyContext("/api/")];
    request = new IncomingMessage(new Socket());
    response = new ServerResponse(request);
  });

  afterEach(() => request.destroy());

  it.each([
    "/api/screens/v1/play",
    "/api/screens?locale=en",
    "/api/creator-hub/asset-catalog",
    "/api/creator-hub?next=/api/places",
    "/api/builder-foundation/scene",
    "/api/marketplace-screen?view=all",
  ])("should bypass the gateway for the Sites route %s", async (url) => {
    request.url = url;
    expect(await options.bypass!(request, response, options)).toBe(url);
  });

  it.each([
    "/api/places",
    "/api/places?next=/api/screens",
    "/api/screenshots",
    "/api/creator-hubris",
    "/api/builder-foundations",
    "/api/marketplace-screens",
    "/api/screens%2Fv1/play",
  ])("should keep the unrelated API route %s on the gateway", async (url) => {
    request.url = url;
    expect(await options.bypass!(request, response, options)).toBeUndefined();
  });

  it("should fail closed when the request URL is missing", async () => {
    request.url = undefined;
    expect(await options.bypass!(request, response, options)).toBeUndefined();
  });

  it("should continue deriving resource namespaces from the route files", () => {
    expect(sitesApiRoutes()).toEqual([
      "/api/builder-foundation",
      "/api/creator-hub",
      "/api/marketplace-screen",
      "/api/screens",
    ]);
  });
});
