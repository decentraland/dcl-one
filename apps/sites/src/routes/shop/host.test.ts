import { createElement } from "react";
import { renderToString } from "react-dom/server";
import { MemoryRouter } from "react-router";
import { describe, expect, it, vi } from "vitest";

import type { ShopWalletHost } from "@ui/shop/wallet/session";

import { managedShopProvider, useSitesShopHost } from "./host";

const ACCOUNT = "0x0000000000000000000000000000000000000B0B";

describe("the managed wallet's signing seam for the Shop", () => {
  it("names its account and signs EIP-712 data through the in-app signer", async () => {
    const sign = vi.fn(async () => "0xsigned");
    const provider = managedShopProvider(ACCOUNT, sign);
    expect(await provider.request({ method: "eth_accounts" })).toEqual([ACCOUNT.toLowerCase()]);
    const typed = { domain: { chainId: 137 }, types: {}, primaryType: "MetaTransaction", message: { nonce: 1 } };
    expect(await provider.request({ method: "eth_signTypedData_v4", params: [ACCOUNT, JSON.stringify(typed)] })).toBe("0xsigned");
    expect(sign).toHaveBeenCalledWith(typed, ACCOUNT.toLowerCase());
  });

  it("never signs for another account and sends nothing itself", async () => {
    const sign = vi.fn(async () => "0xsigned");
    const provider = managedShopProvider(ACCOUNT, sign);
    await expect(provider.request({ method: "eth_signTypedData_v4", params: ["0x0000000000000000000000000000000000000001", "{}"] })).rejects.toMatchObject({ code: 4100 });
    await expect(provider.request({ method: "eth_sendTransaction", params: [{}] })).rejects.toMatchObject({ code: 4200 });
    expect(sign).not.toHaveBeenCalled();
  });
});

describe("the Shop's wallet host in sites", () => {
  it("is still resolving while the page renders on the server, so a null identity is not taken for signed out", () => {
    let host: ShopWalletHost | null = null;
    function Probe() {
      host = useSitesShopHost();
      return null;
    }
    renderToString(createElement(MemoryRouter, null, createElement(Probe)));
    expect(host).toMatchObject({ identity: null, resolving: true });
  });
});
