// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { type ComponentProps, useState } from "react";
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

import type { ProfilePlace } from "@/data/lib/catalyst/places/profile-places";
import { emptyProfile, type ProfileVM } from "@/data/lib/catalyst/social/profile";
import { profileKeys } from "@/data/lib/profile/queries";

import { ProfilePlacesScreen } from "./ProfilePlacesScreen";

const A = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const OTHER = "0x1111111111111111111111111111111111111111";
let tabs: ComponentProps<typeof ProfilePlacesScreen>["tabs"];
let identity: { signer: string; ephemeral: { address: string; privateKey: string }; expiration: string; authChain: never[] };
let scene: ProfilePlace;
let world: ProfilePlace;

const page = (places: ProfilePlace[], total = places.length) => ({ places, total });

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

const chip = (name: string) => screen.getByRole("tab", { name });

beforeEach(() => {
  auth.state = { identity: null, isConnected: false, address: null };
  tabs = [
    { id: "overview", label: "Overview", href: `/profile/${A}` },
    { id: "places", label: "Places", href: `/profile/${A}/places` },
  ];
  identity = { signer: A, ephemeral: { address: "0xeph", privateKey: "0x01" }, expiration: "2999-01-01T00:00:00.000Z", authChain: [] };
  scene = { id: "p1", title: "ABC Clubhouse", image: "https://example.com/a.png", owner: A, world: false, basePosition: "-31,-27" };
  world = { id: "abcdcl.dcl.eth", title: "Medieval Plaza", owner: A, world: true, basePosition: "0,0", worldName: "abcdcl.dcl.eth" };
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("ProfilePlacesScreen", () => {
  describe("when a visitor views a member's places", () => {
    describe("and the member owns a place and a world", () => {
      beforeEach(async () => {
        net.fetchProfilePlaces.mockResolvedValue(page([scene, world]));
        show(profile());
        await screen.findByText("2 places");
      });

      it("should list every owned place and world with its count, owner, location and Jump In", async () => {
        expect(await screen.findByText("2 places")).toBeTruthy();
        const card = screen.getByRole("link", { name: "ABC Clubhouse" });
        expect(card.getAttribute("href")).toBe("/places/p1");
        expect(screen.getByRole("link", { name: "Medieval Plaza" }).getAttribute("href")).toBe("/places/abcdcl.dcl.eth");
        expect(screen.getAllByRole("link", { name: "Alice" })[0]?.getAttribute("href")).toBe(`/profile/${A}`);
        expect(screen.getByText("-31,-27")).toBeTruthy();
        expect(screen.getByText("abcdcl.dcl.eth")).toBeTruthy();
        const [sceneJump, worldJump] = screen.getAllByRole("link", { name: "Jump in" });
        expect(sceneJump?.getAttribute("href")).toContain("position=-31%2C-27");
        expect(worldJump?.getAttribute("href")).toContain("realm=abcdcl.dcl.eth");
      });

      it("should not show the view switch, which is the owner's only", () => {
        expect(screen.queryByRole("tablist", { name: "Places shown" })).toBeNull();
      });

      it("should show only the profile's Places panel, with no filtered panel inside it", () => {
        expect(screen.getAllByRole("tabpanel").map((panel) => panel.getAttribute("aria-labelledby"))).toEqual([
          screen.getByRole("tab", { name: "Places" }).id,
        ]);
      });

      it("should not read favourites", () => {
        expect(net.fetchFavoritePlaces).not.toHaveBeenCalled();
      });

      it("should not read the owners' profiles", () => {
        expect(net.fetchCommunityProfiles).not.toHaveBeenCalled();
      });
    });

    describe("and the catalogue holds more places than the page shows", () => {
      beforeEach(() => {
        net.fetchProfilePlaces.mockResolvedValue(page([scene, world], 250));
        show(profile());
      });

      it("should say how many of the total are shown", async () => {
        expect(await screen.findByText("Showing 2 of 250 places")).toBeTruthy();
      });
    });

    describe("and the member owns a world without a name", () => {
      beforeEach(() => {
        net.fetchProfilePlaces.mockResolvedValue(page([{ ...world, worldName: undefined }]));
        show(profile());
      });

      it("should give it no location and no Jump In, instead of a Genesis City parcel", async () => {
        const card = (await screen.findByRole("link", { name: "Medieval Plaza" })).closest("li") as HTMLElement;
        expect(within(card).queryByText("0,0")).toBeNull();
        expect(within(card).queryByRole("link", { name: /jump in/i })).toBeNull();
      });
    });

    describe("and a background re-read fails", () => {
      let client: QueryClient;

      beforeEach(async () => {
        net.fetchProfilePlaces.mockResolvedValueOnce(page([scene])).mockRejectedValueOnce(new Error("503"));
        client = show(profile());
        await screen.findByRole("link", { name: "ABC Clubhouse" });

        await act(async () => {
          await client.refetchQueries({ queryKey: profileKeys.places(A) });
          // Query observers notify on the next tick; let the page render the failed re-read.
          await new Promise((resolve) => setTimeout(resolve, 0));
        });
      });

      it("should keep the list on screen", () => {
        expect(client.getQueryState(profileKeys.places(A))?.status).toBe("error");
        expect(screen.getByRole("link", { name: "ABC Clubhouse" })).toBeTruthy();
        expect(screen.queryByText("Places are unavailable right now.")).toBeNull();
      });
    });

    describe("and the member owns one place", () => {
      beforeEach(() => {
        net.fetchProfilePlaces.mockResolvedValue(page([scene]));
        show(profile());
      });

      it("should say one place, not one places", async () => {
        expect(await screen.findByText("1 place")).toBeTruthy();
      });

      describe("and the visitor clicks its Jump In", () => {
        let jump: HTMLElement;

        beforeEach(async () => {
          jump = await screen.findByRole("link", { name: "Jump in" });
          jump.addEventListener("click", (event) => event.preventDefault());
          await userEvent.click(jump);
        });

        it("should record the Jump In as started, with the place it goes to", () => {
          expect(net.track).toHaveBeenCalledWith("jump_in_started", { place_id: "p1" }, { sid: "sid-1", story: "jump-in" });
        });

        it("should record the Jump In as handed off, with its launch link", () => {
          expect(net.track).toHaveBeenCalledWith(
            "jump_in_completed",
            { place_id: "p1", launch_url: jump.getAttribute("href") },
            { sid: "sid-1", story: "jump-in" },
          );
        });
      });

      describe("and the visitor opens the place page and the owner's profile from the card", () => {
        beforeEach(async () => {
          const title = await screen.findByRole("link", { name: "ABC Clubhouse" });
          const owner = screen.getByRole("link", { name: "Alice" });
          for (const link of [title, owner]) link.addEventListener("click", (event) => event.preventDefault());

          await userEvent.click(title);
          await userEvent.click(owner);
        });

        it("should record no Jump In", () => {
          expect(net.track).not.toHaveBeenCalled();
        });
      });
    });

    describe("and the visitor clicks a world's Jump In", () => {
      let jump: HTMLElement;

      beforeEach(async () => {
        net.fetchProfilePlaces.mockResolvedValue(page([world]));
        show(profile());
        jump = await screen.findByRole("link", { name: "Jump in" });
        jump.addEventListener("click", (event) => event.preventDefault());
        await userEvent.click(jump);
      });

      it("should link the Jump In to the world's realm", () => {
        expect(jump.getAttribute("href")).toContain("realm=abcdcl.dcl.eth");
      });

      it("should record the Jump In with its realm launch link", () => {
        expect(net.track).toHaveBeenCalledWith(
          "jump_in_completed",
          { place_id: world.id, launch_url: jump.getAttribute("href") },
          { sid: "sid-1", story: "jump-in" },
        );
      });
    });

    describe("and the member has no places", () => {
      beforeEach(() => {
        net.fetchProfilePlaces.mockResolvedValue(page([]));
        show(profile());
      });

      it("should show the visitor's note, not the owner's call to action", async () => {
        expect(await screen.findByText("This member has not registered any places yet.")).toBeTruthy();
        expect(screen.queryByRole("link", { name: /get a name/i })).toBeNull();
      });
    });

    describe("and the places are still being read", () => {
      beforeEach(() => {
        net.fetchProfilePlaces.mockReturnValue(new Promise(() => {}));
        show(profile());
      });

      it("should show loading", () => {
        expect(screen.getByText("Loading places").closest("[role=status]")).toBeTruthy();
      });
    });

    describe("and the read fails once", () => {
      beforeEach(() => {
        net.fetchProfilePlaces.mockRejectedValueOnce(new Error("503")).mockResolvedValueOnce(page([scene]));
        show(profile());
      });

      it("should say the places are unavailable, and recover on Try again", async () => {
        expect(await screen.findByText("Places are unavailable right now.")).toBeTruthy();
        expect(screen.queryByText("This member has not registered any places yet.")).toBeNull();
        await userEvent.click(screen.getByRole("button", { name: "Try again" }));
        expect(await screen.findByRole("link", { name: "ABC Clubhouse" })).toBeTruthy();
      });
    });
  });

  describe("when the owner views their empty My Places", () => {
    beforeEach(() => {
      net.fetchProfilePlaces.mockResolvedValue(page([]));
    });

    describe("and the profile is loaded", () => {
      beforeEach(() => {
        show(profile(), { own: true });
      });

      it("should announce how loading ended", async () => {
        const title = "Nothing here yet, but that's easy to change";
        expect((await screen.findByText(title, { selector: "span" })).closest("[role=status]")).toBeTruthy();
      });
    });

    describe("and the owner has no NAME", () => {
      beforeEach(() => {
        show(profile({ hasClaimedName: false }), { own: true });
      });

      it("should send the owner to get one", async () => {
        expect(await screen.findByText("Nothing here yet, but that's easy to change", { selector: "p" })).toBeTruthy();
        expect(screen.getByRole("link", { name: "Get a name" }).getAttribute("href")).toBe("/shop/names");
        expect(screen.queryByText("This member has not registered any places yet.")).toBeNull();
      });
    });

    describe("and the owner holds a NAME", () => {
      beforeEach(() => {
        show(profile({ hasClaimedName: true }), { own: true });
      });

      it("should send the owner to publish a World", async () => {
        const main = await screen.findByText(/You already have a NAME/);
        const panel = main.closest("div") as HTMLElement;
        expect(within(panel).getByRole("link", { name: "Manage World" }).getAttribute("href")).toBe("/download/creator-hub");
        expect(screen.queryByRole("link", { name: "Get a name" })).toBeNull();
      });
    });

    describe("and the profile has no claimed-NAME flag", () => {
      beforeEach(() => {
        const vm = profile();
        delete (vm as Partial<ProfileVM>).hasClaimedName;
        show(vm, { own: true });
      });

      it("should treat it as one without a NAME", async () => {
        expect(await screen.findByRole("link", { name: "Get a name" })).toBeTruthy();
      });
    });

    describe("and the profile is not ready yet", () => {
      let client: QueryClient;

      beforeEach(() => {
        client = show(profile({ hasClaimedName: false }), { own: true, profileReady: false });
      });

      it("should wait for the profile before choosing which call to action to show", async () => {
        await waitFor(() => expect(client.getQueryState(profileKeys.places(A))?.status).toBe("success"));
        expect(screen.getByText("Loading places").closest("[role=status]")).toBeTruthy();
        expect(screen.queryByRole("link", { name: "Get a name" })).toBeNull();
      });
    });
  });

  describe("when the owner views their favourites", () => {
    describe("and the owner is signed in with no favourites", () => {
      beforeEach(() => {
        auth.state = { identity, isConnected: true, address: A };
        net.fetchProfilePlaces.mockResolvedValue(page([scene]));
        net.fetchFavoritePlaces.mockResolvedValue(page([]));
      });

      describe("and the screen renders", () => {
        beforeEach(async () => {
          show(profile(), { own: true });
          await screen.findByText("1 place");
        });

        it("should not read favourites until the owner picks them, then select that tab", async () => {
          expect(net.fetchFavoritePlaces).not.toHaveBeenCalled();
          expect(chip("My Places").getAttribute("aria-selected")).toBe("true");
          expect(screen.getByRole("tabpanel", { name: /My Places/ }).getAttribute("aria-labelledby")).toBe(chip("My Places").id);

          await userEvent.click(chip("Favourites"));

          expect(chip("Favourites").getAttribute("aria-selected")).toBe("true");
          expect(chip("My Places").getAttribute("aria-selected")).toBe("false");
          expect(screen.getByRole("tabpanel", { name: /Favourites/ }).getAttribute("aria-labelledby")).toBe(chip("Favourites").id);
          expect(await screen.findByText("Nothing saved yet", { selector: "p" })).toBeTruthy();
          expect(net.fetchFavoritePlaces).toHaveBeenCalledWith(identity, expect.anything(), expect.anything());
          expect(screen.getByRole("link", { name: "Explore places" }).getAttribute("href")).toBe("/places");
        });

        describe("and the owner presses ArrowRight on the My Places tab", () => {
          beforeEach(async () => {
            chip("My Places").focus();
            await userEvent.keyboard("{ArrowRight}");
          });

          it("should move focus to Favourites, as a tab list does", () => {
            expect(document.activeElement).toBe(chip("Favourites"));
          });

          it("should switch the view to Favourites", async () => {
            expect(await screen.findByText("Nothing saved yet", { selector: "p" })).toBeTruthy();
          });
        });
      });

      describe("and the viewer stops being the owner", () => {
        beforeEach(async () => {
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
          await userEvent.click(await screen.findByRole("tab", { name: "Favourites" }));
        });

        it("should go back to the owned places", async () => {
          expect(await screen.findByText("Nothing saved yet", { selector: "p" })).toBeTruthy();

          await userEvent.click(screen.getByRole("button", { name: "sign out" }));

          expect(await screen.findByRole("link", { name: "ABC Clubhouse" })).toBeTruthy();
          expect(screen.queryByText("Nothing saved yet")).toBeNull();
        });
      });
    });

    describe("and the owner is signed out", () => {
      beforeEach(async () => {
        net.fetchProfilePlaces.mockResolvedValue(page([]));
        show(profile(), { own: true });
        await userEvent.click(chip("Favourites"));
      });

      it("should ask the owner to sign in instead of reading", async () => {
        expect(screen.getByText("Sign in to see your favourites.")).toBeTruthy();
        await userEvent.click(screen.getByRole("button", { name: "Sign in" }));
        expect(net.openSignIn).toHaveBeenCalledOnce();
        expect(net.fetchFavoritePlaces).not.toHaveBeenCalled();
        expect(screen.queryByText("Nothing saved yet")).toBeNull();
      });
    });

    describe("and the owner is signed in with no owned places", () => {
      beforeEach(() => {
        auth.state = { identity, isConnected: true, address: A };
        net.fetchProfilePlaces.mockResolvedValue(page([]));
      });

      describe("and the favourites read fails", () => {
        beforeEach(async () => {
          net.fetchFavoritePlaces.mockRejectedValue(new Error("401"));
          show(profile(), { own: true });
          await userEvent.click(chip("Favourites"));
        });

        it("should say the favourites are unavailable", async () => {
          expect(await screen.findByText("Your favourites are unavailable right now.")).toBeTruthy();
          expect(screen.queryByText("Nothing saved yet")).toBeNull();
        });
      });

      describe("and the favourites read fails once", () => {
        beforeEach(async () => {
          net.fetchFavoritePlaces.mockRejectedValueOnce(new Error("503")).mockResolvedValueOnce(page([scene]));
          net.fetchCommunityProfiles.mockResolvedValue({});
          show(profile(), { own: true });
          await userEvent.click(chip("Favourites"));
        });

        it("should recover the favourites on Try again", async () => {
          await userEvent.click(await screen.findByRole("button", { name: "Try again" }));

          expect(await screen.findByRole("link", { name: "ABC Clubhouse" })).toBeTruthy();
          expect(screen.queryByText("Your favourites are unavailable right now.")).toBeNull();
        });
      });

      describe("and the saved places belong to another member", () => {
        beforeEach(async () => {
          net.fetchFavoritePlaces.mockResolvedValue(page([{ ...scene, owner: OTHER }, { ...world, owner: OTHER }]));
          net.fetchCommunityProfiles.mockResolvedValue({ [OTHER]: { name: "Bob", picture: null, claimed: true } });
          show(profile(), { own: true });
          await userEvent.click(chip("Favourites"));
        });

        it("should name each saved place's owner", async () => {
          expect((await screen.findAllByRole("link", { name: "Bob" }))[0]?.getAttribute("href")).toBe(`/profile/${OTHER}`);
        });

        it("should read the owners in one batch", async () => {
          await screen.findAllByRole("link", { name: "Bob" });
          expect(net.fetchCommunityProfiles).toHaveBeenCalledOnce();
          expect(net.fetchCommunityProfiles).toHaveBeenCalledWith([OTHER], expect.anything());
        });
      });

      describe("and a saved place is the owner's own", () => {
        beforeEach(async () => {
          net.fetchFavoritePlaces.mockResolvedValue(page([scene]));
          net.fetchCommunityProfiles.mockResolvedValue({});
          show(profile(), { own: true });
          await userEvent.click(chip("Favourites"));
        });

        it("should credit it to the owner's profile", async () => {
          expect((await screen.findByRole("link", { name: "Alice" })).getAttribute("href")).toBe(`/profile/${A}`);
        });
      });

      describe("and the saved places' owners cannot be read", () => {
        beforeEach(async () => {
          net.fetchFavoritePlaces.mockResolvedValue(page([{ ...scene, owner: OTHER }]));
          net.fetchCommunityProfiles.mockRejectedValue(new Error("down"));
          show(profile(), { own: true });
          await userEvent.click(chip("Favourites"));
        });

        it("should still list the saved places, without an owner row", async () => {
          expect(await screen.findByRole("link", { name: "ABC Clubhouse" })).toBeTruthy();
          await waitFor(() => expect(net.fetchCommunityProfiles).toHaveBeenCalled());
          expect(screen.queryByText(/^By\b/)).toBeNull();
        });
      });
    });
  });
});
