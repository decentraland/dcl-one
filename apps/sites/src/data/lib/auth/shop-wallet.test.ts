import { afterEach, describe, expect, it, vi } from "vitest";

const facts = vi.hoisted(() => ({ inApp: null as { address: string; token: string } | null, injected: false, provider: null as unknown }));
vi.mock("./thirdweb/session", () => ({ getThirdwebSession: () => facts.inApp }));
vi.mock("./wallet", async (actual) => ({
  ...(await actual<typeof import("./wallet")>()),
  hasWallet: () => facts.injected,
  walletProvider: () => facts.provider,
}));

import { playShopWallet } from "./shop-wallet";

const ACCOUNT = "0x" + "ab".repeat(20);

afterEach(() => {
  facts.inApp = null;
  facts.injected = false;
  facts.provider = null;
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
