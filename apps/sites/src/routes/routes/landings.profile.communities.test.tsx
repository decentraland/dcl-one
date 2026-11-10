// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, waitFor } from "@testing-library/react";
import type { ComponentProps } from "react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});

import type StProfileCommunitiesTab from "@ui/web/pages/StProfileCommunitiesTab";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { fetchMyCommunities } from "@/data/lib/catalyst/social/profile-communities";
import { emptyProfile } from "@/data/lib/catalyst/social/profile";
import { track } from "@/core/lib/telemetry/track";
import LandingsProfile from "./landings.profile";

type TabProps = ComponentProps<typeof StProfileCommunitiesTab>;
const seen: { props: TabProps | null } = { props: null };

vi.mock("@ui/web/pages/StProfileCommunitiesTab", () => ({
  default: (props: TabProps) => {
    seen.props = props;
    return null;
  },
}));
vi.mock("@/core/lib/telemetry/track", () => ({ track: vi.fn() }));
vi.mock("@/data/lib/catalyst/social/profile-communities", () => ({ fetchMyCommunities: vi.fn() }));
const auth = vi.hoisted(() => ({ identity: null as unknown }));
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => auth }));

const ADDRESS = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const IDENTITY: AuthIdentity = {
  signer: ADDRESS,
  ephemeral: { address: "0xeph", privateKey: "0xdeadbeef" },
  expiration: "2999-01-01T00:00:00.000Z",
  authChain: [],
};

function renderTab(own: boolean) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const loaderData = {
    sid: "sid-1",
    address: ADDRESS,
    own,
    tab: "communities",
    source: "live",
    profileFallback: false,
    hasClaimedName: false,
    profile: emptyProfile(ADDRESS),
    creations: { wearables: [], emotes: [] },
    creationsFallback: false,
    places: [],
    photos: [],
    photosFallback: false,
    assets: { wearables: [], names: [] },
  };
  // The exported route reads its loader data from the data router, so seed one with it.
  const router = createMemoryRouter(
    [{ id: "profile", path: "/profile/:address", Component: LandingsProfile as never }],
    { initialEntries: [`/profile/${ADDRESS}`], hydrationData: { loaderData: { profile: loaderData } } },
  );
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return client;
}

beforeEach(() => {
  seen.props = null;
  auth.identity = IDENTITY;
});
afterEach(() => {
  vi.clearAllMocks();
});

describe("the profile's communities tab", () => {
  it("lists the signed-in member's communities once they load", async () => {
    const row = { id: "c1", name: "Builders", membersCount: 3, role: "owner", thumb: null };
    vi.mocked(fetchMyCommunities).mockResolvedValue([row] as never);

    renderTab(true);

    expect(seen.props?.loading).toBe(true);
    await waitFor(() => expect(seen.props?.communities).toEqual([row]));
    expect(seen.props?.loading).toBe(false);
  });

  it("reports a failed load once, and shows no rows", async () => {
    vi.mocked(fetchMyCommunities).mockRejectedValue(new Error("401"));

    renderTab(true);

    await waitFor(() => expect(seen.props?.loading).toBe(false));
    expect(seen.props?.communities).toEqual([]);
    const failures = vi
      .mocked(track)
      .mock.calls.filter(([event]) => event === "profile_communities_load_failed");
    expect(failures).toEqual([
      ["profile_communities_load_failed", { address: ADDRESS }, { sid: "sid-1", story: "landings/profile" }],
    ]);
  });

  it("requests nothing on someone else's profile", () => {
    renderTab(false);

    expect(fetchMyCommunities).not.toHaveBeenCalled();
    expect(seen.props).toMatchObject({ communities: [], loading: false });
  });

  it("requests nothing when signed out", () => {
    auth.identity = null;

    renderTab(true);

    expect(fetchMyCommunities).not.toHaveBeenCalled();
    expect(seen.props).toMatchObject({ communities: [], loading: false });
  });
});
