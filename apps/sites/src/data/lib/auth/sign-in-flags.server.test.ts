import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { resetSignInLayoutVariant, signInLayoutVariant } from "./sign-in-flags.server";

const fetchMock = vi.fn();

function answer(body: unknown, status = 200) {
  fetchMock.mockResolvedValueOnce(new Response(JSON.stringify(body), { status }));
}

beforeEach(() => {
  vi.stubGlobal("fetch", fetchMock);
  vi.stubEnv("FEATURE_FLAGS_ORIGIN", "");
  resetSignInLayoutVariant();
});
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.unstubAllEnvs();
  fetchMock.mockReset();
});

describe("the sign-in layout rollout", () => {
  it("reads decentraland.org's variant, once a minute whatever the request", async () => {
    answer({ variants: { "dapps-sign-in-primary-option": { name: "two-options", enabled: true } } });
    expect(await signInLayoutVariant()).toBe("two-options");
    expect(await signInLayoutVariant()).toBe("two-options");
    expect(fetchMock).toHaveBeenCalledOnce();
    const [url, init] = fetchMock.mock.calls[0]!;
    expect(url).toBe("https://feature-flags.decentraland.org/dapps.json");
    expect((init as RequestInit).headers).toEqual({ Referer: "https://decentraland.org/" });
  });

  it("asks as the configured site on another domain", async () => {
    vi.stubEnv("FEATURE_FLAGS_ORIGIN", "https://decentraland.zone/");
    answer({ variants: {} });
    await signInLayoutVariant();
    expect((fetchMock.mock.calls[0]![1] as RequestInit).headers).toEqual({ Referer: "https://decentraland.zone/" });
  });

  it("is the full layout when the variant is off or absent, or the service fails", async () => {
    for (const reply of [
      () => answer({ variants: { "dapps-sign-in-primary-option": { name: "two-options", enabled: false } } }),
      () => answer({ flags: {} }),
      () => answer({}, 503),
      () => fetchMock.mockRejectedValueOnce(new TypeError("fetch failed")),
    ]) {
      resetSignInLayoutVariant();
      reply();
      expect(await signInLayoutVariant()).toBeNull();
    }
  });

  it("keeps a failure for its minute, then asks again", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    answer({}, 503);
    expect(await signInLayoutVariant()).toBeNull();
    expect(await signInLayoutVariant()).toBeNull();
    expect(fetchMock).toHaveBeenCalledOnce();
    vi.setSystemTime(Date.now() + 61_000);
    answer({ variants: { "dapps-sign-in-primary-option": { name: "one-option", enabled: true } } });
    expect(await signInLayoutVariant()).toBe("one-option");
  });
});
