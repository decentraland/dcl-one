// @vitest-environment jsdom
// The cart document's head script finds the cart's account as the site's session readers do. Each test writes the
// session and the carts with the site's and the Shop's own writers, so a change to either format fails here.
import { afterEach, describe, expect, it, vi } from "vitest";
import { privateKeyToAccount } from "viem/accounts";

import { evmAddress } from "@shop/model/address";
import { itemAsset } from "@shop/model/asset";
import { CHAIN } from "@shop/model/chain";
import { PRIORITY_LINE_COUNT } from "@ui/shop/cart/pictures";
import { createCartStore } from "@ui/shop/cart/store";
import { installCreatorHubHost } from "@web-core/creator-hub-host";

import { clearIdentity, setIdentity } from "@/data/lib/auth/session";
import { storeShared } from "@/data/lib/auth/single-sign-on";
import type { AuthIdentity } from "@/data/lib/auth/types";

import { CART_PICTURES_SCRIPT } from "./cart-pictures";

const A = "0x4e9c4a2502fdf71e93ed8ed6ca9ddbd891d6f295";
const B = "0x1111111111111111111111111111111111111111";
const PRIVATE = `0x${"12".repeat(32)}` as const;
// With its public half the shared record is written at once.
const PUBLIC = privateKeyToAccount(PRIVATE).publicKey;

function identity(signer: string, daysLeft: number): AuthIdentity {
  return {
    signer,
    ephemeral: { address: `0x${"c".repeat(40)}`, privateKey: PRIVATE, publicKey: PUBLIC },
    expiration: new Date(Date.now() + daysLeft * 86_400_000).toISOString(),
    authChain: [{ type: "SIGNER", payload: signer, signature: "" }],
  };
}

const collection = evmAddress("0x000000000000000000000000000000000000c011");
const picture = (n: number) => `https://example.test/${n}.png`;
const hat = (n: number) => ({ asset: itemAsset(CHAIN.POLYGON, collection, BigInt(n)), listing: { acquisition: "store" as const }, display: { name: `Hat ${n}`, thumbnail: picture(n) } });

/** Fills the cart of an account (null: signed out) through the Shop's cart store. */
function fill(account: string | null, hats: number[]) {
  const cart = createCartStore();
  cart.scopeTo(account);
  for (const n of hats) cart.add(hat(n));
}

/** The first pictures of the cart the page opens for an account: the store's own merge on sign-in. */
function pageFirst(account: string | null): string[] {
  const cart = createCartStore();
  cart.scopeTo(account);
  return cart.getSnapshot().lines.slice(0, PRIORITY_LINE_COUNT).map((line) => line.display.thumbnail!);
}

const preloads = () => [...document.head.querySelectorAll('link[rel="preload"][as="image"]')].map((link) => link.getAttribute("href"));
const run = () => new Function(CART_PICTURES_SCRIPT)();

afterEach(() => {
  clearIdentity();
  localStorage.clear();
  document.head.querySelectorAll('link[rel="preload"]').forEach((link) => link.remove());
  vi.restoreAllMocks();
});

describe("the cart document's head script", () => {
  it("preloads the signed-in account's lines, then the anonymous ones it lacks, as the store merges them", () => {
    fill(A, [1, 2]);
    fill(null, [2, 3, 4]);
    setIdentity(identity(A, 1));
    run();
    expect(preloads()).toEqual([1, 2, 3].map(picture));
    expect(preloads()).toEqual(pageFirst(A));
  });

  it("reads the freshest shared session when the site has none of its own", () => {
    fill(A, [1]);
    fill(B, [5, 6]);
    storeShared(identity(A, 1));
    storeShared(identity(B, 2));
    run();
    expect(preloads()).toEqual([5, 6].map(picture));
    expect(preloads()).toEqual(pageFirst(B));
  });

  it("reads the anonymous cart for an expired session", () => {
    fill(A, [1]);
    fill(null, [3]);
    setIdentity(identity(A, -1));
    run();
    expect(preloads()).toEqual([picture(3)]);
  });

  it("does nothing while the Explorer's Create panel frames the page with its own session", () => {
    fill(null, [3]);
    const parent = {} as Window;
    installCreatorHubHost(parent, { version: 1, identity: () => null, subscribe: () => () => {}, signIn() {}, walletProvider: () => null, openLink() {} });
    vi.spyOn(window, "parent", "get").mockReturnValue(parent);
    run();
    expect(preloads()).toEqual([]);
  });
});
