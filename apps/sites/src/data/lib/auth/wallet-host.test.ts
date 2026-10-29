import { afterEach, expect, it, vi } from "vitest";
import { hasWallet, installHostWalletProvider, walletProvider } from "./wallet";

afterEach(() => vi.unstubAllGlobals());

it("uses the explorer's selected provider, not a different injected wallet", () => {
  const injected = { request: vi.fn() };
  const selected = { request: vi.fn() };
  vi.stubGlobal("window", { ethereum: injected });
  const restore = installHostWalletProvider(() => selected);
  expect(walletProvider()).toBe(selected);
  expect(hasWallet()).toBe(true);
  restore();
  expect(walletProvider()).toBe(injected);
});

it("does not silently switch wallets when the host provider disconnects", () => {
  vi.stubGlobal("window", { ethereum: { request: vi.fn() } });
  const restore = installHostWalletProvider(() => null);
  expect(hasWallet()).toBe(false);
  expect(() => walletProvider()).toThrow("No browser wallet");
  restore();
});

it("recognizes the host wallet without a default injected provider", () => {
  vi.stubGlobal("window", {});
  const selected = { request: vi.fn() };
  const restore = installHostWalletProvider(() => selected);
  expect(hasWallet()).toBe(true);
  expect(walletProvider()).toBe(selected);
  restore();
});
