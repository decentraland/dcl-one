import { describe, expect, it } from "vitest";

import { isOwnProfile, profileHref, profileTabs, readProfileTab } from "./tabs";

const A = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";

describe("profile tabs", () => {
  it("list another member's tabs and the member's own in the source order", () => {
    expect(profileTabs(false).map((tab) => tab.id)).toEqual(["overview", "creations", "communities", "places", "photos"]);
    expect(profileTabs(true).map((tab) => tab.id)).toEqual([
      "overview",
      "assets",
      "communities",
      "places",
      "photos",
      "referral-rewards",
    ]);
  });

  it("read a missing or empty segment as Overview", () => {
    expect(readProfileTab(undefined, false)).toBe("overview");
    expect(readProfileTab("  ", true)).toBe("overview");
  });

  it("read a segment in any case, and refuse one this profile does not show", () => {
    expect(readProfileTab("Places", false)).toBe("places");
    expect(readProfileTab("assets", false)).toBeNull();
    expect(readProfileTab("creations", true)).toBeNull();
    expect(readProfileTab("anything", true)).toBeNull();
  });

  it("put Overview at the root and every other tab in the path, lowercased", () => {
    expect(profileHref(A.toUpperCase().replace("0X", "0x"))).toBe(`/profile/${A}`);
    expect(profileHref(A, "photos")).toBe(`/profile/${A}/photos`);
  });

  it("call a profile the member's own only for the same wallet, in any case", () => {
    expect(isOwnProfile(A, A.toUpperCase().replace("0X", "0x"))).toBe(true);
    expect(isOwnProfile(A, "0x1111111111111111111111111111111111111111")).toBe(false);
    expect(isOwnProfile(A, null)).toBe(false);
    expect(isOwnProfile("", "")).toBe(false);
  });
});
