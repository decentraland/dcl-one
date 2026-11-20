// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { createMemoryRouter, RouterProvider, useParams } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});
const net = vi.hoisted(() => ({
  fetchProfile: vi.fn(),
  fetchProfileFlags: vi.fn(),
  fetchReferralProgress: vi.fn(),
}));
const auth = vi.hoisted(() => ({ state: { identity: null as unknown, isConnected: false, address: null as string | null } }));
vi.mock("@/data/lib/catalyst/social/profile", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/social/profile")>()),
  fetchProfile: net.fetchProfile,
}));
vi.mock("@/data/lib/profile/flags", async (original) => ({
  ...(await original<typeof import("@/data/lib/profile/flags")>()),
  fetchProfileFlags: net.fetchProfileFlags,
}));
vi.mock("@/data/lib/catalyst/social/referrals", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/social/referrals")>()),
  fetchReferralProgress: net.fetchReferralProgress,
}));
vi.mock("@/data/lib/catalyst/social/badges", () => ({ fetchUserBadges: () => Promise.resolve([]) }));
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));
vi.mock("@/data/lib/auth/index", () => ({ useAuth: () => auth.state }));

import { PROFILE_FLAGS_RETRIES } from "./hooks";
import { ProfilePage } from "./ProfilePage";

const A = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const B = "0x1111111111111111111111111111111111111111";
const identity = { signer: A, ephemeral: { address: "0xeph", privateKey: "0xdeadbeef" }, expiration: "2999-01-01T00:00:00.000Z", authChain: [] };

function Page() {
  const params = useParams();
  return <ProfilePage address={params.address ?? ""} tabSegment={params.tab} ownOnServer={undefined} sid="sid" />;
}

/** A client navigation: no server reading of the cookie, and nothing in the cache. */
function open(path: string) {
  // The flags read asks for its own retries; they follow one another at once here.
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, retryDelay: 0 } } });
  const router = createMemoryRouter(
    [
      { path: "/profile/:address", Component: Page },
      { path: "/profile/:address/:tab", Component: Page },
    ],
    { initialEntries: [path] },
  );
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return router;
}

beforeEach(() => {
  auth.state = { identity, isConnected: true, address: A };
  net.fetchProfile.mockResolvedValue({ name: "Alice", hasClaimedName: false, description: "", avatar: { snapshots: { face256: "" } } });
  net.fetchProfileFlags.mockResolvedValue({ referral: true });
  net.fetchReferralProgress.mockResolvedValue({ accepted: 2, viewed: 2, rewardImages: {} });
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("the profile page's Referral Rewards tab", () => {
  it("shows the tab on the member's own profile once the flag is read on", async () => {
    open(`/profile/${A}`);

    expect(await screen.findByRole("tab", { name: "Referral Rewards" })).toBeTruthy();
  });

  it("leaves the tab out while the flag is off", async () => {
    net.fetchProfileFlags.mockResolvedValue({ referral: false });

    open(`/profile/${A}`);

    await screen.findByRole("tab", { name: "My Photos" });
    await waitFor(() => expect(net.fetchProfileFlags).toHaveBeenCalled());
    expect(screen.queryByRole("tab", { name: "Referral Rewards" })).toBeNull();
  });

  it("opens the tab from a direct navigation, showing the member's progress", async () => {
    const router = open(`/profile/${A}/referral-rewards`);

    expect(await screen.findByText("2 Friends Joined")).toBeTruthy();
    expect(router.state.location.pathname).toBe(`/profile/${A}/referral-rewards`);
    expect(screen.getByRole("tab", { name: "Referral Rewards" }).getAttribute("aria-selected")).toBe("true");
  });

  it("goes back to Overview when the flag turns out off, without reading the progress", async () => {
    net.fetchProfileFlags.mockResolvedValue({ referral: false });

    const router = open(`/profile/${A}/referral-rewards`);

    await waitFor(() => expect(router.state.location.pathname).toBe(`/profile/${A}`));
    expect(screen.queryByRole("tab", { name: "Referral Rewards" })).toBeNull();
  });

  it("keeps the tab when the flags cannot be read, after reading them again", async () => {
    net.fetchProfileFlags.mockRejectedValue(new Error("502"));

    const router = open(`/profile/${A}/referral-rewards`);

    await waitFor(() => expect(net.fetchProfileFlags).toHaveBeenCalledTimes(1 + PROFILE_FLAGS_RETRIES));
    expect(await screen.findByText("2 Friends Joined")).toBeTruthy();
    expect(router.state.location.pathname).toBe(`/profile/${A}/referral-rewards`);
    expect(screen.getByRole("tab", { name: "Referral Rewards" }).getAttribute("aria-selected")).toBe("true");
  });

  it("keeps the tab through a failed flags read, then follows the flag the next read brings", async () => {
    net.fetchProfileFlags.mockRejectedValueOnce(new Error("502"));

    const router = open(`/profile/${A}/referral-rewards`);

    expect(await screen.findByText("2 Friends Joined")).toBeTruthy();
    await waitFor(() => expect(net.fetchProfileFlags).toHaveBeenCalledTimes(2));
    expect(router.state.location.pathname).toBe(`/profile/${A}/referral-rewards`);
  });

  it("goes back to Overview only once a read after a failed one says the flag is off", async () => {
    let fail: (error: Error) => void = () => {};
    net.fetchProfileFlags
      .mockImplementationOnce(() => new Promise((_resolve, reject) => (fail = reject)))
      .mockResolvedValue({ referral: false });

    const router = open(`/profile/${A}/referral-rewards`);
    await screen.findByRole("tab", { name: "Referral Rewards" });
    expect(router.state.location.pathname).toBe(`/profile/${A}/referral-rewards`);
    fail(new Error("502"));

    await waitFor(() => expect(router.state.location.pathname).toBe(`/profile/${A}`));
    expect(net.fetchProfileFlags).toHaveBeenCalledTimes(2);
  });

  it("moves the profile site's old `referral` tab id to the current path", async () => {
    const router = open(`/profile/${A}/referral`);

    await waitFor(() => expect(router.state.location.pathname).toBe(`/profile/${A}/referral-rewards`));
    expect(await screen.findByText("2 Friends Joined")).toBeTruthy();
  });

  it("reads no flags and shows no such tab on another member's profile", async () => {
    const router = open(`/profile/${B}/referral-rewards`);

    await waitFor(() => expect(router.state.location.pathname).toBe(`/profile/${B}`));
    await screen.findByRole("tab", { name: "Photos" });
    expect(net.fetchProfileFlags).not.toHaveBeenCalled();
    expect(net.fetchReferralProgress).not.toHaveBeenCalled();
  });
});
