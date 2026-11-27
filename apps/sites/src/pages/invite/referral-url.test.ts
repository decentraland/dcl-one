import { describe, expect, it } from "vitest";

import { referralUrl } from "./referral-url";

describe("when building an invite's sign-in link", () => {
  describe("and the invite names a wallet", () => {
    it("should put the referrer on the sign-in and on its redirect to the download", () => {
      const url = new URL(referralUrl("0xabc"), "https://example.org");
      expect(url.pathname).toBe("/auth/login");
      expect(url.searchParams.get("referrer")).toBe("0xabc");
      expect(url.searchParams.get("redirectTo")).toBe("/download?referrer=0xabc");
    });
  });

  describe("and the invite names no wallet", () => {
    it("should send the visit to the download after the sign-in, without a referrer", () => {
      const url = new URL(referralUrl(""), "https://example.org");
      expect(url.pathname).toBe("/auth/login");
      expect(url.searchParams.has("referrer")).toBe(false);
      expect(url.searchParams.get("redirectTo")).toBe("/download");
    });
  });
});
