import { describe, expect, it } from "vitest";

import { isLegacyTabSegment, isOwnProfile, profileHref, profileTabs, readProfileTab } from "./tabs";

const A = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";

const tabIds = (own: boolean, referral?: boolean) => profileTabs(own, { referral }).map((tab) => tab.id);

describe("profileTabs", () => {
  describe("when the profile is another member's", () => {
    it("should list the tabs in the source order", () => {
      expect(profileTabs(false).map((tab) => tab.id)).toEqual(["overview", "creations", "communities", "places", "photos"]);
    });

    describe("and the referral flag is on", () => {
      it("should not show Referral Rewards", () => {
        expect(tabIds(false, true)).not.toContain("referral-rewards");
      });
    });
  });

  describe("when the profile is the member's own", () => {
    describe("and the referral flag is on", () => {
      it("should list the tabs in the source order, ending with Referral Rewards", () => {
        expect(profileTabs(true, { referral: true }).map((tab) => tab.id)).toEqual([
          "overview",
          "assets",
          "communities",
          "places",
          "photos",
          "referral-rewards",
        ]);
      });
    });

    describe("and the referral flag is not set", () => {
      it("should not show Referral Rewards", () => {
        expect(tabIds(true)).not.toContain("referral-rewards");
      });
    });

    describe("and the referral flag is off", () => {
      it("should not show Referral Rewards", () => {
        expect(tabIds(true, false)).not.toContain("referral-rewards");
      });
    });
  });
});

describe("readProfileTab", () => {
  describe("when the segment is referral-rewards", () => {
    describe("and the referral flag is not set", () => {
      it("should refuse it", () => {
        expect(readProfileTab("referral-rewards", true)).toBeNull();
      });
    });

    describe("and the referral flag is on for the member's own profile", () => {
      it("should read it as Referral Rewards", () => {
        expect(readProfileTab("referral-rewards", true, { referral: true })).toBe("referral-rewards");
      });
    });

    describe("and the referral flag is on for another member's profile", () => {
      it("should refuse it", () => {
        expect(readProfileTab("referral-rewards", false, { referral: true })).toBeNull();
      });
    });
  });

  describe("when the segment is the profile site's old `referral` tab id", () => {
    describe("and the referral flag is on for the member's own profile", () => {
      it("should read it as Referral Rewards", () => {
        expect(readProfileTab("referral", true, { referral: true })).toBe("referral-rewards");
      });

      it("should read it in any case", () => {
        expect(readProfileTab("Referral", true, { referral: true })).toBe("referral-rewards");
      });
    });

    describe("and the referral flag is off", () => {
      it("should refuse it", () => {
        expect(readProfileTab("referral", true, { referral: false })).toBeNull();
      });
    });

    describe("and the profile is another member's", () => {
      it("should refuse it", () => {
        expect(readProfileTab("referral", false, { referral: true })).toBeNull();
      });
    });
  });

  describe("when the segment is missing", () => {
    it("should read it as Overview", () => {
      expect(readProfileTab(undefined, false)).toBe("overview");
    });
  });

  describe("when the segment is empty", () => {
    it("should read it as Overview", () => {
      expect(readProfileTab("  ", true)).toBe("overview");
    });
  });

  describe("when the segment is in mixed case", () => {
    it("should read it lowercased", () => {
      expect(readProfileTab("Places", false)).toBe("places");
    });
  });

  describe("when the segment names a tab this profile does not show", () => {
    describe("and it is assets on another member's profile", () => {
      it("should refuse it", () => {
        expect(readProfileTab("assets", false)).toBeNull();
      });
    });

    describe("and it is creations on the member's own profile", () => {
      it("should refuse it", () => {
        expect(readProfileTab("creations", true)).toBeNull();
      });
    });

    describe("and it is not a tab at all", () => {
      it("should refuse it", () => {
        expect(readProfileTab("anything", true)).toBeNull();
      });
    });
  });
});

describe("isLegacyTabSegment", () => {
  describe("when the segment is the old `referral` id with spaces and capitals", () => {
    it("should be legacy", () => {
      expect(isLegacyTabSegment(" Referral ")).toBe(true);
    });
  });

  describe("when the segment is the current referral-rewards id", () => {
    it("should not be legacy", () => {
      expect(isLegacyTabSegment("referral-rewards")).toBe(false);
    });
  });

  describe("when the segment is missing", () => {
    it("should not be legacy", () => {
      expect(isLegacyTabSegment(undefined)).toBe(false);
    });
  });
});

describe("profileHref", () => {
  describe("when no tab is given", () => {
    it("should put Overview at the root with the address lowercased", () => {
      expect(profileHref(A.toUpperCase().replace("0X", "0x"))).toBe(`/profile/${A}`);
    });
  });

  describe("when a tab is given", () => {
    it("should put the tab in the path", () => {
      expect(profileHref(A, "photos")).toBe(`/profile/${A}/photos`);
    });
  });
});

describe("isOwnProfile", () => {
  describe("when the viewer is the same wallet in another case", () => {
    it("should call the profile the member's own", () => {
      expect(isOwnProfile(A, A.toUpperCase().replace("0X", "0x"))).toBe(true);
    });
  });

  describe("when the viewer is another wallet", () => {
    it("should not call the profile the member's own", () => {
      expect(isOwnProfile(A, "0x1111111111111111111111111111111111111111")).toBe(false);
    });
  });

  describe("when there is no viewer", () => {
    it("should not call the profile the member's own", () => {
      expect(isOwnProfile(A, null)).toBe(false);
    });
  });

  describe("when both addresses are empty", () => {
    it("should not call the profile the member's own", () => {
      expect(isOwnProfile("", "")).toBe(false);
    });
  });
});
