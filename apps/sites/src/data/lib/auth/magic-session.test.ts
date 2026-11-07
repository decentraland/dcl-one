// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const sdk = vi.hoisted(() => ({ key: "pk_example123", logout: vi.fn(async () => true), created: vi.fn() }));
vi.mock("./magic-config", () => ({ magicApiKey: () => sdk.key }));
vi.mock("magic-sdk", () => ({
  Magic: class {
    user = { logout: sdk.logout };
    constructor(key: string) {
      sdk.created(key);
    }
  },
}));
vi.mock("@magic-ext/oauth2", () => ({ OAuthExtension: class {} }));

import { endMagicSession, hasMagicSession, markMagicSession } from "./magic-session";

beforeEach(() => localStorage.clear());
afterEach(() => {
  vi.clearAllMocks();
  sdk.key = "pk_example123";
});

describe("ending Magic's session on sign-out", () => {
  it("logs out of Magic after a sign-in through it, once", async () => {
    markMagicSession();
    await endMagicSession();
    expect(sdk.created).toHaveBeenCalledWith("pk_example123");
    expect(sdk.logout).toHaveBeenCalledOnce();
    await endMagicSession();
    expect(sdk.logout).toHaveBeenCalledOnce();
  });

  it("never loads Magic for a reader who signed in with a wallet", async () => {
    await endMagicSession();
    expect(sdk.created).not.toHaveBeenCalled();
  });

  it("tries again on the next sign-out when Magic could not be reached", async () => {
    markMagicSession();
    sdk.logout.mockRejectedValueOnce(new Error("offline"));
    await expect(endMagicSession()).resolves.toBeUndefined();
    expect(localStorage.getItem("dcl:auth:magic-session")).toBe("1");
    await endMagicSession();
    expect(sdk.logout).toHaveBeenCalledTimes(2);
    expect(localStorage.getItem("dcl:auth:magic-session")).toBeNull();
  });
});

describe("the Magic marker as a reading", () => {
  it("says a Magic session is held from its mark until it is cleared", () => {
    expect(hasMagicSession()).toBe(false);
    markMagicSession();
    expect(hasMagicSession()).toBe(true);
    localStorage.clear();
    expect(hasMagicSession()).toBe(false);
  });
});
