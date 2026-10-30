import { act, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import type { AuthIdentity } from "../../data/auth/identity";
import type { Eip1193Provider } from "../../data/auth/wallet";
import { createCartStore } from "../cart/store";
import { evmAddress } from "../model/address";
import { itemAsset } from "../model/asset";
import { CHAIN } from "../model/chain";
import { manaFromWhole } from "../model/money";
import { presentPayment } from "../payment/present";
import { BOOK, parcelResale, rental, SIGNED_OUT, web2 } from "../payment/fixtures";
import { readChainId, requestChainSwitch, requireChain, WrongNetworkError } from "./chain";
import { ShopWalletProvider, useShopWallet } from "./context";
import ManaGuidance from "./ManaGuidance";
import { takeWalletReturn } from "./returnTo";
import { shopSession } from "./session";

function fakeProvider(initial: { chainId: number; account: string }) {
  const handlers = new Map<string, Set<(...a: unknown[]) => void>>();
  const state = { ...initial };
  const provider: Eip1193Provider & { emit(event: string, ...args: unknown[]): void; calls: string[] } = {
    calls: [],
    async request({ method, params }) {
      provider.calls.push(method);
      if (method === "eth_chainId") return `0x${state.chainId.toString(16)}`;
      if (method === "eth_accounts") return [state.account];
      if (method === "wallet_switchEthereumChain") {
        state.chainId = Number.parseInt((params![0] as { chainId: string }).chainId, 16);
        return null;
      }
      throw Object.assign(new Error("unsupported"), { code: 4200 });
    },
    on(event, handler) {
      if (!handlers.has(event)) handlers.set(event, new Set());
      handlers.get(event)!.add(handler);
    },
    removeListener(event, handler) {
      handlers.get(event)?.delete(handler);
    },
    emit(event, ...args) {
      if (event === "accountsChanged") state.account = (args[0] as string[])[0] ?? "";
      if (event === "chainChanged") state.chainId = Number.parseInt(args[0] as string, 16);
      for (const h of handlers.get(event) ?? []) h(...args);
    },
  };
  return provider;
}

function identity(signer: string, ms = 60_000): AuthIdentity {
  return {
    signer,
    ephemeral: { address: "0x0000000000000000000000000000000000000001", privateKey: `0x${"11".repeat(32)}` },
    expiration: new Date(Date.now() + ms).toISOString(),
    authChain: [],
  };
}

describe("the session seam", () => {
  it("web3 only when the host says a web3 wallet signed the chain; never from an address or balance", () => {
    const a = BOOK.buyer;
    expect(shopSession({ identity: identity(a), walletKind: "web3" })).toEqual({ kind: "web3", wallet: a });
    expect(shopSession({ identity: identity(a), walletKind: "managed" })).toEqual({ kind: "web2", account: a });
    expect(shopSession({ identity: identity(a), walletKind: null })).toEqual({ kind: "web2", account: a });
    expect(shopSession({ identity: identity(a, -1), walletKind: "web3" })).toEqual(SIGNED_OUT);
    expect(shopSession({ identity: null, walletKind: "web3" })).toEqual(SIGNED_OUT);
  });
});

describe("the network, read and changed only on request", () => {
  it("reads eth_chainId, refuses the wrong chain without prompting, switches only when asked", async () => {
    const provider = fakeProvider({ chainId: CHAIN.ETHEREUM, account: BOOK.buyer });
    expect(await readChainId(provider)).toBe(1);
    await expect(requireChain(provider, CHAIN.POLYGON)).rejects.toBeInstanceOf(WrongNetworkError);
    expect(provider.calls).not.toContain("wallet_switchEthereumChain");
    await requestChainSwitch(provider, CHAIN.POLYGON);
    expect(await readChainId(provider)).toBe(137);
  });

  it("an account switch rescopes the cart; a wallet on another account is an explicit mismatch", async () => {
    const provider = fakeProvider({ chainId: CHAIN.POLYGON, account: BOOK.buyer });
    const cart = createCartStore({ storage: () => null });
    cart.add({ asset: itemAsset(CHAIN.POLYGON, evmAddress(BOOK.collection), 7n), display: { name: "Hat" } });
    let host = { identity: identity(BOOK.buyer), walletKind: "web3" as const, provider: () => provider };
    const wrapper = ({ children }: { children: ReactNode }) => (
      <ShopWalletProvider host={host} cart={cart}>
        {children}
      </ShopWalletProvider>
    );
    const { result, rerender } = renderHook(() => useShopWallet(), { wrapper });
    await waitFor(() => expect(result.current.chainId).toBe(137));
    expect(cart.getSnapshot().account).toBe(BOOK.buyer);
    expect(cart.getSnapshot().lines).toHaveLength(1);
    await waitFor(() => expect(result.current.walletAccount).toBe(BOOK.buyer));
    const before = result.current.revision;

    act(() => provider.emit("accountsChanged", [BOOK.reseller]));
    await waitFor(() => expect(result.current.accountMismatch).toBe(true));
    expect(cart.getSnapshot().account, "the signed session still decides the cart").toBe(BOOK.buyer);
    expect(result.current.revision).not.toBe(before);

    host = { ...host, identity: identity(BOOK.reseller) };
    rerender();
    await waitFor(() => expect(cart.getSnapshot().account).toBe(BOOK.reseller));
    expect(cart.getSnapshot().lines).toEqual([]);
    expect(result.current.accountMismatch).toBe(false);

    act(() => provider.emit("chainChanged", "0x1"));
    await waitFor(() => expect(result.current.chainId).toBe(1));
  });
});

describe("the web2 MANA guidance", () => {
  const view = presentPayment(parcelResale(web2()), web2(), { report: () => {} });
  const item = { name: "Parcel 12,-4", href: "/shop/token/0x1a4d/42" };

  it("names the network, the exact balance and the gas, and never suggests credits", () => {
    const { container } = render(
      <ManaGuidance guidance={view.guidance!} required={manaFromWhole(4000)} reasons={view.reasons} item={item} returnTo="/shop/cart" />,
    );
    expect(screen.getByRole("heading", { name: "This purchase is paid in MANA" })).toBeTruthy();
    expect(container.textContent).toContain("4,000 MANA on Ethereum");
    expect(container.textContent).toContain("Some ETH on Ethereum to pay the network fee (gas).");
    expect(container.textContent).toContain("LAND, estates and rentals are sold in MANA.");
    expect(container.textContent).not.toMatch(/credit/i);
  });

  it("connects the signed-in account's wallet first, keeping the item and the return destination", () => {
    const onConnectWallet = vi.fn();
    render(
      <ManaGuidance
        guidance={view.guidance!}
        required={manaFromWhole(4000)}
        reasons={view.reasons}
        item={item}
        returnTo="/shop/cart"
        onConnectWallet={onConnectWallet}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /Connect the wallet for 0x0000…0b0b/ }));
    expect(onConnectWallet).toHaveBeenCalled();
    expect(takeWalletReturn()).toMatchObject({ itemHref: item.href, returnTo: "/shop/cart", fromAccount: BOOK.buyer });
  });

  it("makes the account switch explicit, naming where the item goes", () => {
    const onSwitchAccount = vi.fn();
    render(
      <ManaGuidance
        guidance={view.guidance!}
        required={manaFromWhole(4000)}
        reasons={view.reasons}
        item={item}
        returnTo="/shop/cart"
        onSwitchAccount={onSwitchAccount}
      />,
    );
    expect(screen.queryByRole("button", { name: "Switch to another wallet" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Use a different wallet" }));
    expect(screen.getByRole("group", { name: "Switch account" }).textContent).toContain(
      "Parcel 12,-4 will be sent to the connected wallet's address, not to 0x0000…0b0b.",
    );
    fireEvent.click(screen.getByRole("button", { name: "Switch to another wallet" }));
    expect(onSwitchAccount).toHaveBeenCalled();
  });

  it("after the switch: the receiving account, the network switch and the revalidation are shown", () => {
    const onSwitchNetwork = vi.fn();
    const onRetry = vi.fn();
    render(
      <ManaGuidance
        guidance={view.guidance!}
        required={manaFromWhole(4000)}
        reasons={view.reasons}
        item={item}
        returnTo="/shop/cart"
        wallet={{ account: BOOK.reseller, chainId: CHAIN.POLYGON, manaBalance: manaFromWhole(10) }}
        onSwitchNetwork={onSwitchNetwork}
        revalidation={{ status: "failed", onRetry }}
      />,
    );
    expect(screen.getByText(/will be received by 0x0000…5e11/)).toBeTruthy();
    expect(screen.getByText("Your wallet is on Polygon. This purchase runs on Ethereum.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Switch to Ethereum" }));
    expect(onSwitchNetwork).toHaveBeenCalledWith(CHAIN.ETHEREUM);
    expect(screen.getByText(/holds 10 MANA on Ethereum; this purchase needs 4,000 MANA/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(onRetry).toHaveBeenCalled();
  });

  it("an explicit switch from the guidance brings the cart line to the new account's cart", async () => {
    const data = new Map<string, string>();
    const cart = createCartStore({ storage: () => ({ getItem: (k) => data.get(k) ?? null, setItem: (k, v) => void data.set(k, v), removeItem: (k) => void data.delete(k) }) });
    const resale = parcelResale(web2());
    const carry = { asset: resale.asset, listing: { orderId: "order-1" }, display: { name: "Parcel 12,-4", href: "/shop/token/x/1" } };
    const hostFor = (signer: string, walletKind: "web3" | "managed") => ({ identity: identity(signer), walletKind, provider: null });
    const onSwitchAccount = vi.fn();
    const ui = (host: ReturnType<typeof hostFor>) => (
      <ShopWalletProvider host={host} cart={cart}>
        <ManaGuidance guidance={view.guidance!} required={manaFromWhole(4000)} reasons={view.reasons} item={item} returnTo="/shop/cart" carry={carry} onSwitchAccount={onSwitchAccount} />
      </ShopWalletProvider>
    );
    const { rerender } = render(ui(hostFor(BOOK.buyer, "managed")));
    await waitFor(() => expect(cart.getSnapshot().account).toBe(BOOK.buyer.toLowerCase()));
    cart.add(carry);
    fireEvent.click(screen.getByRole("button", { name: "Use a different wallet" }));
    fireEvent.click(screen.getByRole("button", { name: "Switch to another wallet" }));
    expect(onSwitchAccount).toHaveBeenCalled();
    rerender(ui(hostFor(BOOK.reseller, "web3")));
    await waitFor(() => expect(cart.getSnapshot().account).toBe(BOOK.reseller.toLowerCase()));
    expect(cart.getSnapshot().lines.map((l) => [l.display.name, l.seen])).toEqual([["Parcel 12,-4", null]]);
    expect(takeWalletReturn()).toBeNull();
  });

  it("signed out: sign in with a wallet; a Polygon rental needs no gas", () => {
    const out = presentPayment(rental(SIGNED_OUT), SIGNED_OUT, { report: () => {} });
    const onSignIn = vi.fn();
    const polygon = { ...out.guidance!, chainId: CHAIN.POLYGON, nativeGasRequired: false };
    const { container } = render(
      <ManaGuidance guidance={polygon} required={manaFromWhole(30)} reasons={out.reasons} item={item} returnTo="/shop" onSignIn={onSignIn} />,
    );
    expect(container.textContent).toContain("No POL needed: Polygon purchases are relayed.");
    fireEvent.click(screen.getByRole("button", { name: "Sign in with a wallet" }));
    expect(onSignIn).toHaveBeenCalled();
    expect(container.textContent).not.toMatch(/credit/i);
  });
});
