import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { privateKeyToAccount } from "viem/accounts";
import type { Hex } from "viem";

const sdk = vi.hoisted(() => ({ init: vi.fn(), connect: vi.fn(), request: vi.fn(), disconnect: vi.fn() }));
vi.mock("@walletconnect/sign-client", () => ({ default: { init: sdk.init } }));
const account = privateKeyToAccount(`0x${"12".repeat(32)}`);
const other = privateKeyToAccount(`0x${"34".repeat(32)}`);
const uri = `wc:${"a".repeat(64)}@2?relay-protocol=irn&symKey=${"b".repeat(64)}`;
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
const approved = { topic: "session-topic", namespaces: { eip155: { accounts: [`eip155:1:${account.address}`] } } };
let approval: ReturnType<typeof deferred<typeof approved>>;
const cancel: Array<() => void> = [];
beforeEach(() => {
  vi.resetModules();
  vi.clearAllMocks();
  window.__DCL_PUBLIC__ = { walletConnectProjectId: "test-project" };
  approval = deferred<typeof approved>();
  sdk.init.mockResolvedValue(sdk);
  sdk.connect.mockResolvedValue({ uri, approval: () => approval.promise });
  sdk.disconnect.mockResolvedValue(undefined);
  sdk.request.mockImplementation(({ request }: { request: { params: Hex[] } }) =>
    account.signMessage({ message: { raw: request.params[0]! } }),
  );
});
afterEach(() => { cancel.splice(0).forEach((stop) => stop()); delete window.__DCL_PUBLIC__; vi.unstubAllGlobals(); });
async function start(opts: { signal?: AbortSignal } = {}) {
  const { startWalletConnectSignIn } = await import("./walletconnect");
  const session = await startWalletConnectSignIn(opts);
  cancel.push(session.cancel);
  return session;
}

it("does not contact WalletConnect without a configured project", async () => {
  delete window.__DCL_PUBLIC__;
  await expect(start()).rejects.toThrow("not configured");
  expect(sdk.init).not.toHaveBeenCalled();
});

it("creates a verified Decentraland delegation and closes the wallet session", async () => {
  const session = await start();
  expect(session.uri).toBe(uri);
  expect(session.qrDataUrl).toMatch(/^data:image\/svg\+xml/);
  approval.resolve(approved);
  const identity = await session.connected;
  expect(identity?.signer).toBe(account.address.toLowerCase());
  expect(identity?.authChain[1]?.payload).toContain(identity!.ephemeral.address);
  expect(sdk.request).toHaveBeenCalledWith(expect.objectContaining({
    topic: "session-topic", chainId: "eip155:1",
    request: expect.objectContaining({ method: "personal_sign" }),
  }));
  expect(sdk.disconnect).toHaveBeenCalledWith(expect.objectContaining({ topic: "session-topic" }));
});

it("rejects a signature from a different wallet", async () => {
  sdk.request.mockImplementation(({ request }: { request: { params: Hex[] } }) =>
    other.signMessage({ message: { raw: request.params[0]! } }),
  );
  const session = await start();
  approval.resolve(approved);
  await expect(session.connected).rejects.toThrow("does not match");
  expect(sdk.disconnect).toHaveBeenCalled();
});

it("surfaces wallet rejection instead of leaving the QR waiting", async () => {
  const session = await start();
  approval.reject(new Error("User rejected the connection"));
  await expect(session.connected).rejects.toThrow("User rejected");
});

it("settles cancellation immediately and never signs a late approval", async () => {
  const session = await start();
  session.cancel();
  await expect(session.connected).resolves.toBeNull();
  approval.resolve(approved);
  await vi.waitFor(() => expect(sdk.disconnect).toHaveBeenCalled());
  expect(sdk.request).not.toHaveBeenCalled();
});

it("ignores signatures arriving after cancellation", async () => {
  const signature = deferred<Hex>();
  sdk.request.mockReturnValue(signature.promise);
  const session = await start();
  approval.resolve(approved);
  await vi.waitFor(() => expect(sdk.request).toHaveBeenCalled());
  session.cancel();
  await expect(session.connected).resolves.toBeNull();
  signature.resolve(await account.signMessage({ message: "late signature" }));
  await vi.waitFor(() => expect(sdk.disconnect).toHaveBeenCalled());
});

it("cancels startup without opening a wallet request", async () => {
  const controller = new AbortController();
  controller.abort();
  await expect(start({ signal: controller.signal })).rejects.toThrow();
  expect(sdk.connect).not.toHaveBeenCalled();
});

it("permits retry after client initialization fails", async () => {
  sdk.init.mockRejectedValueOnce(new Error("Relay unavailable"));
  await expect(start()).rejects.toThrow("Relay unavailable");
  const session = await start();
  approval.resolve(approved);
  await expect(session.connected).resolves.not.toBeNull();
  expect(sdk.init).toHaveBeenCalledTimes(2);
});

it("loads public configuration for the static explorer", async () => {
  delete window.__DCL_PUBLIC__;
  const fetcher = vi.fn().mockResolvedValue(Response.json({ walletConnectProjectId: "explorer-project" }));
  vi.stubGlobal("fetch", fetcher);
  const { loadWalletConnectConfig, hasWalletConnect } = await import("./walletconnect");
  await expect(loadWalletConnectConfig()).resolves.toBe(true);
  expect(hasWalletConnect()).toBe(true);
  await loadWalletConnectConfig();
  expect(fetcher).toHaveBeenCalledTimes(1);
});

it("rejects malformed public configuration", async () => {
  delete window.__DCL_PUBLIC__;
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue(Response.json({ walletConnectProjectId: 123 })));
  const { loadWalletConnectConfig } = await import("./walletconnect");
  await expect(loadWalletConnectConfig()).rejects.toThrow("Invalid wallet sign-in configuration");
});
