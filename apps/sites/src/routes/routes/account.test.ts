import fs from "node:fs";
import { fileURLToPath } from "node:url";
import { RouterContextProvider } from "react-router";
import { describe, expect, it } from "vitest";

import { markPublicResponse, requestSessionContext } from "@/core/lib/telemetry/session";

import { headers as accountHeaders, loader as accountLoader } from "./account";
import { loader as indexLoader } from "./account._index";
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
  it("sends a reader without a wallet cookie to sign in, coming back to the same page and query", () => {
    const response = thrown(() =>
      accountLoader({ request: request("https://site.test/account/credits?utm=mail"), context: new RouterContextProvider() } as never),
    ) as Response;

    expect(response.status).toBe(302);
    expect(response.headers.get("Location")).toBe(`/auth/login?redirectTo=${encodeURIComponent("/account/credits?utm=mail")}`);
  });

  it("does not take a malformed wallet cookie as a sign-in", () => {
    const response = thrown(() =>
      accountLoader({ request: request("https://site.test/account/notifications", "not-a-wallet"), context: new RouterContextProvider() } as never),
    ) as Response;

    expect(response.headers.get("Location")).toMatch(/^\/auth\/login\?redirectTo=/);
  });

  it("lets a signed-in reader through with the request's session", () => {
    const context = new RouterContextProvider();
    context.set(requestSessionContext, { sid: "from-middleware", created: false });

    const result = accountLoader({ request: request("https://site.test/account/notifications", WALLET), context } as never) as unknown as Wrapped;

    expect(result.data).toEqual({ sid: "from-middleware" });
  });

  it("moves a request on the account host to the same page on the main host first", () => {
    const response = thrown(() =>
      accountLoader({ request: request("https://account.site.test/account/notifications"), context: new RouterContextProvider() } as never),
    ) as Response;

    expect(response.headers.get("Location")).toBe("https://site.test/account/notifications");
  });

  it("refuses a context marked public, so no sid reaches a shared cache", () => {
    const context = new RouterContextProvider();
    markPublicResponse(context);

    expect(() => accountLoader({ request: request("https://site.test/account", WALLET), context } as never)).toThrow(/publicly cacheable/);
  });

  it("marks every account page private and uncacheable", () => {
    expect(accountHeaders()).toEqual({ "Cache-Control": "private, no-store" });
  });
});

describe("/account index", () => {
  it("opens the notifications section, keeping the query", () => {
    const response = indexLoader({ request: request("https://site.test/account?utm=mail") } as never) as Response;

    expect(response.status).toBe(302);
    expect(response.headers.get("Location")).toBe("/account/notifications?utm=mail");
  });
});

describe("/account unknown sections", () => {
  it("answers 404 whether or not the reader is signed in", () => {
    for (const wallet of [undefined, WALLET]) {
      const error = thrown(() => notFoundLoader({ request: request("https://site.test/account/wallets", wallet) } as never)) as {
        init: { status: number };
      };
      expect(error.init.status).toBe(404);
    }
  });

  it("moves an account-host request before answering", () => {
    const response = thrown(() => notFoundLoader({ request: request("https://account.site.test/account/nope") } as never)) as Response;

    expect(response.headers.get("Location")).toBe("https://site.test/account/nope");
  });
});

describe("account route tree", () => {
  const routes = fs.readdirSync(fileURLToPath(new URL(".", import.meta.url)));

  it("keeps the emailed confirmation links outside the signed-in layout", () => {
    const confirmations = routes.filter((file) => /confirm|email-confirmed/.test(file) && !file.includes(".test."));

    expect(confirmations.length).toBeGreaterThan(0);
    expect(confirmations.every((file) => file.startsWith("account_."))).toBe(true);
  });

  it("nests only the settings sections under the layout", () => {
    const nested = routes.filter((file) => file.startsWith("account.") && !file.includes(".test.")).sort();

    expect(nested).toEqual(["account._index.tsx", "account.credits.tsx", "account.notifications.tsx", "account.tsx"]);
  });
});
