import { RouterContextProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { markPublicResponse } from "@/core/lib/telemetry/session";

import { loader } from "./root";

afterEach(() => vi.unstubAllEnvs());

const load = async () => {
  const context = new RouterContextProvider();
  markPublicResponse(context);
  return loader({ request: new Request("https://example.test/"), params: {}, context } as never);
};

it("hands the page the build revision the image sets", async () => {
  vi.stubEnv("COMMIT_HASH", "abc123");
  expect((await load()).commitHash).toBe("abc123");
});

it("hands the page no revision when the image sets none", async () => {
  vi.stubEnv("COMMIT_HASH", "");
  expect((await load()).commitHash).toBe("");
});

it("hands the page the Pulse host the deployment sets", async () => {
  vi.stubEnv("PUBLIC_PULSE_SERVER", "pulse.example.test:7743");
  expect((await load()).pulseServer).toBe("pulse.example.test:7743");
});

it("hands the page no Pulse host when the deployment sets none", async () => {
  vi.stubEnv("PUBLIC_PULSE_SERVER", "");
  expect((await load()).pulseServer).toBe("");
});

describe("when the shared locale is read for an Events request", () => {
  let context: RouterContextProvider;
  beforeEach(() => { context = new RouterContextProvider(); });

  it.each([
    ["lang=es", "es"],
    ["lang=pt", "en"],
    ["", "en"],
  ])("should choose the allowlisted preference or English from %s", async (cookie, expected) => {
    const response = await loader({
      request: new Request("https://example.test/events", { headers: { cookie } }),
      params: {},
      context,
    } as never);
    expect(response.locale).toBe(expected);
  });

  it("should omit the preference from a shared-cacheable document", async () => {
    markPublicResponse(context);
    const response = await loader({
      request: new Request("https://example.test/docs/", { headers: { cookie: "lang=es" } }),
      params: {},
      context,
    } as never);
    expect(response.locale).toBeUndefined();
  });
});

describe("when the root loader names the Segment user", () => {
  const A = "0x1111111111111111111111111111111111111111";
  const B = "0x2222222222222222222222222222222222222222";
  const loadWith = (cookie: string, publicResponse = false) => {
    const context = new RouterContextProvider();
    if (publicResponse) markPublicResponse(context);
    return loader({ request: new Request("https://example.test/events", { headers: { cookie } }), params: {}, context } as never);
  };

  it("should name no user from the display wallet cookie alone", async () => {
    const data = await loadWith(`dcl_wallet=${A}`);
    expect(data.wallet).toBe(A);
    expect(data.analyticsUserId).toBeNull();
  });

  it("should name the verified wallet when the display cookie disagrees", async () => {
    expect((await loadWith(`dcl_wallet=${A}; dcl_wallet_verified=${B}`)).analyticsUserId).toBe(B);
  });

  it("should name no user once the verified cookie is gone (signed out)", async () => {
    expect((await loadWith(`dcl_wallet=${A}`)).analyticsUserId).toBeNull();
    expect((await loadWith("")).analyticsUserId).toBeNull();
  });

  it("should name the new account when the verified cookie changes between requests", async () => {
    expect((await loadWith(`dcl_wallet_verified=${A}`)).analyticsUserId).toBe(A);
    expect((await loadWith(`dcl_wallet_verified=${B}`)).analyticsUserId).toBe(B);
  });

  it("should ignore a verified cookie that is no address", async () => {
    expect((await loadWith("dcl_wallet_verified=not-a-wallet")).analyticsUserId).toBeNull();
  });

  it("should name no user on a shared-cacheable response", async () => {
    expect((await loadWith(`dcl_wallet_verified=${A}`, true)).analyticsUserId).toBeNull();
  });
});

describe("when a signed-in visitor's wallet reaches the root loader", () => {
  const WALLET = "0x00000000000000000000000000000000000000aa";
  let context: RouterContextProvider;
  const request = (path: string) =>
    new Request(`https://example.test${path}`, { headers: { cookie: `dcl_wallet=${WALLET}` } });

  beforeEach(() => { context = new RouterContextProvider(); });

  it("should hand a private page the wallet", async () => {
    const response = await loader({ request: request("/events"), params: {}, context } as never);
    expect(response.wallet).toBe(WALLET);
  });

  describe("and the response is shared-cacheable", () => {
    let response: Awaited<ReturnType<typeof loader>>;

    beforeEach(async () => {
      markPublicResponse(context);
      response = await loader({ request: request("/docs/"), params: {}, context } as never);
    });

    it("should omit the wallet", () => {
      expect(response.wallet).toBeNull();
    });

    it("should carry the address nowhere in the data", () => {
      expect(JSON.stringify(response)).not.toContain(WALLET);
    });
  });
});
