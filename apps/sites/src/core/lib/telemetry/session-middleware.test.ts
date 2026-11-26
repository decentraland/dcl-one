import { data, redirect, RouterContextProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { testRequestHandler } from "../server-build.test-helper";
import { markPublicResponse, pageSid, requestSession, sessionMiddleware, sidLoader } from "./session";

type Seen = { root?: string | null; page?: string; wallet?: string | null };
type Args = { request: Request; context: Readonly<RouterContextProvider> };

/**
 * The request pipeline `react-router-serve` runs (`createRequestHandler`), over a minimal build: the
 * root route with Sites' session middleware and a loader that records its sid, and child pages that
 * answer, fail, redirect or throw and record theirs.
 */
function app(seen: Seen) {
  const page = (module: Record<string, unknown>) => ({ default: () => null, ...module });
  const entry = {
    default: (_request: Request, status: number, headers: Headers, context: { loaderData: unknown }) =>
      new Response(JSON.stringify(context.loaderData), { status, headers }),
  };
  return testRequestHandler(entry, [
    { id: "root", path: "", module: page({
      middleware: [sessionMiddleware],
      loader: ({ request, context }: Args) => {
        seen.root = pageSid(request, context);
        return { sid: seen.root };
      },
      ErrorBoundary: () => null,
    }) },
    { id: "routes/page", parentId: "root", path: "page", module: page({
      loader: ({ request, context }: Args) => {
        const session = sidLoader(request, context);
        seen.page = session.sid;
        seen.wallet = session.wallet;
        return session.wrap({ sid: session.sid });
      },
    }) },
    { id: "routes/fail", parentId: "root", path: "fail", module: page({
      loader: ({ request, context }: Args) => {
        seen.page = sidLoader(request, context).sid;
        throw data(null, { status: 503 });
      },
    }) },
    { id: "routes/move", parentId: "root", path: "move", module: page({
      loader: ({ request, context }: Args) => {
        seen.page = requestSession(request, context).sid;
        throw redirect("/page");
      },
    }) },
    { id: "routes/crash", parentId: "root", path: "crash", module: page({
      loader: () => {
        throw new Error("boom");
      },
    }) },
    // A resource route with a publicly cacheable reply, like internal/feature-flags.
    { id: "routes/asset", parentId: "root", path: "asset", module: page({
      default: undefined,
      loader: () => Response.json({ ok: true }, { headers: { "Cache-Control": "public, max-age=60" } }),
    }) },
    // A reply only a shared cache may store, by s-maxage alone.
    { id: "routes/shared", parentId: "root", path: "shared", module: page({
      default: undefined,
      loader: () => Response.json({ ok: true }, { headers: { "Cache-Control": "s-maxage=60" } }),
    }) },
    // A public document route, like the docs: its middleware marks it before the loaders run.
    { id: "routes/docs", parentId: "root", path: "docs", module: page({
      middleware: [({ context }: Args, next: () => Promise<Response>) => (markPublicResponse(context), next())],
      loader: () => ({ page: "docs" }),
      headers: () => ({ "Cache-Control": "public, max-age=300" }),
    }) },
    // A child middleware that answers before any loader, like the docs files.
    { id: "routes/early", parentId: "root", path: "early", module: page({
      middleware: [() => new Response("early", { status: 200 })],
      loader: () => null,
    }) },
    // A loader that returns a response whose headers cannot change (Response.redirect).
    { id: "routes/frozen", parentId: "root", path: "frozen", module: page({
      loader: () => Response.redirect("https://sites.test/page", 302),
    }) },
  ]);
}

const get = (path: string, cookie?: string) =>
  new Request(`https://sites.test${path}`, { headers: cookie ? { cookie } : {} });
const sidCookies = (response: Response) => response.headers.getSetCookie().filter((cookie) => cookie.startsWith("sid="));
const cookieSid = (cookie: string) => decodeURIComponent(cookie.split(";")[0]!.slice("sid=".length));

describe("session middleware, through the request handler", () => {
  let seen: Seen;
  let response: Response;

  beforeEach(() => {
    seen = {};
  });

  describe("when a first visit asks for a page", () => {
    let cookies: string[];

    beforeEach(async () => {
      response = await app(seen)(get("/page"));
      cookies = sidCookies(response);
    });

    it("should answer the page", () => {
      expect(response.status).toBe(200);
    });

    it("should mint a sid in the root", () => {
      expect(seen.root).toBeTruthy();
    });

    it("should give the page the root's sid", () => {
      expect(seen.page).toBe(seen.root);
    });

    it("should set a single sid cookie with the root's sid", () => {
      expect(cookies).toHaveLength(1);
      expect(cookieSid(cookies[0]!)).toBe(seen.root);
    });

    it("should make the cookie HttpOnly", () => {
      expect(cookies[0]).toContain("HttpOnly");
    });

    it("should keep the cookie for a year", () => {
      expect(cookies[0]).toContain("Max-Age=31536000");
    });
  });

  describe("when the visit carries an existing session", () => {
    beforeEach(async () => {
      response = await app(seen)(get("/page", "sid=existing-session"));
    });

    it("should keep the session in the root", () => {
      expect(seen.root).toBe("existing-session");
    });

    it("should keep the session in the page", () => {
      expect(seen.page).toBe("existing-session");
    });

    it("should set no cookie", () => {
      expect(sidCookies(response)).toEqual([]);
    });
  });

  describe("when the sid cookie is empty", () => {
    beforeEach(async () => {
      response = await app(seen)(get("/page", "sid="));
    });

    it("should mint a sid in the root", () => {
      expect(seen.root).toBeTruthy();
    });

    it("should give the page the root's sid", () => {
      expect(seen.page).toBe(seen.root);
    });

    it("should set the minted sid's cookie", () => {
      expect(sidCookies(response).map(cookieSid)).toEqual([seen.root]);
    });
  });

  describe("when a client navigation's data request follows the document", () => {
    let sid: string;
    let navigation: Response;

    beforeEach(async () => {
      const document = await app(seen)(get("/page"));
      sid = cookieSid(sidCookies(document)[0]!);
      seen = {};
      navigation = await app(seen)(get("/page.data", `sid=${encodeURIComponent(sid)}`));
    });

    it("should answer the data request", () => {
      expect(navigation.status).toBe(200);
    });

    it("should serve it with the session the document set", () => {
      expect(seen.page).toBe(sid);
    });

    it("should set no cookie", () => {
      expect(sidCookies(navigation)).toEqual([]);
    });
  });

  describe("when a loader answers a first visit with an error status", () => {
    beforeEach(async () => {
      response = await app(seen)(get("/fail"));
    });

    it("should answer the error status", () => {
      expect(response.status).toBe(503);
    });

    it("should give the page the root's sid", () => {
      expect(seen.page).toBe(seen.root);
    });

    it("should set the one cookie", () => {
      expect(sidCookies(response).map(cookieSid)).toEqual([seen.root]);
    });
  });

  describe("when a loader redirects a first visit", () => {
    beforeEach(async () => {
      response = await app(seen)(get("/move"));
    });

    it("should answer the redirect", () => {
      expect(response.status).toBe(302);
      expect(response.headers.get("Location")).toBe("/page");
    });

    it("should give the page the root's sid", () => {
      expect(seen.page).toBe(seen.root);
    });

    it("should set the one cookie", () => {
      expect(sidCookies(response).map(cookieSid)).toEqual([seen.root]);
    });
  });

  describe("when a loader crashes", () => {
    beforeEach(async () => {
      response = await app(seen)(get("/crash"));
    });

    it("should still answer", () => {
      expect(response.status).toBe(500);
    });

    it("should set the cookie", () => {
      expect(sidCookies(response).map(cookieSid)).toEqual([seen.root]);
    });
  });

  describe("when two first visits interleave", () => {
    let other: Seen;
    let ra: Response;
    let rb: Response;

    beforeEach(async () => {
      other = {};
      [ra, rb] = await Promise.all([app(seen)(get("/page")), app(other)(get("/page"))]);
    });

    it("should give them different sessions", () => {
      expect(seen.root).not.toBe(other.root);
    });

    it("should set each visit's own sid cookie", () => {
      expect(cookieSid(sidCookies(ra)[0]!)).toBe(seen.root);
      expect(cookieSid(sidCookies(rb)[0]!)).toBe(other.root);
    });
  });
});

describe("session middleware and shared caches", () => {
  let seen: Seen;
  let response: Response;

  beforeEach(() => {
    seen = {};
  });

  describe("when a resource reply is publicly cacheable", () => {
    beforeEach(async () => {
      response = await app(seen)(get("/asset"));
    });

    it("should answer it", () => {
      expect(response.status).toBe(200);
    });

    it("should set no cookie", () => {
      expect(sidCookies(response)).toEqual([]);
    });
  });

  describe("when only a shared cache may store the reply (s-maxage)", () => {
    beforeEach(async () => {
      response = await app(seen)(get("/shared"));
    });

    it("should answer it", () => {
      expect(response.status).toBe(200);
    });

    it("should set no cookie", () => {
      expect(sidCookies(response)).toEqual([]);
    });
  });

  describe("when the route is a public document", () => {
    beforeEach(async () => {
      response = await app(seen)(get("/docs"));
    });

    it("should answer it as publicly cacheable", () => {
      expect(response.status).toBe(200);
      expect(response.headers.get("Cache-Control")).toContain("public");
    });

    it("should give the root data no session id", () => {
      expect(seen.root).toBeNull();
    });

    it("should set no cookie", () => {
      expect(sidCookies(response)).toEqual([]);
    });
  });

  describe("when a child middleware answers before the loaders", () => {
    beforeEach(async () => {
      response = await app(seen)(get("/early"));
    });

    it("should answer the middleware's reply", async () => {
      expect(await response.text()).toBe("early");
    });

    it("should still set the cookie", () => {
      expect(sidCookies(response)).toHaveLength(1);
    });
  });

  describe("when the reply's headers cannot change", () => {
    beforeEach(async () => {
      response = await app(seen)(get("/frozen"));
    });

    it("should answer the reply", () => {
      expect(response.status).toBe(302);
    });

    it("should set the cookie by copying the reply", () => {
      expect(sidCookies(response)).toHaveLength(1);
    });
  });
});

describe("session middleware and the shared cookie domain", () => {
  afterEach(() => vi.unstubAllEnvs());

  describe("when the sid cookie is split across domains", () => {
    let seen: Seen;
    let cookies: string[];

    beforeEach(async () => {
      vi.stubEnv("SID_COOKIE_DOMAIN", "sites.test");
      seen = {};
      cookies = sidCookies(await app(seen)(get("/page", "sid=first; sid=second")));
    });

    it("should set two cookies", () => {
      expect(cookies).toHaveLength(2);
    });

    it("should move the root's sid to the shared domain once", () => {
      expect(cookies[0]).toContain(`sid=${seen.root}`);
      expect(cookies[0]).toContain("Domain=sites.test");
    });

    it("should expire the host-only cookie", () => {
      expect(cookies[1]).toContain("Max-Age=0");
    });
  });
});

describe("sign-in and sign-out", () => {
  const wallet = `0x${"a".repeat(40)}`;
  let signedIn: Seen;
  let signedOut: Seen;
  let rin: Response;
  let rout: Response;

  beforeEach(async () => {
    signedIn = {};
    signedOut = {};
    rin = await app(signedIn)(get("/page", `sid=s1; dcl_wallet_verified=${wallet}`));
    rout = await app(signedOut)(get("/page", "sid=s1"));
  });

  describe("when the member is signed in", () => {
    it("should keep the session id and read the wallet", () => {
      expect(signedIn).toMatchObject({ root: "s1", page: "s1", wallet });
    });
  });

  describe("when the member is signed out", () => {
    it("should keep the session id and read no wallet", () => {
      expect(signedOut).toMatchObject({ root: "s1", page: "s1", wallet: null });
    });
  });

  it("should set no cookie on either", () => {
    expect([...sidCookies(rin), ...sidCookies(rout)]).toEqual([]);
  });
});

describe("sidLoader without the middleware", () => {
  describe("when called on its own", () => {
    let session: ReturnType<typeof sidLoader>;
    let wrapped: { init?: { headers?: Headers } };

    beforeEach(() => {
      session = sidLoader(get("/page"), new RouterContextProvider());
      wrapped = session.wrap({}) as unknown as { init?: { headers?: Headers } };
    });

    it("should mint the session", () => {
      expect(session.created).toBe(true);
    });

    it("should set the cookie itself, as before", () => {
      expect(new Headers(wrapped.init?.headers).getSetCookie().map(cookieSid)).toEqual([session.sid]);
    });
  });

  describe("when the route is marked public", () => {
    let context: RouterContextProvider;

    beforeEach(() => {
      context = new RouterContextProvider();
      markPublicResponse(context);
    });

    it("should refuse, so no sid reaches shared page data", () => {
      expect(() => sidLoader(get("/docs"), context)).toThrow(/publicly cacheable/);
    });
  });
});
