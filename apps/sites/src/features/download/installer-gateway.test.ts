// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  chooseInstallerUrl,
  collectDeepLinkParams,
  resolveGatewayAnonUserId,
  resolveReferrer,
  withInstallerQuery,
} from "./installer-gateway";

const UUID = "0b7f4c2e-1d3a-4e5f-9a8b-7c6d5e4f3a2b";
const WALLET = "0xAbCdEf0123456789abcdef0123456789ABCDEF01";
const params = (query: string) => new URLSearchParams(query);

beforeEach(() => {
  sessionStorage.clear();
  localStorage.clear();
});
afterEach(() => {
  vi.restoreAllMocks();
});

describe("collectDeepLinkParams", () => {
  it("re-emits a position from its integers and keeps a valid realm", () => {
    expect(collectDeepLinkParams(params("position=10.20&realm=foo.dcl.eth"))).toEqual({ position: "10,20", realm: "foo.dcl.eth" });
  });

  it("drops the Explorer's defaults, a malformed position and a realm outside the allowlist", () => {
    expect(collectDeepLinkParams(params("position=0,0&realm=main"))).toEqual({});
    expect(collectDeepLinkParams(params("position=1,2,3&realm=a/b"))).toEqual({});
    expect(collectDeepLinkParams(params("position=x,y"))).toEqual({});
    expect(collectDeepLinkParams(params(""))).toEqual({});
  });
});

describe("resolveReferrer", () => {
  it("takes the URL's wallet, lowercased, over the stored one", () => {
    sessionStorage.setItem("dcl_referrer", "0x" + "1".repeat(40));
    expect(resolveReferrer(params(`referrer=${WALLET}`))).toBe(WALLET.toLowerCase());
  });

  it("clears the stored wallet and sends none when the URL's is invalid", () => {
    sessionStorage.setItem("dcl_referrer", "0x" + "1".repeat(40));
    expect(resolveReferrer(params("referrer=0x123"))).toBeNull();
    expect(sessionStorage.getItem("dcl_referrer")).toBeNull();
  });

  it("falls back to the stored wallet without a URL one, and to none without either", () => {
    expect(resolveReferrer(params(""))).toBeNull();
    sessionStorage.setItem("dcl_referrer", WALLET);
    expect(resolveReferrer(params(""))).toBe(WALLET.toLowerCase());
  });
});

describe("resolveGatewayAnonUserId", () => {
  it("passes a UUID through and mints nothing without a deep link or referrer", () => {
    expect(resolveGatewayAnonUserId(UUID, "custom", {}, null)).toBe(UUID);
    expect(resolveGatewayAnonUserId(undefined, "custom", {}, null)).toBeUndefined();
    expect(localStorage.getItem("dcl_gateway_anon_user_id")).toBeNull();
  });

  it("keeps one attribution UUID per non-UUID visitor id when the installer carries a deep link", () => {
    const first = resolveGatewayAnonUserId(undefined, "custom-a", { position: "1,2" }, null);
    expect(first).toMatch(/^[0-9a-f-]{36}$/);
    expect(resolveGatewayAnonUserId(undefined, "custom-a", { realm: "x" }, null)).toBe(first);
    expect(JSON.parse(localStorage.getItem("dcl_gateway_anon_user_id")!)).toEqual({ segmentId: "custom-a", attributionId: first });
    expect(resolveGatewayAnonUserId(undefined, "custom-b", {}, WALLET)).not.toBe(first);
  });

  it("still returns a stable id when storage is blocked", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    const id = resolveGatewayAnonUserId(undefined, "custom-c", { position: "1,2" }, null);
    expect(resolveGatewayAnonUserId(undefined, "custom-c", { position: "1,2" }, null)).toBe(id);
  });
});

describe("chooseInstallerUrl", () => {
  const base = {
    os: "Windows" as const,
    arch: "amd64" as const,
    fallback: "https://installers.test/Decentraland_installer.exe",
    anonUserId: UUID,
    signal: new AbortController().signal,
  };

  it("serves the plain installer without a gateway, never asking for a token", async () => {
    const identityId = vi.fn();
    expect(await chooseInstallerUrl({ ...base, gateway: null, identityId })).toBe(base.fallback);
    expect(identityId).not.toHaveBeenCalled();
  });

  it("serves the plain installer with a gateway but neither a token nor a campaign id", async () => {
    expect(await chooseInstallerUrl({ ...base, gateway: "https://g.test", identityId: async () => undefined, anonUserId: undefined })).toBe(
      base.fallback,
    );
  });
});

describe("withInstallerQuery", () => {
  it("appends only the values present, in order", () => {
    expect(withInstallerQuery("https://g.test/a.exe", { anon_user_id: UUID, position: undefined, referrer: null })).toBe(
      `https://g.test/a.exe?anon_user_id=${UUID}`,
    );
  });
});
