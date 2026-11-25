// @vitest-environment jsdom
// The cart document's head script finds the cart's account as the site's session readers do. Each test writes the
// session and the carts with the site's and the Shop's own writers, and asks the site's reader for the account, so a
// change to either format or to the reader's rules fails here.
import { afterEach, describe, expect, it, vi } from "vitest";
import { privateKeyToAccount } from "viem/accounts";

import { PRIORITY_LINE_COUNT } from "@ui/shop/cart/pictures";
import { hat, picture, preloaded } from "@ui/shop/cart/pictures.test-helper";
import { createCartStore } from "@ui/shop/cart/store";
import { installCreatorHubHost } from "@web-core/creator-hub-host";

import { headAccountScript } from "@/data/lib/auth/head-account";
import { clearIdentity, SESSION_STORAGE_KEY, setIdentity, suspendWalletIdentity } from "@/data/lib/auth/session";
import { SINGLE_SIGN_ON_PREFIX, storeShared } from "@/data/lib/auth/single-sign-on";
import type { AuthIdentity } from "@/data/lib/auth/types";

import { cartPicturesHeadScript } from "./cart-pictures";

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

/** Fills the cart of an account (null: signed out) through the Shop's cart store. */
function fill(account: string | null, hats: number[]) {
  const cart = createCartStore();
  cart.scopeTo(account);
  for (const n of hats) cart.add(hat(n));
}

/**
 * The account that the site's session reader picks on a page load of a production build: a fresh copy of the module
 * reads the storage, and a record that fails the schema check is read as it is.
 */
async function readerAccount(): Promise<string | null> {
  vi.resetModules();
  const { setValidationDevMode } = await import("@web-core/validate");
  setValidationDevMode(false);
  vi.spyOn(console, "warn").mockImplementation(() => {});
  const { getIdentity } = await import("@/data/lib/auth/session");
  return getIdentity()?.signer.toLowerCase() ?? null;
}

/** The first pictures of the cart that the page opens: the reader's account, merged as the store merges it on sign-in. */
async function pageFirst(): Promise<string[]> {
  const cart = createCartStore();
  cart.scopeTo(await readerAccount());
  return cart.getSnapshot().lines.slice(0, PRIORITY_LINE_COUNT).map((line) => line.display.thumbnail!);
}

const run = () => new Function(cartPicturesHeadScript())();

/** The account that the head script's account function finds. */
const headAccount = (): string | false | null => new Function(`return (${headAccountScript()})()`)();

const sharedKey = (signer: string) => SINGLE_SIGN_ON_PREFIX + signer.toLowerCase();

/** Changes a session record as the site wrote it. */
function change(key: string, edit: (record: { [field: string]: any }) => void) {
  const record = JSON.parse(localStorage.getItem(key)!);
  edit(record);
  localStorage.setItem(key, JSON.stringify(record));
}

/** The site's own session record of `signer`, without the shared record that the sign-in also writes. */
function ownOnly(signer: string) {
  setIdentity(identity(signer, 1));
  localStorage.removeItem(sharedKey(signer));
}

afterEach(() => {
  clearIdentity();
  localStorage.clear();
  sessionStorage.clear();
  document.head.querySelectorAll('link[rel="preload"]').forEach((link) => link.remove());
  vi.restoreAllMocks();
});

describe("the cart document's head script", () => {
  it("preloads the signed-in account's lines, then the anonymous ones it lacks, as the store merges them", async () => {
    fill(A, [1, 2]);
    fill(null, [2, 3, 4]);
    setIdentity(identity(A, 1));
    run();
    expect(preloaded()).toEqual([1, 2, 3].map(picture));
    expect(preloaded()).toEqual(await pageFirst());
  });

  it("reads the freshest shared session when the site has none of its own", async () => {
    fill(A, [1]);
    fill(B, [5, 6]);
    storeShared(identity(A, 1));
    storeShared(identity(B, 2));
    run();
    expect(preloaded()).toEqual([5, 6].map(picture));
    expect(preloaded()).toEqual(await pageFirst());
  });

  it("reads the anonymous cart for an expired session", async () => {
    fill(A, [1]);
    fill(null, [3]);
    setIdentity(identity(A, -1));
    run();
    expect(preloaded()).toEqual([picture(3)]);
    expect(preloaded()).toEqual(await pageFirst());
  });

  it("reads the anonymous cart while a wallet switch puts the account aside, and takes no shared session then", async () => {
    fill(A, [1]);
    fill(B, [5]);
    fill(null, [3]);
    setIdentity(identity(A, 1));
    storeShared(identity(B, 2));
    suspendWalletIdentity();
    run();
    expect(preloaded()).toEqual([picture(3)]);
    expect(preloaded()).toEqual(await pageFirst());
    // With the site's own record gone, the shared session stays unused during the switch.
    localStorage.removeItem(SESSION_STORAGE_KEY);
    document.head.querySelectorAll('link[rel="preload"]').forEach((link) => link.remove());
    run();
    expect(preloaded()).toEqual([picture(3)]);
    expect(preloaded()).toEqual(await pageFirst());
  });

  it("does nothing while the Explorer's Create panel frames the page with its own session", () => {
    fill(null, [3]);
    const parent = {} as Window;
    installCreatorHubHost(parent, { version: 1, identity: () => null, subscribe: () => () => {}, signIn() {}, walletProvider: () => null, openLink() {} });
    vi.spyOn(window, "parent", "get").mockReturnValue(parent);
    run();
    expect(preloaded()).toEqual([]);
  });
});

describe("the account that the head script finds", () => {
  it.each<[string, () => void]>([
    ["a session of its own", () => ownOnly(A)],
    ["its own session with a private key that is not 32 bytes", () => {
      ownOnly(A);
      change(SESSION_STORAGE_KEY, (record) => void (record.ephemeral.privateKey = "0x12"));
    }],
    ["its own session with an auth chain link of an unknown type", () => {
      ownOnly(A);
      change(SESSION_STORAGE_KEY, (record) => void (record.authChain[0].type = "OTHER"));
    }],
    ["its own session with an expiration that is not a date", () => {
      ownOnly(A);
      change(SESSION_STORAGE_KEY, (record) => void (record.expiration = "soon"));
    }],
    ["its own session without an ephemeral private key", () => {
      ownOnly(A);
      change(SESSION_STORAGE_KEY, (record) => void delete record.ephemeral.privateKey);
    }],
    ["its own session without an ephemeral address", () => {
      ownOnly(A);
      change(SESSION_STORAGE_KEY, (record) => void delete record.ephemeral.address);
    }],
    ["a broken session of its own and a shared one", () => {
      ownOnly(A);
      localStorage.setItem(SESSION_STORAGE_KEY, "{");
      storeShared(identity(B, 1));
    }],
    ["a shared session under another account's key", () => {
      storeShared(identity(A, 1));
      localStorage.setItem(sharedKey(B), localStorage.getItem(sharedKey(A))!);
      localStorage.removeItem(sharedKey(A));
    }],
    ["a shared session whose signer is not an address", () => {
      storeShared(identity(A, 1));
      change(sharedKey(A), (record) => void (record.authChain[0].payload = "nobody"));
    }],
    ["a shared session with a private key that is not 32 bytes", () => {
      storeShared(identity(A, 1));
      change(sharedKey(A), (record) => void (record.ephemeralIdentity.privateKey = "0x12"));
    }],
    ["an expired shared session", () => {
      storeShared(identity(A, 1));
      change(sharedKey(A), (record) => void (record.expiration = new Date(Date.now() - 1_000).toISOString()));
    }],
  ])("is the account that the site's reader finds, for %s", async (_, seed) => {
    seed();
    expect(headAccount()).toBe(await readerAccount());
  });

  it("keeps its embedded text safe in a script element", () => {
    expect(cartPicturesHeadScript()).not.toMatch(/<\/script/i);
  });
});
