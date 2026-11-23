import fs from "node:fs";
import { fileURLToPath } from "node:url";
import { RouterContextProvider } from "react-router";
import { beforeEach, describe, expect, it } from "vitest";

import { markPublicResponse, requestSessionContext } from "@/core/lib/telemetry/session";

import { headers as accountHeaders, loader as accountLoader } from "./account";
import { loader as notFoundLoader } from "./account_.$";

const WALLET = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";

function request(url: string, wallet?: string) {
  return new Request(url, { headers: wallet ? { cookie: `dcl_wallet=${wallet}` } : {} });
}

function thrown(fn: () => unknown): unknown {
  try {
    fn();
  } catch (error) {
    return error;
  }
  throw new Error("expected the loader to throw");
}

type Wrapped = { data: { sid: string }; init?: { status?: number } };

describe("/account layout loader", () => {
  let context: RouterContextProvider;

  beforeEach(() => {
    context = new RouterContextProvider();
  });

  describe("when the reader has no wallet cookie", () => {
    let response: Response;

    beforeEach(() => {
      response = thrown(() => accountLoader({ request: request("https://site.test/account/credits?utm=mail"), context } as never)) as Response;
    });

    it("should redirect", () => {
      expect(response.status).toBe(302);
    });

    it("should send the reader to sign in, coming back to the same page and query", () => {
      expect(response.headers.get("Location")).toBe(`/auth/login?redirectTo=${encodeURIComponent("/account/credits?utm=mail")}`);
    });
  });

  describe("when the wallet cookie is malformed", () => {
    let response: Response;

    beforeEach(() => {
      response = thrown(() =>
        accountLoader({ request: request("https://site.test/account/notifications", "not-a-wallet"), context } as never),
      ) as Response;
    });

    it("should not take it as a sign-in and send the reader to sign in", () => {
      expect(response.headers.get("Location")).toMatch(/^\/auth\/login\?redirectTo=/);
    });
  });

  describe("when the reader is signed in", () => {
    let result: Wrapped;

    beforeEach(() => {
      context.set(requestSessionContext, { sid: "from-middleware", created: false });

      result = accountLoader({ request: request("https://site.test/account/notifications", WALLET), context } as never) as unknown as Wrapped;
    });

    it("should let the reader through with the request's session", () => {
      expect(result.data).toEqual({ sid: "from-middleware" });
    });
  });

  describe("when the request is on the account host", () => {
    let response: Response;

    beforeEach(() => {
      response = thrown(() => accountLoader({ request: request("https://account.site.test/account/notifications"), context } as never)) as Response;
    });

    it("should move the request to the same page on the main host first", () => {
      expect(response.headers.get("Location")).toBe("https://site.test/account/notifications");
    });
  });

  describe("when the context is marked public", () => {
    beforeEach(() => {
      markPublicResponse(context);
    });

    it("should refuse it, so no sid reaches a shared cache", () => {
      expect(() => accountLoader({ request: request("https://site.test/account", WALLET), context } as never)).toThrow(/publicly cacheable/);
    });
  });
});

describe("/account headers", () => {
  it("should mark every account page private and uncacheable", () => {
    expect(accountHeaders()).toEqual({ "Cache-Control": "private, no-store" });
  });
});

describe("/account unknown sections", () => {
  describe.each([
    ["signed out", undefined],
    ["signed in", WALLET],
  ])("when the reader is %s", (_case, wallet) => {
    let error: { init: { status: number } };

    beforeEach(() => {
      error = thrown(() => notFoundLoader({ request: request("https://site.test/account/wallets", wallet) } as never)) as {
        init: { status: number };
      };
    });

    it("should answer 404", () => {
      expect(error.init.status).toBe(404);
    });
  });

  describe("when the request is on the account host", () => {
    let response: Response;

    beforeEach(() => {
      response = thrown(() => notFoundLoader({ request: request("https://account.site.test/account/nope") } as never)) as Response;
    });

    it("should move the request before answering", () => {
      expect(response.headers.get("Location")).toBe("https://site.test/account/nope");
    });
  });
});

describe("account route tree", () => {
  describe("when listing the emailed confirmation routes", () => {
    let confirmations: string[];

    beforeEach(() => {
      const routes = fs.readdirSync(fileURLToPath(new URL(".", import.meta.url)));
      confirmations = routes.filter((file) => /confirm|email-confirmed/.test(file) && !file.includes(".test."));
    });

    it("should find at least one", () => {
      expect(confirmations.length).toBeGreaterThan(0);
    });

    it("should keep them outside the signed-in layout", () => {
      expect(confirmations.every((file) => file.startsWith("account_."))).toBe(true);
    });
  });
});
