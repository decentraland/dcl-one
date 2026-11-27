import { afterEach, describe, expect, it, vi } from "vitest";

const resolveReferrerAddress = vi.fn();
const fetchReferrerProfile = vi.fn();
vi.mock("./referral", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./referral")>()),
  resolveReferrerAddress: (...args: unknown[]) => resolveReferrerAddress(...args),
  fetchReferrerProfile: (...args: unknown[]) => fetchReferrerProfile(...args),
}));

import { referrerFromAddress } from "./referral";
import { loadInviteReferral } from "./referral.server";

const ADDRESS = "0xabcdef0123456789abcdef0123456789abcdef01";

afterEach(() => {
  resolveReferrerAddress.mockReset();
  fetchReferrerProfile.mockReset();
});

describe("when loading an invite's referrer", () => {
  describe("and the param names a wallet whose profile resolves", () => {
    it("should return the profile, looked up with the trimmed param and the request's signal", async () => {
      const signal = new AbortController().signal;
      const profile = { ...referrerFromAddress(ADDRESS), name: "Jane", resolved: true };
      resolveReferrerAddress.mockResolvedValue(ADDRESS);
      fetchReferrerProfile.mockResolvedValue(profile);
      expect(await loadInviteReferral("  Jane ", signal)).toEqual({ referrer: profile });
      expect(resolveReferrerAddress).toHaveBeenCalledWith("Jane", { signal });
      expect(fetchReferrerProfile).toHaveBeenCalledWith(ADDRESS, { signal });
    });
  });

  describe("and the profile lookup fails", () => {
    it("should keep the wallet without a name", async () => {
      resolveReferrerAddress.mockResolvedValue(ADDRESS);
      fetchReferrerProfile.mockRejectedValue(new Error("down"));
      expect(await loadInviteReferral(ADDRESS)).toEqual({ referrer: referrerFromAddress(ADDRESS) });
    });
  });

  describe("and the param names no wallet", () => {
    it("should return a referrer without an address and read no profile", async () => {
      resolveReferrerAddress.mockResolvedValue(null);
      expect(await loadInviteReferral("nobody")).toEqual({ referrer: referrerFromAddress(null) });
      expect(fetchReferrerProfile).not.toHaveBeenCalled();
    });
  });

  describe("and resolving the param fails", () => {
    it("should reject, so the route keeps the page without a referrer", async () => {
      resolveReferrerAddress.mockRejectedValue(new Error("node down"));
      await expect(loadInviteReferral("Jane")).rejects.toThrow("node down");
    });
  });
});
