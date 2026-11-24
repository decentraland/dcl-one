import { RouterContextProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/data/lib/download/counts.server", () => ({
  loadDownloadCounts: vi.fn(),
}));
vi.mock("@/data/lib/catalyst/landings/home.server", () => ({
  loadHomeLive: vi.fn(),
}));
vi.mock("@/features/home/cards.server", () => ({
  whatsOnCards: () => [],
}));
vi.mock("@/features/home/content", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/features/home/content")>()),
  homeAgentContent: (content: unknown) => content,
}));

import { requestSessionContext } from "@/core/lib/telemetry/session";
import { loadHomeLive } from "@/data/lib/catalyst/landings/home.server";
import { loadDownloadCounts } from "@/data/lib/download/counts.server";
import { loader } from "./_index";

type Wrapped = { data: { sid: string }; init?: { headers?: HeadersInit } };

const request = (cookie?: string) => new Request("https://sites.example/", { headers: cookie ? { cookie } : {} });
const setCookies = (result: Wrapped) => new Headers(result.init?.headers).getSetCookie();

describe("/ loader session", () => {
  beforeEach(() => {
    vi.mocked(loadDownloadCounts).mockResolvedValueOnce("+438K");
    vi.mocked(loadHomeLive).mockResolvedValueOnce({
      content: {},
      cards: [],
      profiles: {},
    } as never);
  });

  afterEach(() => {
    vi.resetAllMocks();
  });

  it("tags the Google Play link with the visitor's campaign from the request URL", async () => {
    const context = new RouterContextProvider();
    context.set(requestSessionContext, { sid: "s", created: false });
    const result = (await loader({
      request: new Request("https://sites.example/?utm_source=Partner&utm_term=Avatars"),
      context,
    } as never)) as unknown as { data: { googlePlay: string } };
    expect(Object.fromEntries(new URL(result.data.googlePlay).searchParams)).toMatchObject({
      utm_org: "dclrgl",
      utm_source: "partner",
      utm_term: "avatars",
      referrer: "utm_org=dclrgl&utm_source=partner&utm_medium=qr&utm_campaign=dclpage&utm_content=android&utm_term=avatars",
    });
  });

  describe("when the root middleware set the session", () => {
    let result: Wrapped;

    beforeEach(async () => {
      const context = new RouterContextProvider();
      context.set(requestSessionContext, { sid: "from-middleware", created: true });

      result = (await loader({ request: request(), context } as never)) as unknown as Wrapped;
    });

    it("should use the session the middleware set", () => {
      expect(result.data.sid).toBe("from-middleware");
    });

    it("should leave the cookie to the middleware", () => {
      expect(setCookies(result)).toEqual([]);
    });
  });

  describe("when no middleware ran", () => {
    describe("and the request has no sid cookie", () => {
      let result: Wrapped;

      beforeEach(async () => {
        result = (await loader({ request: request(), context: new RouterContextProvider() } as never)) as unknown as Wrapped;
      });

      it("should mint a session", () => {
        expect(result.data.sid).toMatch(/.+/);
      });

      it("should set the session cookie", () => {
        expect(setCookies(result)).toHaveLength(1);
        expect(setCookies(result)[0]).toContain(`sid=${encodeURIComponent(result.data.sid)}`);
      });
    });

    describe("and the request has a sid cookie", () => {
      let result: Wrapped;

      beforeEach(async () => {
        result = (await loader({ request: request("sid=s1"), context: new RouterContextProvider() } as never)) as unknown as Wrapped;
      });

      it("should keep the existing sid", () => {
        expect(result.data.sid).toBe("s1");
      });

      it("should set no cookie", () => {
        expect(setCookies(result)).toEqual([]);
      });
    });
  });
});
