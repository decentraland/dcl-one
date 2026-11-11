// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
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
  fetchProfilePlaces: vi.fn(),
  fetchFavoritePlaces: vi.fn(),
  fetchCommunityProfiles: vi.fn(),
  track: vi.fn(),
  openSignIn: vi.fn(),
}));
const auth = vi.hoisted(() => ({ state: { identity: null as unknown, isConnected: false, address: null as string | null } }));
vi.mock("@/data/lib/catalyst/places/profile-places", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/places/profile-places")>()),
  fetchProfilePlaces: net.fetchProfilePlaces,
  fetchFavoritePlaces: net.fetchFavoritePlaces,
}));
vi.mock("@ui/data/hooks/useCommunityProfiles", () => ({ fetchCommunityProfiles: net.fetchCommunityProfiles }));
vi.mock("@/core/lib/telemetry/track", () => ({ track: net.track }));
vi.mock("@/data/lib/auth/index", () => ({ useAuth: () => auth.state }));
vi.mock("../components/auth/signin-store", async (original) => ({
  ...(await original<typeof import("../components/auth/signin-store")>()),
  openSignIn: net.openSignIn,
}));

import { SceneCard } from "@dcl/one-ui/apps/sites/provisional/SceneCard/SceneCard";

import type { ProfilePlace } from "@/data/lib/catalyst/places/profile-places";
import { emptyProfile, type ProfileVM } from "@/data/lib/catalyst/social/profile";
import { profileKeys } from "@/data/lib/profile/queries";

import { ProfilePlacesScreen } from "./ProfilePlacesScreen";

const A = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const OTHER = "0x1111111111111111111111111111111111111111";
const tabs = [
  { id: "overview", label: "Overview", href: `/profile/${A}` },
  { id: "places", label: "Places", href: `/profile/${A}/places` },
];
const identity = { signer: A, ephemeral: { address: "0xeph", privateKey: "0x01" }, expiration: "2999-01-01T00:00:00.000Z", authChain: [] };

const scene: ProfilePlace = { id: "p1", title: "ABC Clubhouse", image: "https://example.com/a.png", owner: A, world: false, basePosition: "-31,-27" };
const world: ProfilePlace = { id: "abcdcl.dcl.eth", title: "Medieval Plaza", owner: A, world: true, basePosition: "0,0", worldName: "abcdcl.dcl.eth" };

function profile(over: Partial<ProfileVM> = {}): ProfileVM {
  return { ...emptyProfile(A), name: "Alice", nameColor: "#ff8362", ...over };
}

function show(vm: ProfileVM, { own = false, profileReady = true } = {}) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter(
    [{ path: "*", element: <ProfilePlacesScreen profile={vm} profileReady={profileReady} own={own} tabs={tabs} onTab={vi.fn()} sid="sid-1" /> }],
    { initialEntries: [`/profile/${A}/places`] },
  );
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return client;
}

const chip = (name: string) => screen.getByRole("button", { name });

beforeEach(() => {
  auth.state = { identity: null, isConnected: false, address: null };
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("profile places, a member's profile", () => {
  it("lists every owned place and world with its count, owner, location and Jump In", async () => {
    net.fetchProfilePlaces.mockResolvedValue([scene, world]);

    show(profile());

    expect(await screen.findByText("2 places")).toBeTruthy();
    const card = screen.getByRole("link", { name: "ABC Clubhouse" });
    expect(card.getAttribute("href")).toBe("/places/p1");
    expect(screen.getByRole("link", { name: "Medieval Plaza" }).getAttribute("href")).toBe("/places/abcdcl.dcl.eth");
    expect(screen.getAllByRole("link", { name: "Alice" })[0]?.getAttribute("href")).toBe(`/profile/${A}`);
    expect(screen.getByRole("link", { name: "Jump in to -31,-27" }).getAttribute("href")).toContain("position=-31%2C-27");
    expect(screen.getByRole("link", { name: "Jump in to abcdcl.dcl.eth" }).getAttribute("href")).toContain("realm=abcdcl.dcl.eth");
    // The view switch is the owner's only.
    expect(screen.queryByRole("group", { name: "Places shown" })).toBeNull();
    expect(net.fetchFavoritePlaces).not.toHaveBeenCalled();
    expect(net.fetchCommunityProfiles).not.toHaveBeenCalled();
  });

  it("says one place, not one places", async () => {
    net.fetchProfilePlaces.mockResolvedValue([scene]);

    show(profile());

    expect(await screen.findByText("1 place")).toBeTruthy();
  });

  it("records a Jump In as started and handed off, with the place it goes to", async () => {
    net.fetchProfilePlaces.mockResolvedValue([scene]);
    show(profile());
    const jump = await screen.findByRole("link", { name: "Jump in" });
    jump.addEventListener("click", (event) => event.preventDefault());

    await userEvent.click(jump);

    const context = { sid: "sid-1", story: "jump-in" };
    expect(net.track).toHaveBeenCalledWith("jump_in_started", { place_id: "p1" }, context);
    expect(net.track).toHaveBeenCalledWith(
      "jump_in_completed",
      { place_id: "p1", launch_url: jump.getAttribute("href") },
      context,
    );
  });

  it("shows the visitor's note, not the owner's call to action, when the member has none", async () => {
    net.fetchProfilePlaces.mockResolvedValue([]);

    show(profile());

    expect(await screen.findByText("This member has not registered any places yet.")).toBeTruthy();
    expect(screen.queryByRole("link", { name: /get a name/i })).toBeNull();
  });

  it("shows loading while the places are read", () => {
    net.fetchProfilePlaces.mockReturnValue(new Promise(() => {}));

    show(profile());

    expect(screen.getByText("Loading places").closest("[role=status]")).toBeTruthy();
  });

  it("says the places are unavailable when the read fails, and recovers on Try again", async () => {
    net.fetchProfilePlaces.mockRejectedValueOnce(new Error("503")).mockResolvedValueOnce([scene]);

    show(profile());

    expect(await screen.findByText("Places are unavailable right now.")).toBeTruthy();
    expect(screen.queryByText("This member has not registered any places yet.")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("link", { name: "ABC Clubhouse" })).toBeTruthy();
  });
});

describe("profile places, the owner's empty My Places", () => {
  it("sends an owner without a NAME to get one", async () => {
    net.fetchProfilePlaces.mockResolvedValue([]);

    show(profile({ hasClaimedName: false }), { own: true });

    expect(await screen.findByText("Nothing here yet, but that's easy to change")).toBeTruthy();
    expect(screen.getByRole("link", { name: "Get a name" }).getAttribute("href")).toBe("/shop/names");
    expect(screen.queryByText("This member has not registered any places yet.")).toBeNull();
  });

  it("sends an owner who holds a NAME to publish a World", async () => {
    net.fetchProfilePlaces.mockResolvedValue([]);

    show(profile({ hasClaimedName: true }), { own: true });

    const main = await screen.findByText(/You already have a NAME/);
    const panel = main.closest("div") as HTMLElement;
    expect(within(panel).getByRole("link", { name: "Manage World" }).getAttribute("href")).toBe("/create/worlds");
    expect(screen.queryByRole("link", { name: "Get a name" })).toBeNull();
  });

  it("treats a profile without the claimed-NAME flag as one without a NAME", async () => {
    net.fetchProfilePlaces.mockResolvedValue([]);
    const vm = profile();
    delete (vm as Partial<ProfileVM>).hasClaimedName;

    show(vm, { own: true });

    expect(await screen.findByRole("link", { name: "Get a name" })).toBeTruthy();
  });

  it("waits for the profile before choosing which call to action to show", async () => {
    net.fetchProfilePlaces.mockResolvedValue([]);

    const client = show(profile({ hasClaimedName: false }), { own: true, profileReady: false });

    await waitFor(() => expect(client.getQueryState(profileKeys.places(A))?.status).toBe("success"));
    expect(screen.getByText("Loading places").closest("[role=status]")).toBeTruthy();
    expect(screen.queryByRole("link", { name: "Get a name" })).toBeNull();
  });
});

describe("profile places, the owner's favourites", () => {
  it("does not read favourites until the owner picks them, then marks the chip pressed", async () => {
    auth.state = { identity, isConnected: true, address: A };
    net.fetchProfilePlaces.mockResolvedValue([scene]);
    net.fetchFavoritePlaces.mockResolvedValue([]);

    show(profile(), { own: true });
    await screen.findByText("1 place");
    expect(net.fetchFavoritePlaces).not.toHaveBeenCalled();
    expect(chip("My Places").getAttribute("aria-pressed")).toBe("true");

    await userEvent.click(chip("Favourites"));

    expect(chip("Favourites").getAttribute("aria-pressed")).toBe("true");
    expect(await screen.findByText("Nothing saved yet")).toBeTruthy();
    expect(net.fetchFavoritePlaces).toHaveBeenCalledWith(identity, expect.anything(), expect.anything());
    expect(screen.getByRole("link", { name: "Explore places" }).getAttribute("href")).toBe("/places");
  });

  it("goes back to the owned places when the viewer stops being the owner", async () => {
    auth.state = { identity, isConnected: true, address: A };
    net.fetchProfilePlaces.mockResolvedValue([scene]);
    net.fetchFavoritePlaces.mockResolvedValue([]);
    function SignsOut() {
      const [own, setOwn] = useState(true);
      return (
        <>
          <button onClick={() => setOwn(false)}>sign out</button>
          <ProfilePlacesScreen profile={profile()} profileReady own={own} tabs={tabs} onTab={vi.fn()} sid="sid-1" />
        </>
      );
    }
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const router = createMemoryRouter([{ path: "*", element: <SignsOut /> }], { initialEntries: [`/profile/${A}/places`] });
    render(
      <QueryClientProvider client={client}>
        <RouterProvider router={router} />
      </QueryClientProvider>,
    );
    await userEvent.click(await screen.findByRole("button", { name: "Favourites" }));
    expect(await screen.findByText("Nothing saved yet")).toBeTruthy();

    await userEvent.click(screen.getByRole("button", { name: "sign out" }));

    expect(await screen.findByRole("link", { name: "ABC Clubhouse" })).toBeTruthy();
    expect(screen.queryByText("Nothing saved yet")).toBeNull();
  });

  it("asks a signed-out owner to sign in instead of reading", async () => {
    net.fetchProfilePlaces.mockResolvedValue([]);
    show(profile(), { own: true });

    await userEvent.click(chip("Favourites"));

    expect(screen.getByText("Sign in to see your favourites.")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Sign in" }));
    expect(net.openSignIn).toHaveBeenCalledOnce();
    expect(net.fetchFavoritePlaces).not.toHaveBeenCalled();
    expect(screen.queryByText("Nothing saved yet")).toBeNull();
  });

  it("says the favourites are unavailable when the read fails", async () => {
    auth.state = { identity, isConnected: true, address: A };
    net.fetchProfilePlaces.mockResolvedValue([]);
    net.fetchFavoritePlaces.mockRejectedValue(new Error("401"));
    show(profile(), { own: true });

    await userEvent.click(chip("Favourites"));

    expect(await screen.findByText("Your favourites are unavailable right now.")).toBeTruthy();
    expect(screen.queryByText("Nothing saved yet")).toBeNull();
  });

  it("names each saved place's owner from one batch read", async () => {
    auth.state = { identity, isConnected: true, address: A };
    net.fetchProfilePlaces.mockResolvedValue([]);
    net.fetchFavoritePlaces.mockResolvedValue([{ ...scene, owner: OTHER }, { ...world, owner: OTHER }]);
    net.fetchCommunityProfiles.mockResolvedValue({ [OTHER]: { name: "Bob", picture: null, claimed: true } });
    show(profile(), { own: true });

    await userEvent.click(chip("Favourites"));

    expect((await screen.findAllByRole("link", { name: "Bob" }))[0]?.getAttribute("href")).toBe(`/profile/${OTHER}`);
    expect(net.fetchCommunityProfiles).toHaveBeenCalledOnce();
    expect(net.fetchCommunityProfiles).toHaveBeenCalledWith([OTHER], expect.anything());
  });

  it("credits the owner's own saved place to their profile", async () => {
    auth.state = { identity, isConnected: true, address: A };
    net.fetchProfilePlaces.mockResolvedValue([]);
    net.fetchFavoritePlaces.mockResolvedValue([scene]);
    net.fetchCommunityProfiles.mockResolvedValue({});
    show(profile(), { own: true });

    await userEvent.click(chip("Favourites"));

    expect((await screen.findByRole("link", { name: "Alice" })).getAttribute("href")).toBe(`/profile/${A}`);
  });

  it("still lists the saved places, without an owner row, when the owners cannot be read", async () => {
    auth.state = { identity, isConnected: true, address: A };
    net.fetchProfilePlaces.mockResolvedValue([]);
    net.fetchFavoritePlaces.mockResolvedValue([{ ...scene, owner: OTHER }]);
    net.fetchCommunityProfiles.mockRejectedValue(new Error("down"));
    show(profile(), { own: true });

    await userEvent.click(chip("Favourites"));

    expect(await screen.findByRole("link", { name: "ABC Clubhouse" })).toBeTruthy();
    await waitFor(() => expect(net.fetchCommunityProfiles).toHaveBeenCalled());
    expect(screen.queryByText(/^by/)).toBeNull();
  });
});

describe("scene card", () => {
  it("shows no image, no owner row and no Jump In when it has none, and still opens the scene", () => {
    const { container } = render(<SceneCard title="Bare" href="/places/x" location="1,2" />);

    expect(container.querySelector("img")).toBeNull();
    expect(screen.queryByText(/^by/)).toBeNull();
    expect(screen.queryByRole("link", { name: /jump in/i })).toBeNull();
    // Without a Jump In the location is a label, not a link.
    expect(screen.getByText("1,2").closest("a")).toBeNull();
    expect(screen.getByRole("link", { name: "Bare" }).getAttribute("href")).toBe("/places/x");
  });

  it("drops an image that fails to load instead of showing a broken one", () => {
    const { container } = render(<SceneCard title="Broken" href="/places/x" imageUrl="https://example.com/missing.png" />);

    fireEvent.error(container.querySelector("img") as HTMLImageElement);

    expect(container.querySelector("img")).toBeNull();
  });
});
