
import { render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { createShopClient } from "../../api/shop";
import { resetShopQueries } from "../query";
import { catalogTransport, ShopHarness, type CatalogAnswers, type SessionKind } from "../fixtures";
import type { CatalogItem } from "../lib/catalogItem";
import OutfitStudio from "./OutfitStudio";
import { parseOutfitImport, toggleOutfitItem } from "./outfits";

const OUTFIT = {
  id: "0f1e2d3c-4b5a-4968-8778-695a4b3c2d1e",
  name: "Nacho Night",
  thumbnailHash: "",
  items: [{ contractAddress: "0x0000000000000000000000000000000000000abc", itemId: "7" }],
  bodyShape: "unisex",
  gradientFrom: "#3a0653",
  gradientTo: "#175e52",
  published: true,
};

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn(async () => new Response("{}", { status: 200 })));
  resetShopQueries();
});
afterEach(() => vi.unstubAllGlobals());

function studio(session: SessionKind, overrides: CatalogAnswers) {
  const transport = catalogTransport("ok", overrides);
  const signer = { address: "0x0000000000000000000000000000000000000001", sign: async () => ({}) };
  const client = createShopClient({ base: "http://shop.test", mode: "canonical", transport, signer: session === "signedOut" ? null : async () => signer });
  render(
    <ShopHarness path="/shop/outfits/manage" entry="/shop/outfits/manage" client={client} session={session}>
      <OutfitStudio />
    </ShopHarness>,
  );
}

describe("the outfit studio", () => {
  it("signed out asks to sign in", () => {
    studio("signedOut", {});
    expect(screen.getByTestId("outfit-studio-signin")).toHaveTextContent("Sign in to manage your outfits");
    expect(screen.getByRole("button", { name: "Sign in" })).toBeTruthy();
  });

  it("an account the server refuses sees the styling-team notice", async () => {
    studio("web3", {});
    expect(await screen.findByTestId("outfit-studio-unavailable")).toHaveTextContent("This area is only for the shop's styling team.");
  });

  it("a creator sees their outfits", async () => {
    studio("web3", { "/v1/outfits/all": { outfits: [OUTFIT, { ...OUTFIT, id: "x", name: "", published: false }] } });
    const rows = await screen.findAllByTestId("outfit-studio-row");
    expect(rows).toHaveLength(2);
    expect(rows[0]).toHaveTextContent("Nacho Night");
    expect(rows[0]).toHaveTextContent("Published");
    expect(rows[1]).toHaveTextContent("Untitled outfit");
    expect(rows[1]).toHaveTextContent("Draft");
  });
});

describe("outfit helpers", () => {
  const item = (id: string, wearableCategory: string): CatalogItem =>
    ({ id: `0xabc-${id}`, contractAddress: "0xABC", itemId: id, category: "wearable", wearableCategory }) as CatalogItem;

  it("a picked wearable replaces the one in its slot, and picking it again removes it", () => {
    const hat = item("1", "hat");
    const resolved = new Map([["0xabc-1", hat]]);
    const withHat = toggleOutfitItem([], hat, resolved);
    expect(withHat).toEqual([{ contractAddress: "0xabc", itemId: "1" }]);
    expect(toggleOutfitItem(withHat, item("2", "hat"), resolved)).toEqual([{ contractAddress: "0xabc", itemId: "2" }]);
    expect(toggleOutfitItem(withHat, hat, resolved)).toEqual([]);
  });

  it("a preview link imports its collection items, body shape and colors", () => {
    const parsed = parseOutfitImport(
      "https://wearable-preview.decentraland.org/?urn=urn:decentraland:matic:collections-v2:0x0000000000000000000000000000000000000abc:3&urn=urn:decentraland:off-chain:base-avatars:eyes_00&bodyShape=urn:decentraland:off-chain:base-avatars:BaseFemale&skinColor=%23aabbcc",
    );
    expect(parsed).toEqual({ items: [{ contractAddress: "0x0000000000000000000000000000000000000abc", itemId: "3" }], bodyShape: "female", colors: { skin: "aabbcc", hair: undefined, eyes: undefined } });
    expect(parseOutfitImport("nothing here")).toBeNull();
  });
});
