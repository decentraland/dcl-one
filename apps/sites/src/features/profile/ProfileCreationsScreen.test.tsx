// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
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
const net = vi.hoisted(() => ({ fetchProfileCreations: vi.fn() }));
const auth = vi.hoisted(() => ({ state: { identity: null as unknown, isConnected: false, address: null as string | null } }));
vi.mock("@/data/lib/catalyst/marketplace/profile-collectibles", async (original) => ({
  ...(await original<typeof import("@/data/lib/catalyst/marketplace/profile-collectibles")>()),
  fetchProfileCreations: net.fetchProfileCreations,
}));
vi.mock("@/data/lib/auth/index", () => ({ useAuth: () => auth.state }));

import type { ProfileCreation } from "@/data/lib/catalyst/marketplace/profile-collectibles";
import { emptyProfile, type ProfileVM } from "@/data/lib/catalyst/social/profile";
import { PROFILE_COLLECTIBLES_PAGE, profileKeys } from "@/data/lib/profile/queries";

import { ProfileCreationsScreen } from "./ProfileCreationsScreen";

const A = "0x09bc69a986e1b37fb01ef36124f4c2e14fa0ab6b";
const tabs = [
  { id: "overview", label: "Overview", href: `/profile/${A}` },
  { id: "creations", label: "Creations", href: `/profile/${A}/creations` },
];

function creation(n: number, over: Partial<ProfileCreation> = {}): ProfileCreation {
  return {
    id: `0xabc-${n}`,
    contractAddress: "0xabc",
    itemId: `${n}`,
    name: `Hat ${n}`,
    thumbnail: "https://example.com/hat.png",
    rarity: "legendary",
    network: "polygon",
    payment: { kind: "mana", mana: "369" },
    traits: { category: "hat", bodyShapes: ["BaseMale", "BaseFemale"], isSmart: true },
    ...over,
  };
}

const page = (items: ProfileCreation[], total = items.length) => ({ items, received: items.length, total });

function profile(over: Partial<ProfileVM> = {}): ProfileVM {
  return { ...emptyProfile(A), name: "Billyjuice", nameColor: "#ff8362", ...over };
}

function show(vm: ProfileVM = profile(), { own = false } = {}) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter(
    [{ path: "*", element: <ProfileCreationsScreen profile={vm} own={own} tabs={tabs} onTab={vi.fn()} /> }],
    { initialEntries: [`/profile/${A}/creations`] },
  );
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return { client, router };
}

const card = (name: string) => screen.getByRole("link", { name }).closest("article") as HTMLElement;

afterEach(() => {
  auth.state = { identity: null, isConnected: false, address: null };
  cleanup();
  vi.resetAllMocks();
});

describe("ProfileCreationsScreen", () => {
  let user: ReturnType<typeof userEvent.setup>;

  beforeEach(() => {
    user = userEvent.setup();
  });

  describe("when the member's wearables load", () => {
    let link: HTMLElement;
    let hat: HTMLElement;

    beforeEach(async () => {
      net.fetchProfileCreations.mockResolvedValue(page([creation(0)]));
      show();
      link = await screen.findByRole("link", { name: "Hat 0" });
      hat = card("Hat 0");
    });

    it("should link each card to its Shop item", () => {
      expect(link.getAttribute("href")).toBe("/shop/item/0xabc/0");
    });

    it("should read the member's creations", () => {
      expect(net.fetchProfileCreations.mock.calls[0]?.[0]).toBe(A);
    });

    it("should open on the wearables, from the first page", () => {
      expect(net.fetchProfileCreations.mock.calls[0]?.[1]).toMatchObject({ category: "wearable", skip: 0, first: PROFILE_COLLECTIBLES_PAGE });
    });

    it("should title the card with the item's name", () => {
      expect(within(hat).getByRole("heading", { level: 3, name: "Hat 0" })).toBeTruthy();
    });

    it("should show the creator in the profile's own name colour", () => {
      expect(within(hat).getByText("Billyjuice").style.getPropertyValue("--creator-color")).toBe("#ff8362");
    });

    it("should show the rarity and the MANA price", () => {
      expect(within(hat).getByText("legendary")).toBeTruthy();
      expect(hat.textContent).toContain("Price: 369 MANA");
    });

    it("should show the category, body shape and smart traits", () => {
      expect(within(hat).getByRole("img", { name: "Hat" })).toBeTruthy();
      expect(within(hat).getByRole("img", { name: "Unisex" })).toBeTruthy();
      expect(within(hat).getByRole("img", { name: "Smart wearable" })).toBeTruthy();
    });

    it("should link Buy to the Shop item", () => {
      expect(within(hat).getByRole("link", { name: "Buy Hat 0" }).getAttribute("href")).toBe("/shop/item/0xabc/0");
    });

    it("should link View all to the member's Shop account", () => {
      expect(screen.getByRole("link", { name: "View all" }).getAttribute("href")).toBe(`/shop/accounts/${A}`);
    });

    it("should announce the count in the live region when loading ends", () => {
      expect(screen.getByText("1 creation").closest('[role="status"]')).not.toBeNull();
    });
  });

  describe("when an item is not for sale", () => {
    let hat: HTMLElement;

    beforeEach(async () => {
      net.fetchProfileCreations.mockResolvedValue(page([creation(0, { payment: { kind: "none" } })]));
      show();
      hat = await screen.findByRole("link", { name: "Hat 0" }).then(() => card("Hat 0"));
    });

    it("should say it is not for sale", () => {
      expect(within(hat).getByText("Not for sale")).toBeTruthy();
    });

    it("should not offer Buy", () => {
      expect(within(hat).queryByRole("link", { name: "Buy Hat 0" })).toBeNull();
    });

    it("should offer the Shop page instead", () => {
      expect(within(hat).getByRole("link", { name: "View in Shop: Hat 0" }).getAttribute("href")).toBe("/shop/item/0xabc/0");
    });
  });

  describe.each([
    ["signed out", false],
    ["signed in", true],
  ])("when an item is credit-eligible and the visitor is %s", (_session, signedIn) => {
    let hat: HTMLElement;

    beforeEach(async () => {
      if (signedIn) auth.state = { identity: { signer: A }, isConnected: true, address: "0x2222222222222222222222222222222222222222" };
      net.fetchProfileCreations.mockResolvedValue(page([creation(0, { payment: { kind: "credits", credits: "12" } })]));
      show();
      hat = await screen.findByRole("link", { name: "Hat 0" }).then(() => card("Hat 0"));
    });

    it("should price it in credits, as its Shop card does", () => {
      expect(hat.textContent).toContain("Price: 12 credits");
    });

    it("should not show MANA", () => {
      expect(within(hat).queryByText(/MANA/)).toBeNull();
    });

    it("should offer Buy", () => {
      expect(within(hat).getByRole("link", { name: "Buy Hat 0" })).toBeTruthy();
    });
  });

  describe("when the credit quote failed", () => {
    let hat: HTMLElement;

    beforeEach(async () => {
      net.fetchProfileCreations.mockResolvedValue(page([creation(0, { payment: { kind: "creditsUnavailable" } })]));
      show();
      hat = await screen.findByRole("link", { name: "Hat 0" }).then(() => card("Hat 0"));
    });

    it("should say the credit price is unavailable", () => {
      expect(within(hat).getByText("Credits price unavailable")).toBeTruthy();
    });

    it("should not show MANA instead", () => {
      expect(within(hat).queryByText(/MANA/)).toBeNull();
    });

    it("should not say the item is not for sale", () => {
      expect(within(hat).queryByText("Not for sale")).toBeNull();
    });
  });

  describe("when every row of a page fails to read", () => {
    beforeEach(() => {
      net.fetchProfileCreations.mockResolvedValue({ items: [], received: PROFILE_COLLECTIBLES_PAGE, total: 30 });
      show();
    });

    it("should say the page could not be read", async () => {
      expect(await screen.findByText("These creations could not be read right now.")).toBeTruthy();
    });

    it("should not say nothing exists", async () => {
      await screen.findByText("These creations could not be read right now.");
      expect(screen.queryByText("This account has not created anything yet.")).toBeNull();
    });

    it("should offer Load more", async () => {
      await screen.findByText("These creations could not be read right now.");
      expect(screen.getByRole("button", { name: "Load more" })).toBeTruthy();
    });
  });

  describe("when a single unreadable page has no next page to load", () => {
    beforeEach(async () => {
      net.fetchProfileCreations.mockResolvedValueOnce({ items: [], received: 2, total: 2 }).mockResolvedValueOnce(page([creation(0)]));
      show();
      await screen.findByText("These creations could not be read right now.");
    });

    it("should not offer Load more", () => {
      expect(screen.queryByRole("button", { name: "Load more" })).toBeNull();
    });

    describe("and Try again is clicked", () => {
      beforeEach(async () => {
        await user.click(screen.getByRole("button", { name: "Try again" }));
      });

      it("should show the page", async () => {
        expect(await screen.findByRole("link", { name: "Hat 0" })).toBeTruthy();
      });
    });
  });

  describe("when a card's image, rarity and traits are missing", () => {
    let hat: HTMLElement;

    beforeEach(async () => {
      net.fetchProfileCreations.mockResolvedValue(
        page([creation(0, { thumbnail: undefined, rarity: undefined, traits: { bodyShapes: [], isSmart: false } })]),
      );
      show();
      hat = await screen.findByRole("link", { name: "Hat 0" }).then(() => card("Hat 0"));
    });

    it("should show no image", () => {
      expect(hat.querySelector("img")).toBeNull();
    });

    it("should not invent a rarity", () => {
      expect(within(hat).queryByText("common")).toBeNull();
    });

    it("should not invent traits", () => {
      expect(within(hat).queryAllByRole("img")).toHaveLength(0);
    });
  });

  describe("when the profile has no name", () => {
    beforeEach(() => {
      net.fetchProfileCreations.mockResolvedValue(page([creation(0)]));
      show(emptyProfile(A));
    });

    it("should name the creator by short address", async () => {
      expect(await screen.findByText("0x09bc\u2026ab6b")).toBeTruthy();
    });
  });

  describe("when the member has emotes", () => {
    beforeEach(async () => {
      net.fetchProfileCreations.mockImplementation(async (_a: string, params: { category: string }) =>
        page(params.category === "emote" ? [creation(9, { name: "Dance" })] : [creation(0)]),
      );
      show();
      await screen.findByRole("link", { name: "Hat 0" });
    });

    describe("and the Emotes tab is clicked", () => {
      beforeEach(async () => {
        await user.click(screen.getByRole("tab", { name: "Emotes" }));
      });

      it("should read and show the emotes instead of the wearables", async () => {
        expect(await screen.findByRole("link", { name: "Dance" })).toBeTruthy();
        expect(screen.queryByRole("link", { name: "Hat 0" })).toBeNull();
      });

      it("should select the Emotes tab", async () => {
        await screen.findByRole("link", { name: "Dance" });
        expect(screen.getByRole("tab", { name: "Emotes" }).getAttribute("aria-selected")).toBe("true");
      });

      it("should label the category panel, inside the profile frame's own tabpanel, with the tab", async () => {
        await screen.findByRole("link", { name: "Dance" });
        const emotes = screen.getByRole("tab", { name: "Emotes" });
        const panel = document.getElementById(emotes.getAttribute("aria-controls") ?? "");
        expect(panel?.getAttribute("role")).toBe("tabpanel");
        expect(panel?.getAttribute("aria-labelledby")).toBe(emotes.id);
      });
    });

    describe("and the categories are reached with the keyboard", () => {
      it("should put only the selected tab in the tab order", () => {
        expect(screen.getByRole("tab", { name: "Wearables" }).tabIndex).toBe(0);
        expect(screen.getByRole("tab", { name: "Emotes" }).tabIndex).toBe(-1);
      });

      describe("and ArrowRight is pressed on the first tab", () => {
        beforeEach(async () => {
          const wearables = screen.getByRole("tab", { name: "Wearables" });
          act(() => wearables.focus());
          await user.keyboard("{ArrowRight}");
        });

        it("should move to the next category and focus its tab", async () => {
          expect(await screen.findByRole("link", { name: "Dance" })).toBeTruthy();
          expect(document.activeElement).toBe(screen.getByRole("tab", { name: "Emotes" }));
        });

        describe("and ArrowRight is pressed on the last tab", () => {
          beforeEach(async () => {
            await screen.findByRole("link", { name: "Dance" });
            await user.keyboard("{ArrowRight}");
          });

          it("should wrap to the first category", async () => {
            expect(await screen.findByRole("link", { name: "Hat 0" })).toBeTruthy();
            expect(screen.getByRole("tab", { name: "Wearables" }).getAttribute("aria-selected")).toBe("true");
          });
        });
      });
    });
  });

  describe("when the panel is loading", () => {
    let resolve: (value: ReturnType<typeof page>) => void;
    let panel: () => HTMLElement;

    beforeEach(() => {
      resolve = () => {};
      net.fetchProfileCreations.mockReturnValueOnce(new Promise((r) => (resolve = r)));
      show();
      panel = () => document.getElementById(screen.getByRole("tab", { name: "Wearables" }).getAttribute("aria-controls") ?? "") as HTMLElement;
    });

    it("should make the panel a tab stop while it holds nothing to focus", () => {
      expect(panel().tabIndex).toBe(0);
    });

    describe("and the rows arrive", () => {
      beforeEach(async () => {
        await act(async () => resolve(page([creation(0)])));
        await screen.findByRole("link", { name: "Hat 0" });
      });

      it("should take the panel out of the tab order", () => {
        expect(panel().tabIndex).toBe(-1);
      });
    });
  });

  describe("when the member has created nothing", () => {
    beforeEach(() => {
      net.fetchProfileCreations.mockResolvedValue(page([]));
    });

    describe("and a visitor reads the profile", () => {
      beforeEach(() => {
        show();
      });

      it("should tell the visitor the member has created nothing", async () => {
        expect(await screen.findByText("This account has not created anything yet.")).toBeTruthy();
      });
    });

    describe("and the owner reads the profile", () => {
      beforeEach(() => {
        show(profile(), { own: true });
      });

      it("should tell the owner their creations will appear here", async () => {
        expect(await screen.findByText("Items you create will appear here.")).toBeTruthy();
      });
    });
  });

  describe("when the read fails", () => {
    beforeEach(() => {
      net.fetchProfileCreations.mockRejectedValueOnce(new Error("503")).mockResolvedValueOnce(page([creation(0)]));
      show();
    });

    it("should say the read failed", async () => {
      expect(await screen.findByText("Creations are unavailable right now.")).toBeTruthy();
    });

    it("should never show an empty list", async () => {
      await screen.findByText("Creations are unavailable right now.");
      expect(screen.queryByText("This account has not created anything yet.")).toBeNull();
    });

    describe("and Try again succeeds", () => {
      beforeEach(async () => {
        await screen.findByText("Creations are unavailable right now.");
        await user.click(screen.getByRole("button", { name: "Try again" }));
      });

      it("should show the list and focus its first card", async () => {
        expect(await screen.findByRole("link", { name: "Hat 0" })).toBeTruthy();
        await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("link", { name: "Hat 0" })));
      });
    });
  });

  describe("when the read fails and trying again fails too", () => {
    let fail: (reason: Error) => void;

    beforeEach(async () => {
      net.fetchProfileCreations
        .mockRejectedValueOnce(new Error("503"))
        .mockReturnValueOnce(new Promise((_, reject) => (fail = reject)));
      show();
      await screen.findByText("Creations are unavailable right now.");
      await user.click(screen.getByRole("button", { name: "Try again" }));
    });

    it("should read from scratch, so Try again leaves and focus falls to the page", async () => {
      expect(await screen.findByText("Loading creations")).toBeTruthy();
    });

    describe("and the retry fails", () => {
      beforeEach(async () => {
        await screen.findByText("Loading creations");
        await act(async () => fail(new Error("503")));
      });

      it("should say the read failed and focus Try again", async () => {
        expect(await screen.findByText("Creations are unavailable right now.")).toBeTruthy();
        await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("button", { name: "Try again" })));
      });
    });
  });

  describe("when the read fails and the retry is loading", () => {
    let release: (value: unknown) => void;

    beforeEach(async () => {
      net.fetchProfileCreations
        .mockRejectedValueOnce(new Error("503"))
        .mockReturnValueOnce(new Promise((resolve) => (release = resolve)));
      show();
      await screen.findByText("Creations are unavailable right now.");
      await user.click(screen.getByRole("button", { name: "Try again" }));
    });

    it("should show the retry loading instead of the stale error", async () => {
      expect(await screen.findByText("Loading creations")).toBeTruthy();
      expect(screen.queryByText("Creations are unavailable right now.")).toBeNull();
    });

    describe("and the retry ends empty", () => {
      beforeEach(async () => {
        await screen.findByText("Loading creations");
        await act(async () => release(page([])));
      });

      it("should say nothing was created and focus the panel", async () => {
        expect(await screen.findByText("This account has not created anything yet.")).toBeTruthy();
        await waitFor(() => expect(document.activeElement?.contains(screen.getByText("This account has not created anything yet."))).toBe(true));
      });
    });
  });

  describe("when a background re-read of a shown list fails", () => {
    let client: QueryClient;

    beforeEach(async () => {
      net.fetchProfileCreations.mockResolvedValueOnce(page([creation(0)])).mockRejectedValueOnce(new Error("503"));
      ({ client } = show());
      await screen.findByRole("link", { name: "Hat 0" });
      await act(async () => {
        await client.refetchQueries({ queryKey: profileKeys.creations(A, "wearable") });
        await new Promise((resolve) => setTimeout(resolve, 0));
      });
    });

    it("should record the error", () => {
      expect(client.getQueryState(profileKeys.creations(A, "wearable"))?.status).toBe("error");
    });

    it("should keep the list on screen", () => {
      expect(screen.getByRole("link", { name: "Hat 0" })).toBeTruthy();
    });

    it("should not show the error state", () => {
      expect(screen.queryByText("Creations are unavailable right now.")).toBeNull();
    });
  });

  describe("when Load more brings the last page and the offsets shifted", () => {
    beforeEach(async () => {
      const first = Array.from({ length: PROFILE_COLLECTIBLES_PAGE }, (_, n) => creation(n));
      // A newer item shifted the offsets: the second page repeats the first page's last row.
      net.fetchProfileCreations
        .mockResolvedValueOnce(page(first, 26))
        .mockResolvedValueOnce(page([creation(PROFILE_COLLECTIBLES_PAGE - 1), creation(24), creation(25)], 26));
      show();
      await screen.findByRole("link", { name: "Hat 0" });
      await user.click(screen.getByRole("button", { name: "Load more" }));
      await screen.findByRole("link", { name: "Hat 25" });
    });

    it("should load after the rows shown", () => {
      expect(screen.getByRole("link", { name: "Hat 25" })).toBeTruthy();
      expect(net.fetchProfileCreations.mock.calls[1]?.[1]).toMatchObject({ skip: PROFILE_COLLECTIBLES_PAGE });
    });

    it("should move focus to the first new card, not the page body, as the button went away", async () => {
      await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("link", { name: "Hat 24" })));
    });

    it("should show each row once", () => {
      expect(screen.getAllByRole("link", { name: `Hat ${PROFILE_COLLECTIBLES_PAGE - 1}` })).toHaveLength(1);
    });

    it("should stop at the catalogue's total", () => {
      expect(screen.queryByRole("button", { name: "Load more" })).toBeNull();
    });
  });

  describe("when Load more is clicked and the next page is loading", () => {
    let release: (value: unknown) => void;
    let button: HTMLElement;

    beforeEach(async () => {
      const first = Array.from({ length: PROFILE_COLLECTIBLES_PAGE }, (_, n) => creation(n));
      net.fetchProfileCreations
        .mockResolvedValueOnce(page(first, 60))
        .mockReturnValueOnce(new Promise((resolve) => (release = resolve)));
      show();
      await screen.findByRole("link", { name: "Hat 0" });
      button = screen.getByRole("button", { name: "Load more" });
      await user.click(button);
      await screen.findByText("Loading more creations");
    });

    it("should say it is loading more", () => {
      expect(screen.getByText("Loading more creations")).toBeTruthy();
    });

    it("should keep focus on Load more", () => {
      expect(document.activeElement).toBe(button);
    });

    it("should mark Load more disabled", () => {
      expect(button.getAttribute("aria-disabled")).toBe("true");
    });

    describe("and Load more is clicked again", () => {
      beforeEach(async () => {
        await user.click(button);
      });

      it("should not ask twice", () => {
        expect(net.fetchProfileCreations).toHaveBeenCalledTimes(2);
      });
    });

    describe("and the page arrives with more remaining", () => {
      beforeEach(async () => {
        await act(async () => release(page([creation(24)], 60)));
      });

      it("should show the new rows and keep the focus on the button that stays", async () => {
        expect(await screen.findByRole("link", { name: "Hat 24" })).toBeTruthy();
        expect(document.activeElement).toBe(button);
      });
    });
  });

  describe("when the last Load more brings only rows already shown", () => {
    beforeEach(async () => {
      const first = Array.from({ length: PROFILE_COLLECTIBLES_PAGE }, (_, n) => creation(n));
      net.fetchProfileCreations.mockResolvedValueOnce(page(first, 25)).mockResolvedValueOnce(page([creation(23)], 25));
      show();
      await screen.findByRole("link", { name: "Hat 0" });
      await user.click(screen.getByRole("button", { name: "Load more" }));
    });

    it("should focus the panel", async () => {
      await waitFor(() => expect(screen.queryByRole("button", { name: "Load more" })).toBeNull());
      await waitFor(() => expect(document.activeElement).not.toBe(document.body));
      expect(document.activeElement?.contains(screen.getByRole("link", { name: "Hat 0" }))).toBe(true);
    });
  });

  describe("when the next page fails", () => {
    let failure: HTMLElement;

    beforeEach(async () => {
      const first = Array.from({ length: PROFILE_COLLECTIBLES_PAGE }, (_, n) => creation(n));
      net.fetchProfileCreations
        .mockResolvedValueOnce(page(first, 30))
        .mockRejectedValueOnce(new Error("503"))
        .mockResolvedValueOnce(page([creation(24)], 30));
      show();
      await screen.findByRole("link", { name: "Hat 0" });
      await user.click(screen.getByRole("button", { name: "Load more" }));
      failure = await screen.findByText("More creations could not be loaded.");
    });

    it("should announce the failure", () => {
      expect(failure.getAttribute("role")).toBe("status");
    });

    it("should keep the loaded rows", () => {
      expect(screen.getByRole("link", { name: "Hat 0" })).toBeTruthy();
    });

    it("should keep the focus on the same button while it turns into Try again", () => {
      expect(document.activeElement).toBe(screen.getByRole("button", { name: "Try again" }));
    });

    describe("and Try again is clicked", () => {
      beforeEach(async () => {
        await user.click(screen.getByRole("button", { name: "Try again" }));
      });

      it("should retry the page", async () => {
        await waitFor(() => expect(screen.getByRole("link", { name: "Hat 24" })).toBeTruthy());
      });
    });
  });
});
