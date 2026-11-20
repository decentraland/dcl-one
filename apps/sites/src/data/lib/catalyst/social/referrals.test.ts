import { afterEach, describe, expect, it, vi } from "vitest";

const client = vi.hoisted(() => ({ signedGetJSON: vi.fn() }));
vi.mock("../client", async (original) => ({
  ...(await original<typeof import("../client")>()),
  signedGetJSON: client.signedGetJSON,
}));

import type { AuthIdentity } from "../../auth/types";

import {
  fetchReferralProgress,
  nextReferralTier,
  REFERRAL_TIERS,
  referralInviteUrl,
  referralJourneyProgress,
} from "./referrals";

const identity: AuthIdentity = {
  signer: "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295",
  ephemeral: { address: `0x${"c".repeat(40)}`, privateKey: `0x${"12".repeat(32)}` },
  expiration: "2999-01-01T00:00:00.000Z",
  authChain: [],
};

afterEach(() => vi.resetAllMocks());

describe("fetchReferralProgress", () => {
  it("signs the read for the member and maps the node's answer", async () => {
    client.signedGetJSON.mockResolvedValue({
      invitedUsersAccepted: 12,
      invitedUsersAcceptedViewed: 9,
      rewardImages: [
        { tier: 5, url: "https://rewards.example.org/jeans.png" },
        { tier: 10, url: "http://rewards.example.org/insecure.png" },
      ],
    });
    const signal = new AbortController().signal;

    const progress = await fetchReferralProgress(identity, { signal });

    expect(client.signedGetJSON).toHaveBeenCalledWith("/v1/referral-progress", { identity, signal });
    // A picture that is not https is left out, so the tier keeps its own.
    expect(progress).toEqual({
      accepted: 12,
      viewed: 9,
      rewardImages: { 5: "https://rewards.example.org/jeans.png" },
    });
  });

  it("refuses an answer without the counts", async () => {
    client.signedGetJSON.mockResolvedValue({ rewardImages: [] });

    await expect(fetchReferralProgress(identity)).rejects.toThrow();
  });
});

describe("referral tiers", () => {
  it("name the next tier until the last one is reached", () => {
    expect(nextReferralTier(0)?.invites).toBe(5);
    expect(nextReferralTier(5)?.invites).toBe(10);
    expect(nextReferralTier(99)?.invites).toBe(100);
    expect(nextReferralTier(100)).toBeNull();
  });

  it("fill the track from empty to full, reaching each tier at the centre of its column", () => {
    const count = REFERRAL_TIERS.length;
    expect(referralJourneyProgress(0)).toBe(0);
    REFERRAL_TIERS.slice(0, -1).forEach((tier, index) => {
      expect(referralJourneyProgress(tier.invites)).toBeCloseTo((index + 0.5) / count);
    });
    // Halfway between the 10 and 20 invite tiers is halfway between their columns.
    expect(referralJourneyProgress(15)).toBeCloseTo(2 / count);
    expect(referralJourneyProgress(100)).toBe(1);
    expect(referralJourneyProgress(250)).toBe(1);
  });

  it("never move the track back as friends join", () => {
    let previous = 0;
    for (let accepted = 0; accepted <= 110; accepted += 1) {
      const now = referralJourneyProgress(accepted);
      expect(now).toBeGreaterThanOrEqual(previous);
      previous = now;
    }
  });
});

describe("referralInviteUrl", () => {
  it("puts the handle after /invite on the site's origin", () => {
    expect(referralInviteUrl("https://example.org/", "0xabc")).toBe("https://example.org/invite/0xabc");
    expect(referralInviteUrl("https://example.org", "A Name")).toBe("https://example.org/invite/A%20Name");
  });
});
