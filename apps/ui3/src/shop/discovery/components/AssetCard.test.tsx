// The product card's own parts: the rarity flap, the creator line while its profile loads and when its
// picture fails, the sale price with what a screen reader hears, and which row the cart button replaces.

import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { catalogRow, CREATOR, PRIMARY_WEARABLE, ShopHarness, type SessionKind } from "../fixtures";
import { toCatalogItem, type CatalogItem } from "../lib/catalogItem";
import { rarityCardPalette, rarityColor } from "../lib/rarity";
import { invalidateShopQueries } from "../query";
import AssetCard, { CardCreator } from "./AssetCard";
import CategoryFlap from "./CategoryFlap";

const FACE = "https://profile-images.example/face.png";

function profileAnswer(picture: boolean) {
  return {
    avatars: [{ ethAddress: CREATOR, userId: CREATOR, name: "nacho", hasClaimedName: true, avatar: picture ? { snapshots: { face256: FACE, body: FACE } } : {} }],
  };
}

let profiles: (() => unknown) | null = null;
beforeEach(() => {
  profiles = null;
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input instanceof Request ? input.url : input);
      if (url.includes("/lambdas/profiles") && profiles) return new Response(JSON.stringify([profiles()]), { status: 200, headers: { "content-type": "application/json" } });
      return new Response("[]", { status: 200, headers: { "content-type": "application/json" } });
    }),
  );
  invalidateShopQueries();
});
afterEach(() => vi.unstubAllGlobals());

const item = (over: Partial<CatalogItem> = {}): CatalogItem => ({ ...toCatalogItem(catalogRow(PRIMARY_WEARABLE)), ...over });

function card(it: CatalogItem, session: SessionKind = "signedOut", props: Partial<Parameters<typeof AssetCard>[0]> = {}) {
  return render(
    <ShopHarness path="*" entry="/shop/items" session={session}>
      <AssetCard item={it} {...props} />
    </ShopHarness>,
  );
}

/** Lets the batched profile read land inside act. */
const settle = () => act(async () => new Promise((r) => setTimeout(r, 120)));

describe("rarityCardPalette", () => {
  it("gives a rarity its tile, flap and label colours, and anything else the neutral tile", () => {
    const epic = rarityCardPalette("Epic");
    expect(epic.flap).toBe(rarityColor("epic"));
    expect(epic.label).toBe("#438fff");
    expect(epic.labelBg).toBe("rgba(67, 143, 255, 0.2)");
    expect(epic.background).toMatch(/^radial-gradient\(ellipse 135.5% 135.5% at 50% 50%/);
    for (const none of [null, undefined, "", "legendaryish"]) {
      expect(rarityCardPalette(none)).toEqual({ flap: "#a09ba8", stroke: "#a09ba8", label: "#cfcdd4", labelBg: "rgba(236, 235, 237, 0.2)", background: "#ecebed" });
    }
  });
});

describe("CategoryFlap", () => {
  it("is drawn in the rarity's colour with the category's glyph, and hidden from assistive tech", () => {
    render(<CategoryFlap item={{ category: "wearable", wearableCategory: "hat", rarity: "epic" }} />);
    const flap = screen.getByTestId("category-flap");
    expect(flap.style.getPropertyValue("--flap-color")).toBe(rarityColor("epic"));
    expect(flap).toHaveAttribute("aria-hidden", "true");
    expect(screen.getByTestId("category-flap-icon")).toBeTruthy();
  });
});

describe("the card's creator line", () => {
  it("stays blank while the profile loads, then shows the avatar and the name, read as by its creator", async () => {
    profiles = () => profileAnswer(true);
    render(
      <ShopHarness>
        <CardCreator address={CREATOR} />
      </ShopHarness>,
    );
    expect(screen.queryByTestId("creator")).toBeNull();
    await settle();
    expect(screen.getByTestId("creator-display")).toHaveTextContent("Nacho");
    expect(screen.getByTestId("creator-ava").getAttribute("src")).toBeTruthy();
    expect(within(screen.getByTestId("creator-name")).getByText("By Nacho")).toBeTruthy();
  });

  it("falls back to the coloured initial when the picture fails", async () => {
    profiles = () => profileAnswer(true);
    render(
      <ShopHarness>
        <CardCreator address={CREATOR} />
      </ShopHarness>,
    );
    await settle();
    fireEvent.error(screen.getByTestId("creator-ava"));
    expect(screen.queryByTestId("creator-ava")).toBeNull();
    expect(screen.getByTestId("creator-ava-letter")).toHaveTextContent("N");
  });

  it("shows a name it is given at once, without waiting for the profile", () => {
    render(
      <ShopHarness>
        <CardCreator address={CREATOR} name="Studio Nacho" />
      </ShopHarness>,
    );
    expect(screen.getByTestId("creator-display")).toHaveTextContent("Studio Nacho");
    expect(screen.getByTestId("creator-ava-letter")).toHaveTextContent("S");
  });
});

describe("the card's price row", () => {
  const sale = { compareAtCredits: 20, priceCredits: 14, saleEndsAt: Date.now() + 86_400_000 };

  it("on sale: the new and the struck price, each read with its unit, and MANA for a web3 wallet", async () => {
    card(item({ ...sale, manaWei: "4020000000000000000" }), "web3");
    await settle();
    const now = screen.getByTestId("card-price-now");
    const was = screen.getByTestId("card-price-was");
    expect(now).toHaveTextContent("14");
    expect(within(now).getByText("14 Credits")).toBeTruthy();
    expect(within(was).getByText("was 20 Credits")).toBeTruthy();
    expect(screen.getByTestId("card-price-mana")).toHaveTextContent("MANA");
  });

  it("the cart button takes the chips row's place only on a card that has something to buy", async () => {
    const { unmount } = card(item());
    await settle();
    expect(screen.getByTestId("card-cart")).toBeTruthy();
    expect(screen.getByTestId("card-price").closest("[data-chips]")).not.toBeNull();
    unmount();
    card(item({ available: 0 }));
    await settle();
    expect(screen.queryByTestId("card-cart")).toBeNull();
    expect(screen.getByTestId("card").querySelector("[data-chips]")).toBeNull();
  });

  it("keeps the thumbnail decorative: the link and the title already name the item", async () => {
    card(item());
    await settle();
    expect(screen.getByTestId("card-media").querySelector("img")?.getAttribute("alt")).toBe("");
  });
});
