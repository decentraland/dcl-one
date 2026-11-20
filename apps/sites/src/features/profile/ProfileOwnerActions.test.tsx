// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});

const net = vi.hoisted(() => ({
  count: vi.fn(),
  list: vi.fn(),
  summaries: vi.fn(),
  referralEnabled: true,
}));

vi.mock("@/data/lib/friends/friends", () => ({ fetchFriendsCount: net.count, fetchFriendsPage: net.list }));
vi.mock("@ui/data/hooks/useCommunityProfiles", () => ({ fetchCommunityProfiles: net.summaries }));
vi.mock("@/data/lib/auth/index", () => ({ useAuth: () => ({ identity }) }));
vi.mock("./hooks", () => ({ useProfileFlags: () => ({ data: { referral: net.referralEnabled } }) }));

import { emptyProfile } from "@/data/lib/catalyst/social/profile";
import { ProfileFriendsCount, ProfileInviteFriends } from "./ProfileOwnerActions";

const ME = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const FIRST = "0x1111111111111111111111111111111111111111";
const NEXT = "0x2222222222222222222222222222222222222222";
const identity = { signer: ME };
const profile = { ...emptyProfile(ME), name: "Alice", hasClaimedName: true };

function show(node: ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter([{ path: "*", element: node }]);
  render(<QueryClientProvider client={client}><RouterProvider router={router} /></QueryClientProvider>);
}

beforeEach(() => {
  net.referralEnabled = true;
  net.count.mockResolvedValue(21);
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  vi.unstubAllGlobals();
});

describe("Profile owner's Friends dialog", () => {
  it("reads the signed list only after opening and retries a failed read", async () => {
    const user = userEvent.setup();
    net.list.mockRejectedValueOnce(new Error("offline")).mockResolvedValueOnce({ addresses: [], total: 0 });
    show(<ProfileFriendsCount address={ME} />);

    const trigger = await screen.findByRole("button", { name: "21 Friends" });
    expect(net.list).not.toHaveBeenCalled();
    await user.click(trigger);
    expect(await screen.findByRole("dialog", { name: "Friends" })).toBeTruthy();
    expect(await screen.findByText("Friends are unavailable right now.")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Try again" }));

    expect(await screen.findByText("You don't have any friends yet.")).toBeTruthy();
    expect(net.list.mock.calls[0]?.[0]).toBe(identity);
  });

  it("hydrates each page once and keeps earlier names while the next page loads", async () => {
    const user = userEvent.setup();
    net.list.mockResolvedValueOnce({ addresses: [FIRST], total: 21 }).mockResolvedValueOnce({ addresses: [NEXT], total: 21 });
    net.summaries.mockResolvedValueOnce({ [FIRST]: { name: "Sam" } });
    let finish = (_: Record<string, { name: string }>) => {};
    net.summaries.mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
    show(<ProfileFriendsCount address={ME} />);

    await user.click(await screen.findByRole("button", { name: "21 Friends" }));
    expect(await screen.findByRole("link", { name: "Sam" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Load more" }));
    expect(screen.getByRole("link", { name: "Sam" })).toBeTruthy();
    expect(net.summaries.mock.calls[1]?.[0]).toEqual([NEXT]);
    finish({ [NEXT]: { name: "Lee" } });

    expect(await screen.findByRole("link", { name: "Lee" })).toBeTruthy();
    expect(net.summaries.mock.calls.map(([addresses]) => addresses)).toEqual([[FIRST], [NEXT]]);
  });

  it("reads no friends count with another wallet's identity", () => {
    show(<ProfileFriendsCount address={FIRST} />);

    expect(net.count).not.toHaveBeenCalled();
  });
});

describe("Profile owner's invite", () => {
  it("copies the claimed NAME while Referral Rewards is enabled", async () => {
    const user = userEvent.setup();
    const writeText = vi.fn(async () => {});
    vi.stubGlobal("navigator", { ...navigator, clipboard: { writeText } });
    show(<ProfileInviteFriends profile={profile} />);

    await user.click(screen.getByRole("button", { name: "Invite Friends" }));

    expect(writeText).toHaveBeenCalledWith(`${window.location.origin}/invite/Alice`);
    expect(await screen.findByRole("button", { name: "Invite Copied" })).toBeTruthy();
  });

  it("hides the invite while Referral Rewards is disabled", () => {
    net.referralEnabled = false;
    show(<ProfileInviteFriends profile={profile} />);

    expect(screen.queryByRole("button", { name: "Invite Friends" })).toBeNull();
  });
});
