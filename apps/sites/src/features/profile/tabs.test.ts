import { describe, expect, it } from "vitest";

import { isLegacyTabSegment, isOwnProfile, profileHref, profileTabs, readProfileTab } from "./tabs";

const A = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";

describe("profile tabs", () => {
  it("list another member's tabs and the member's own in the source order", () => {
    expect(profileTabs(false).map((tab) => tab.id)).toEqual(["overview", "creations", "communities", "places", "photos"]);
    expect(profileTabs(true, { referral: true }).map((tab) => tab.id)).toEqual([
      "overview",
      "assets",
      "communities",
      "places",
      "photos",
      "referral-rewards",
    ]);
  });

  it("show Referral Rewards only on the member's own profile, and only while its flag is on", () => {
    const ids = (own: boolean, referral?: boolean) => profileTabs(own, { referral }).map((tab) => tab.id);
    expect(ids(true)).not.toContain("referral-rewards");
    expect(ids(true, false)).not.toContain("referral-rewards");
    expect(ids(false, true)).not.toContain("referral-rewards");
    expect(readProfileTab("referral-rewards", true)).toBeNull();
    expect(readProfileTab("referral-rewards", true, { referral: true })).toBe("referral-rewards");
    expect(readProfileTab("referral-rewards", false, { referral: true })).toBeNull();
  });

  it("read the profile site's old `referral` tab id as Referral Rewards", () => {
    expect(readProfileTab("referral", true, { referral: true })).toBe("referral-rewards");
    expect(readProfileTab("Referral", true, { referral: true })).toBe("referral-rewards");
    expect(readProfileTab("referral", true, { referral: false })).toBeNull();
    expect(readProfileTab("referral", false, { referral: true })).toBeNull();
    expect(isLegacyTabSegment(" Referral ")).toBe(true);
    expect(isLegacyTabSegment("referral-rewards")).toBe(false);
    expect(isLegacyTabSegment(undefined)).toBe(false);
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
