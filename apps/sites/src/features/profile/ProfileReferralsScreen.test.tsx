// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
const net = vi.hoisted(() => ({ fetchReferralProgress: vi.fn() }));
vi.mock("@/data/lib/catalyst/social/referrals", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/social/referrals")>()),
  fetchReferralProgress: net.fetchReferralProgress,
}));
const auth = vi.hoisted(() => ({ state: { identity: null as unknown, isConnected: false, address: null as string | null } }));
vi.mock("@/data/lib/auth/index", () => ({ useAuth: () => auth.state }));

import { emptyProfile, type ProfileVM } from "@/data/lib/catalyst/social/profile";
import type { ReferralProgress } from "@/data/lib/catalyst/social/referrals";
import { profileKeys } from "@/data/lib/profile/queries";

import { ProfileReferralsScreen, REFERRAL_TERMS_HREF } from "./ProfileReferralsScreen";

const A = "0x09bc69a986e1b37fb01ef36124f4c2e14fa0ab6b";
const B = "0x1111111111111111111111111111111111111111";
const tabs = [
  { id: "overview", label: "Overview", href: `/profile/${A}` },
  { id: "referral-rewards", label: "Referral Rewards", href: `/profile/${A}/referral-rewards` },
];
const identityOf = (signer: string) => ({
  signer,
  ephemeral: { address: "0xeph", privateKey: "0xdeadbeef" },
  expiration: "2999-01-01T00:00:00.000Z",
  authChain: [],
});
const progress = (over: Partial<ReferralProgress> = {}): ReferralProgress => ({
  accepted: 12,
  viewed: 12,
  rewardImages: {},
  ...over,
});

function show(
  profile: Partial<ProfileVM> = {},
  own = true,
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } }),
) {
  const vm: ProfileVM = { ...emptyProfile(A), name: "Billyjuice", nameColor: "#ff8362", ...profile };
  const router = createMemoryRouter(
    [{ path: "*", element: <ProfileReferralsScreen profile={vm} own={own} tabs={tabs} onTab={vi.fn()} /> }],
    { initialEntries: [`/profile/${A}/referral-rewards`] },
  );
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return { client };
}

const tier = (invites: number) =>
  screen
    .getAllByRole("listitem")
    .find((item) => within(item).queryByText(String(invites), { selector: "p" }) !== null) as HTMLElement;

beforeEach(() => {
  auth.state = { identity: identityOf(A), isConnected: true, address: A };
  net.fetchReferralProgress.mockResolvedValue(progress());
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  vi.unstubAllGlobals();
});

describe("Referral Rewards tab", () => {
  it("is the selected tab's panel", async () => {
    show();

    expect(screen.getByRole("tab", { name: "Referral Rewards" }).getAttribute("aria-selected")).toBe("true");
    expect(await screen.findByRole("heading", { name: "Decentraland is Better With Friends" })).toBeTruthy();
  });

  it("asks a signed-out visitor to sign in, without a request", () => {
    auth.state = { identity: null, isConnected: false, address: null };

    show({}, false);

    expect(screen.getByText("Sign in to invite friends and unlock rewards.")).toBeTruthy();
    expect(net.fetchReferralProgress).not.toHaveBeenCalled();
  });

  it("never reads the progress with another wallet's identity", () => {
    auth.state = { identity: identityOf(B), isConnected: true, address: B };

    show({}, false);

    expect(screen.getByText("Sign in to invite friends and unlock rewards.")).toBeTruthy();
    expect(net.fetchReferralProgress).not.toHaveBeenCalled();
  });

  it("shows the owner's page as loading, not the sign-in text, until the browser has their identity", () => {
    auth.state = { identity: null, isConnected: false, address: null };

    show();

    expect(screen.getByText("Loading referral rewards")).toBeTruthy();
    expect(screen.queryByText("Sign in to invite friends and unlock rewards.")).toBeNull();
    expect(net.fetchReferralProgress).not.toHaveBeenCalled();
  });

  it("announces loading, then shows the friends who joined and what the next reward needs", async () => {
    let answer: (value: ReferralProgress) => void = () => {};
    net.fetchReferralProgress.mockReturnValue(new Promise<ReferralProgress>((resolve) => (answer = resolve)));

    show();
    expect(screen.getByText("Loading referral rewards")).toBeTruthy();

    answer(progress({ accepted: 12 }));
    expect(await screen.findByText("12 Friends Joined")).toBeTruthy();
    expect(screen.getByText("8 more friends for your next reward")).toBeTruthy();
    const track = screen.getByRole("progressbar", { name: "Reward journey" });
    expect(track.getAttribute("aria-valuenow")).toBe("12");
    expect(track.getAttribute("aria-valuetext")).toBe("12 Friends Joined. 8 more friends for your next reward.");
  });

  it("keeps the member's progress under their private key", async () => {
    const { client } = show();

    await screen.findByText("12 Friends Joined");

    expect(client.getQueryData(profileKeys.myReferralProgress(A))).toEqual(progress());
    expect(profileKeys.myReferralProgress(A).slice(0, 2)).toEqual(["private", A]);
  });

  it("lists every tier with its rarity, locked until its invites are reached", async () => {
    net.fetchReferralProgress.mockResolvedValue(progress({ accepted: 12 }));

    show();
    await screen.findByText("12 Friends Joined");

    expect(within(tier(5)).getByText("Unlocked")).toBeTruthy();
    expect(within(tier(10)).getByText("Unlocked")).toBeTruthy();
    expect(within(tier(20)).getByText("Locked")).toBeTruthy();
    expect(within(tier(20)).getByText("Legendary")).toBeTruthy();
    expect(within(tier(100)).getByText("IRL Swag Pack")).toBeTruthy();
    expect(within(tier(5)).getByRole("img", { name: /RainbowWave Jeans/ })).toBeTruthy();
  });

  it("shows a granted reward's own picture and marks the tiers reached since the last visit", async () => {
    net.fetchReferralProgress.mockResolvedValue(
      progress({ accepted: 12, viewed: 6, rewardImages: { 5: "https://rewards.example.org/jeans.png" } }),
    );

    show();
    await screen.findByText("12 Friends Joined");

    expect(within(tier(5)).getByRole("img").getAttribute("src")).toBe("https://rewards.example.org/jeans.png");
    expect(within(tier(5)).queryByText("New")).toBeNull();
    expect(within(tier(10)).getByText("New")).toBeTruthy();
  });

  it("marks those tiers once: not again on a return to the tab while the answer is still cached", async () => {
    net.fetchReferralProgress.mockResolvedValue(progress({ accepted: 12, viewed: 6 }));
    const { client } = show();
    await screen.findByText("12 Friends Joined");

    // The cached answer now holds what the node recorded on the read; this visit keeps its marks.
    await waitFor(() =>
      expect(client.getQueryData(profileKeys.myReferralProgress(A))).toMatchObject({ accepted: 12, viewed: 12 }),
    );
    expect(within(tier(10)).getByText("New")).toBeTruthy();

    cleanup();
    show({}, true, client);
    await screen.findByText("12 Friends Joined");

    expect(screen.queryByText("New")).toBeNull();
    expect(net.fetchReferralProgress).toHaveBeenCalledTimes(1);
  });

  it("says every reward is unlocked past the last tier", async () => {
    net.fetchReferralProgress.mockResolvedValue(progress({ accepted: 140 }));

    show();

    expect(await screen.findByText("Every reward unlocked")).toBeTruthy();
    expect(screen.getByRole("progressbar").getAttribute("aria-valuenow")).toBe("100");
  });

  it("copies the invite link by wallet and says so", async () => {
    const user = userEvent.setup();
    const writeText = vi.fn(async () => {});
    vi.stubGlobal("navigator", { ...navigator, clipboard: { writeText } });
    show();
    const link = `${window.location.origin}/invite/${A}`;

    expect(((await screen.findByLabelText("Your invite link")) as HTMLInputElement).value).toBe(link);
    await user.click(screen.getByRole("button", { name: "Copy link" }));

    expect(writeText).toHaveBeenCalledWith(link);
    expect(await screen.findByText("Link copied successfully")).toBeTruthy();
  });

  it("says when the link could not be copied", async () => {
    const user = userEvent.setup();
    vi.stubGlobal("navigator", { ...navigator, clipboard: { writeText: vi.fn(async () => Promise.reject(new Error("denied"))) } });
    show();

    await user.click(await screen.findByRole("button", { name: "Copy link" }));

    expect(await screen.findByText("Couldn't copy the link")).toBeTruthy();
  });

  it("uses the claimed NAME in the link, and shares it on X in a new tab", async () => {
    show({ hasClaimedName: true, name: "Billyjuice" });

    const field = (await screen.findByLabelText("Your invite link")) as HTMLInputElement;
    expect(field.value).toBe(`${window.location.origin}/invite/Billyjuice`);
    const share = screen.getByRole("link", { name: "Share on X" });
    expect(share.getAttribute("target")).toBe("_blank");
    expect(share.getAttribute("href")).toContain(encodeURIComponent(`${window.location.origin}/invite/Billyjuice`));
  });

  it("links the program's terms and opens its three steps on request", async () => {
    const user = userEvent.setup();
    show();

    const toggle = await screen.findByRole("button", { name: "How it works" });
    expect(screen.getByRole("link", { name: "Terms & Conditions" }).getAttribute("href")).toBe(REFERRAL_TERMS_HREF);
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    const steps = screen.getByText(/jump in on 3 separate days/).closest("ol") as HTMLElement;
    expect(steps.hidden).toBe(true);

    await user.click(toggle);

    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect(steps.hidden).toBe(false);
    expect(within(steps).getAllByRole("listitem")).toHaveLength(3);
  });

  it("says when the progress cannot be read, and reads it again on Try again", async () => {
    const user = userEvent.setup();
    net.fetchReferralProgress.mockRejectedValueOnce(new Error("502"));
    show();

    expect(await screen.findByText("Referral rewards are unavailable right now.")).toBeTruthy();
    net.fetchReferralProgress.mockResolvedValue(progress({ accepted: 3 }));
    await user.click(screen.getByRole("button", { name: "Try again" }));

    await waitFor(() => expect(screen.getByText("3 Friends Joined")).toBeTruthy());
  });
});
