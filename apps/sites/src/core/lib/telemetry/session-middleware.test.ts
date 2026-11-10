import { createRequestHandler, data, redirect, type ServerBuild } from "react-router";
import { describe, expect, it } from "vitest";

import { requestSession, sessionMiddleware, sidLoader } from "./session";

type Seen = { root?: string; page?: string; wallet?: string | null };

/**
 * The request pipeline `react-router-serve` runs (`createRequestHandler`), over a minimal build: the
 * root route with Sites' session middleware and a loader that records its sid, and child pages that
 * answer, fail, redirect or throw and record theirs.
 */
function app(seen: Seen) {
  const route = (id: string, parentId: string | undefined, path: string | undefined, module: Record<string, unknown>) => ({
    id,
    parentId,
    path,
    index: undefined,
    caseSensitive: undefined,
    module: { default: () => null, ...module },
  });
  const manifest = (id: string, parentId?: string) => ({
    id,
    parentId,
    path: undefined,
    module: `/${id}.js`,
    imports: [],
    css: [],
    hasAction: false,
    hasLoader: true,
    hasClientAction: false,
    hasClientLoader: false,
    hasClientMiddleware: false,
    hasErrorBoundary: id === "root",
  });
  const build = {
    entry: {
      module: {
        default: (_request: Request, status: number, headers: Headers, context: { loaderData: unknown }) =>
          new Response(JSON.stringify(context.loaderData), { status, headers }),
      },
    },
    routes: {
      root: route("root", undefined, "", {
        middleware: [sessionMiddleware],
        loader: ({ request, context }: { request: Request; context: unknown }) => {
          seen.root = requestSession(request, context).sid;
          return { sid: seen.root };
        },
        ErrorBoundary: () => null,
      }),
      "routes/page": route("routes/page", "root", "page", {
        loader: ({ request, context }: { request: Request; context: unknown }) => {
          const session = sidLoader(request, context);
          seen.page = session.sid;
          seen.wallet = session.wallet;
          return session.wrap({ sid: session.sid });
        },
      }),
      "routes/fail": route("routes/fail", "root", "fail", {
        loader: ({ request, context }: { request: Request; context: unknown }) => {
          seen.page = sidLoader(request, context).sid;
          throw data(null, { status: 503 });
        },
      }),
      "routes/move": route("routes/move", "root", "move", {
        loader: ({ request, context }: { request: Request; context: unknown }) => {
          seen.page = requestSession(request, context).sid;
          throw redirect("/page");
        },
      }),
      "routes/crash": route("routes/crash", "root", "crash", {
        loader: () => {
          throw new Error("boom");
        },
      }),
    },
    assets: {
      entry: { module: "/entry.js", imports: [] },
      url: "/manifest.js",
      version: "test",
      routes: {
        root: manifest("root"),
        "routes/page": manifest("routes/page", "root"),
        "routes/fail": manifest("routes/fail", "root"),
        "routes/move": manifest("routes/move", "root"),
        "routes/crash": manifest("routes/crash", "root"),
      },
    },
    publicPath: "/",
    assetsBuildDirectory: "build/client",
    basename: "/",
    future: {},
    ssr: true,
    isSpaMode: false,
    prerender: [],
    routeDiscovery: { mode: "initial" },
  } as unknown as ServerBuild;
  return createRequestHandler(build, "production");
}

const get = (path: string, cookie?: string) =>
  new Request(`https://sites.test${path}`, { headers: cookie ? { cookie } : {} });
const sidCookies = (response: Response) => response.headers.getSetCookie().filter((cookie) => cookie.startsWith("sid="));
const cookieSid = (cookie: string) => decodeURIComponent(cookie.split(";")[0]!.slice("sid=".length));

describe("session middleware, through the request handler", () => {
  it("gives a first visit one sid: the root, the page and the single cookie agree", async () => {
    const seen: Seen = {};

    const response = await app(seen)(get("/page"));

    expect(response.status).toBe(200);
    expect(seen.root).toBeTruthy();
    expect(seen.page).toBe(seen.root);
    const cookies = sidCookies(response);
    expect(cookies).toHaveLength(1);
    expect(cookieSid(cookies[0]!)).toBe(seen.root);
    expect(cookies[0]).toContain("HttpOnly");
    expect(cookies[0]).toContain("Max-Age=31536000");
  });

  it("keeps an existing session and sets no cookie", async () => {
    const seen: Seen = {};

    const response = await app(seen)(get("/page", "sid=existing-session"));

    expect(seen.root).toBe("existing-session");
    expect(seen.page).toBe("existing-session");
    expect(sidCookies(response)).toEqual([]);
  });

  it("treats an empty sid cookie as no session and mints one", async () => {
    const seen: Seen = {};

    const response = await app(seen)(get("/page", "sid="));

    expect(seen.root).toBeTruthy();
    expect(seen.page).toBe(seen.root);
    expect(sidCookies(response).map(cookieSid)).toEqual([seen.root]);
  });

  it("serves a client navigation's data request with the session the document set", async () => {
    const first: Seen = {};
    const document = await app(first)(get("/page"));
    const sid = cookieSid(sidCookies(document)[0]!);

    const later: Seen = {};
    const navigation = await app(later)(get("/page.data", `sid=${encodeURIComponent(sid)}`));

    expect(navigation.status).toBe(200);
    expect(later.page).toBe(sid);
    expect(sidCookies(navigation)).toEqual([]);
  });

  it("sets the one cookie on a first visit that a loader answers with an error status", async () => {
    const seen: Seen = {};

    const response = await app(seen)(get("/fail"));

    expect(response.status).toBe(503);
    expect(seen.page).toBe(seen.root);
    expect(sidCookies(response).map(cookieSid)).toEqual([seen.root]);
  });

  it("sets the one cookie on a first visit that a loader redirects", async () => {
    const seen: Seen = {};

    const response = await app(seen)(get("/move"));

    expect(response.status).toBe(302);
    expect(response.headers.get("Location")).toBe("/page");
    expect(seen.page).toBe(seen.root);
    expect(sidCookies(response).map(cookieSid)).toEqual([seen.root]);
  });

  it("still answers, with its cookie, when a loader crashes", async () => {
    const seen: Seen = {};

    const response = await app(seen)(get("/crash"));

    expect(response.status).toBe(500);
    expect(sidCookies(response).map(cookieSid)).toEqual([seen.root]);
  });

  it("gives two interleaved first visits different sessions", async () => {
    const a: Seen = {};
    const b: Seen = {};

    const [ra, rb] = await Promise.all([app(a)(get("/page")), app(b)(get("/page"))]);

    expect(a.root).not.toBe(b.root);
    expect(cookieSid(sidCookies(ra)[0]!)).toBe(a.root);
    expect(cookieSid(sidCookies(rb)[0]!)).toBe(b.root);
  });
});

describe("sign-in and sign-out", () => {
  it("keep the session id; only the wallet the page reads changes", async () => {
    const wallet = `0x${"a".repeat(40)}`;
    const signedIn: Seen = {};
    const signedOut: Seen = {};

    const rin = await app(signedIn)(get("/page", `sid=s1; dcl_wallet_verified=${wallet}`));
    const rout = await app(signedOut)(get("/page", "sid=s1"));

    expect(signedIn).toMatchObject({ root: "s1", page: "s1", wallet });
    expect(signedOut).toMatchObject({ root: "s1", page: "s1", wallet: null });
    expect([...sidCookies(rin), ...sidCookies(rout)]).toEqual([]);
  });
});

describe("sidLoader without the middleware", () => {
  it("mints and sets the cookie itself, as before, when called on its own", () => {
    const session = sidLoader(get("/page"), {});
    const wrapped = session.wrap({}) as unknown as { init?: { headers?: Headers } };

    expect(session.created).toBe(true);
    expect(new Headers(wrapped.init?.headers).getSetCookie().map(cookieSid)).toEqual([session.sid]);
  });
});
