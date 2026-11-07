import { afterEach, describe, expect, it, vi } from "vitest";

const facts = vi.hoisted(() => ({
  inApp: null as { address: string; token: string } | null,
  injected: false,
  provider: null as unknown,
  recorded: null as "web3" | "managed" | null,
}));
vi.mock("./thirdweb/session", () => ({ getThirdwebSession: () => facts.inApp }));
vi.mock("./wallet", async (actual) => ({
  ...(await actual<typeof import("./wallet")>()),
  hasWallet: () => facts.injected,
  walletProvider: () => facts.provider,
}));

vi.mock("./signin-kind", () => ({ signInKindFor: () => facts.recorded }));

import { currentWalletKind, playShopWallet, sitesWalletKind, web3Usable } from "./shop-wallet";

const ACCOUNT = "0x" + "ab".repeat(20);

afterEach(() => {
  facts.inApp = null;
  facts.injected = false;
  facts.provider = null;
  facts.recorded = null;
});

describe("the in-world Shop's wallet in the web shell", () => {
  it("is the in-app wallet when this browser's in-app session is the login's account", async () => {
    facts.inApp = { address: ACCOUNT.toUpperCase().replace("0X", "0x"), token: "t" };
    facts.injected = true;
    const wallet = playShopWallet();
    expect(wallet.kind(ACCOUNT)).toBe("managed");
    const provider = wallet.provider(ACCOUNT);
    expect(await provider?.request({ method: "eth_accounts" })).toEqual([ACCOUNT]);
    await expect(provider?.request({ method: "eth_sendTransaction", params: [] })).rejects.toMatchObject({ code: 4200 });
  });

  it("is the browser wallet when the in-app session is another account's, or none", () => {
    const browser = { request: vi.fn() };
    facts.inApp = { address: "0x" + "cd".repeat(20), token: "t" };
    facts.injected = true;
    facts.provider = browser;
    expect(playShopWallet().kind(ACCOUNT)).toBe("web3");
    expect(playShopWallet().provider(ACCOUNT)).toBe(browser);
  });

  it("is none when nothing here can sign for the account", () => {
    expect(playShopWallet().kind(ACCOUNT)).toBeNull();
    expect(playShopWallet().provider(ACCOUNT)).toBeNull();
  });
});

describe("the web Shop's wallet kind: how the account signed in, never whether a wallet extension is installed", () => {
  const me = { signer: ACCOUNT };
  const other = { address: "0x" + "cd".repeat(20), token: "t" };

  it("follows the sign-in's record: a Magic or email account with MetaMask installed is managed, WalletConnect without one is web3", () => {
    expect(sitesWalletKind(me, { inApp: null, injected: true, recorded: "managed" })).toBe("managed");
    expect(sitesWalletKind(me, { inApp: null, injected: false, recorded: "web3" })).toBe("web3");
  });

  it("is managed while this browser holds the account's in-app session, whatever else is recorded", () => {
    expect(sitesWalletKind(me, { inApp: { address: ACCOUNT, token: "t" }, injected: true, recorded: "web3" })).toBe("managed");
    expect(sitesWalletKind(me, { inApp: other, injected: true, recorded: "web3" })).toBe("web3");
  });

  it("without a record (signed before it existed, or shared from another site) it guesses, for signing: web3 only with a browser wallet", () => {
    expect(sitesWalletKind(me, { inApp: null, injected: true })).toBe("web3");
    expect(sitesWalletKind(me, { inApp: null, injected: false })).toBe("managed");
    expect(sitesWalletKind(null, { inApp: null, injected: true, recorded: "web3" })).toBeNull();
  });
});

describe("the identity's kind from this browser's facts", () => {
  const me = { signer: ACCOUNT };

  it("reads the sign-in's record over the wallet extension", () => {
    facts.injected = true;
    facts.recorded = "managed";
    expect(currentWalletKind(me)).toBe("managed");
    facts.recorded = null;
    expect(currentWalletKind(me)).toBe("web3");
    facts.injected = false;
    facts.recorded = "web3";
    expect(currentWalletKind(me)).toBe("web3");
    expect(currentWalletKind(null)).toBeNull();
  });

  it("opens the Marketplace only to a recorded web3 sign-in with a wallet here: never a guess, a Magic account or WalletConnect gone", () => {
    facts.injected = true;
    facts.recorded = "web3";
    expect(web3Usable(me)).toBe(true);
    expect(playShopWallet().marketplace?.(ACCOUNT)).toBe(true);
    // A Magic or shared sign-in with MetaMask installed: managed by record, or only guessed.
    facts.recorded = "managed";
    expect(web3Usable(me)).toBe(false);
    facts.recorded = null;
    expect(web3Usable(me)).toBe(false);
    expect(playShopWallet().marketplace?.(ACCOUNT)).toBe(false);
    // WalletConnect: web3 by record, but nothing here to sign with.
    facts.injected = false;
    facts.recorded = "web3";
    expect(web3Usable(me)).toBe(false);
    expect(playShopWallet().marketplace?.(ACCOUNT)).toBe(false);
  });
});
