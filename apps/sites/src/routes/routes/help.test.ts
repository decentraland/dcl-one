import { RouterContextProvider } from "react-router";
import { beforeEach, describe, expect, it } from "vitest";

import { requestSessionContext } from "@/core/lib/telemetry/session";
import { DEFAULT_INTERCOM_APP_ID } from "@/data/lib/help/config.server";
import { DEFAULT_STATUS_PAGE_URL, DEFAULT_STATUS_SERVICES } from "@/data/lib/help/status";

import { loader, meta } from "./help";

type Wrapped = { data: { sid: string } & Record<string, unknown>; init?: { headers?: HeadersInit } };

const call = async (context: RouterContextProvider, cookie?: string) =>
  (await loader({
    request: new Request("https://site.example/help", { headers: cookie ? { cookie } : {} }),
    context,
  } as never)) as unknown as Wrapped;
const setCookies = (result: Wrapped) => new Headers(result.init?.headers).getSetCookie();

describe("/help loader", () => {
  let result: Wrapped;

  describe("when no configuration overrides the defaults", () => {
    beforeEach(async () => {
      result = await call(new RouterContextProvider());
    });

    it("should hand the page the production services, status page and Intercom workspace", () => {
      expect(result.data).toMatchObject({
        services: DEFAULT_STATUS_SERVICES,
        statusPageUrl: DEFAULT_STATUS_PAGE_URL,
        intercomAppId: DEFAULT_INTERCOM_APP_ID,
        variant: "config_driven",
      });
    });

    it("should hand the page a session id", () => {
      expect(typeof result.data.sid).toBe("string");
    });
  });

  describe("when no middleware ran", () => {
    describe("and the request has no sid cookie", () => {
      beforeEach(async () => {
        result = await call(new RouterContextProvider());
      });

      it("should mint one sid and set one cookie for it", () => {
        expect(setCookies(result)).toHaveLength(1);
        expect(setCookies(result)[0]).toContain(`sid=${encodeURIComponent(result.data.sid)}`);
      });
    });

    describe("and the request has a sid cookie", () => {
      beforeEach(async () => {
        result = await call(new RouterContextProvider(), "sid=s1");
      });

      it("should keep the existing sid", () => {
        expect(result.data.sid).toBe("s1");
      });

      it("should set no cookie", () => {
        expect(setCookies(result)).toEqual([]);
      });
    });
  });

  describe("when the middleware set the session", () => {
    beforeEach(async () => {
      const context = new RouterContextProvider();
      context.set(requestSessionContext, { sid: "from-middleware", created: true });

      result = await call(context);
    });

    it("should use the middleware's session", () => {
      expect(result.data.sid).toBe("from-middleware");
    });

    it("should leave the cookie to the middleware", () => {
      expect(setCookies(result)).toEqual([]);
    });
  });
});

describe("/help meta", () => {
  describe("when the root loader answered the public page base", () => {
    let args: Parameters<typeof meta>[0];

    beforeEach(() => {
      args = { matches: [{ loaderData: { publicPageBase: "https://public.test" } }] } as never;
    });

    it("should carry the title, the description and the canonical path", () => {
      expect(meta(args)).toEqual([
        { title: "Help & Support | Decentraland" },
        { name: "description", content: "Find answers, troubleshoot issues, and get the most out of Decentraland." },
        { tagName: "link", rel: "canonical", href: "https://public.test/help" },
      ]);
    });
  });
});
