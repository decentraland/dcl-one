import { describe, expect, it } from "vitest";

import {
  collectCampaignParams,
  collectDeepLinkParams,
  downloadSuccessHref,
  parseAnonUserId,
  parseReferrer,
  resolveMacArch,
  resolvePlace,
  resolvePlatform,
} from "./params";

const q = (query: string) => new URLSearchParams(query);

describe("resolvePlatform", () => {
  it.each([
    ["os=windows", { os: "windows", arch: "amd64" }],
    ["os=Windows&arch=arm64", { os: "windows", arch: "arm64" }],
    ["os=macOS", { os: "macos", arch: "arm64" }],
    ["os=macos&arch=amd64", { os: "macos", arch: "amd64" }],
    ["os=linux", { os: "macos", arch: "arm64" }],
    ["", { os: "macos", arch: "arm64" }],
    ["os=windows&arch=x64", { os: "windows", arch: "amd64" }],
    ["os=macos&arch=ia32", { os: "macos", arch: "arm64" }],
  ])("%s resolves as the source rule", (query, expected) => {
    expect(resolvePlatform(q(query))).toEqual(expected);
  });
});

describe("resolveMacArch", () => {
  it("allowlists the analytics hint and drops anything else", () => {
    expect(resolveMacArch(q("mac_arch=intel"))).toBe("intel");
    expect(resolveMacArch(q("mac_arch=<script>"))).toBeUndefined();
  });
});

describe("resolvePlace", () => {
  it("keeps known places and maps the rest to unknown", () => {
    expect(resolvePlace("landing-hero")).toBe("landing-hero");
    expect(resolvePlace("nope")).toBe("unknown");
    expect(resolvePlace(null)).toBe("unknown");
  });
});

describe("collectCampaignParams", () => {
  it("keeps utm_* only, normalized and capped", () => {
    expect(
      collectCampaignParams(
        q("utm_source=Partner%20Name!&utm_campaign=Q4&other=x&utm_term="),
      ),
    ).toEqual({ utm_source: "partner_name", utm_campaign: "q4" });
    expect(
      collectCampaignParams(q(`utm_content=${"a".repeat(300)}`)).utm_content,
    ).toHaveLength(256);
  });
});

describe("collectDeepLinkParams", () => {
  it("re-emits integer positions and drops defaults", () => {
    expect(
      collectDeepLinkParams(q("position=10.20&realm=foo.dcl.eth")),
    ).toEqual({
      position: "10,20",
      realm: "foo.dcl.eth",
    });
    expect(collectDeepLinkParams(q("position=0,0&realm=main"))).toEqual({});
  });

  it("drops values outside their character sets", () => {
    expect(
      collectDeepLinkParams(q("position=1,2,3&realm=<x>&communityId=a/b")),
    ).toEqual({});
  });

  it("keeps a valid community id", () => {
    expect(collectDeepLinkParams(q("communityId=7f1c-22aa"))).toEqual({
      communityId: "7f1c-22aa",
    });
  });
});

describe("parseReferrer and parseAnonUserId", () => {
  it("accept only wallet addresses and UUIDs", () => {
    expect(parseReferrer("0xABCDEF0123456789abcdef0123456789ABCDEF01")).toBe(
      "0xabcdef0123456789abcdef0123456789abcdef01",
    );
    expect(parseReferrer("0x123")).toBeUndefined();
    expect(parseAnonUserId("3f9c1e2a-1b2c-4d3e-8f90-0a1b2c3d4e5f")).toBe(
      "3f9c1e2a-1b2c-4d3e-8f90-0a1b2c3d4e5f",
    );
    expect(parseAnonUserId("not-a-uuid")).toBeUndefined();
  });
});

describe("downloadSuccessHref", () => {
  it("builds the source URL shape without letting extras overwrite routing params", () => {
    const href = downloadSuccessHref("windows", "download-page", {
      arch: "amd64",
      anonUserId: "3f9c1e2a-1b2c-4d3e-8f90-0a1b2c3d4e5f",
      deepLink: { position: "1,2" },
      campaign: { utm_source: "x", os: "evil" } as Record<string, string>,
    });
    const params = new URL(href, "https://example.com").searchParams;
    expect(href.startsWith("/download_success?")).toBe(true);
    expect(params.get("os")).toBe("Windows");
    expect(params.get("place")).toBe("download-page");
    expect(params.get("arch")).toBe("amd64");
    expect(params.get("position")).toBe("1,2");
    expect(params.get("utm_source")).toBe("x");
  });
});
