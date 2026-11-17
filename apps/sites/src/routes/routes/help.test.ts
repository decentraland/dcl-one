import { RouterContextProvider } from "react-router";
import { describe, expect, it } from "vitest";

import { requestSessionContext } from "@/core/lib/telemetry/session";
import { DEFAULT_INTERCOM_APP_ID } from "@/data/lib/help/config.server";
import { DEFAULT_STATUS_PAGE_URL, DEFAULT_STATUS_SERVICES } from "@/data/lib/help/status";

import { loader, meta } from "./help";

describe("/help loader", () => {
  it("hands the page the production services, status page and Intercom workspace by default", async () => {
    const result = (await loader({ request: new Request("https://site.example/help"), context: new RouterContextProvider() } as never)) as unknown as {
      data: Record<string, unknown>;
    };
    expect(result.data).toMatchObject({
      services: DEFAULT_STATUS_SERVICES,
      statusPageUrl: DEFAULT_STATUS_PAGE_URL,
      intercomAppId: DEFAULT_INTERCOM_APP_ID,
      variant: "config_driven",
    });
    expect(typeof result.data.sid).toBe("string");
  });
});

describe("/help loader session", () => {
  type Wrapped = { data: { sid: string }; init?: { headers?: HeadersInit } };
  const call = async (context: RouterContextProvider, cookie?: string) =>
    (await loader({
      request: new Request("https://site.example/help", { headers: cookie ? { cookie } : {} }),
      context,
    } as never)) as unknown as Wrapped;
  const setCookies = (result: Wrapped) => new Headers(result.init?.headers).getSetCookie();

  it("mints one sid and sets one cookie when no middleware ran", async () => {
    const result = await call(new RouterContextProvider());

    expect(setCookies(result)).toHaveLength(1);
    expect(setCookies(result)[0]).toContain(`sid=${encodeURIComponent(result.data.sid)}`);
  });

  it("keeps an existing sid cookie and sets none", async () => {
    const result = await call(new RouterContextProvider(), "sid=s1");

    expect(result.data.sid).toBe("s1");
    expect(setCookies(result)).toEqual([]);
  });

  it("uses the middleware's session and leaves the cookie to it", async () => {
    const context = new RouterContextProvider();
    context.set(requestSessionContext, { sid: "from-middleware", created: true });

    const result = await call(context);

    expect(result.data.sid).toBe("from-middleware");
    expect(setCookies(result)).toEqual([]);
  });
});

describe("/help meta", () => {
  it("names the page after the help center", () => {
    expect(meta()[0]).toEqual({ title: "Decentraland - Help Center" });
  });
});
