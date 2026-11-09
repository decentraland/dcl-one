// @vitest-environment jsdom
// Where each sign-in records how it signed (signin-kind.ts), before the identity is set: a browser wallet or
// WalletConnect is web3, an in-app (thirdweb) account managed. The login page's MetaMask path, Magic and sign-out
// are covered beside their own tests.
import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true, $RefreshSig$: () => (type: unknown) => type, $RefreshReg$: () => {} });
});

const A = "0x" + "ab".repeat(20);
const B = "0x" + "cd".repeat(20);
const identityOf = (signer: string) => ({ signer, ephemeral: { address: "0x1", privateKey: "0x2", publicKey: "0x3" }, expiration: new Date(Date.now() + 86_400_000).toISOString(), authChain: [] });

vi.mock("./identity", () => ({
  createIdentity: async () => identityOf(A),
  createIdentityWith: async (signer: string) => identityOf(signer.toLowerCase()),
}));
vi.mock("./thirdweb/signer", () => ({
  makeInAppSigner: (auth: { walletAddress: string; token: string }) => ({ address: auth.walletAddress, token: auth.token, personalSign: async () => "0x" }),
}));
vi.mock("@ui/data/auth/walletconnect", () => ({
  hasWalletConnect: () => true,
  startWalletConnectSignIn: async () => ({ uri: "wc:x", connected: Promise.resolve(identityOf(B)) }),
}));

import { useAuth } from "./context";
import { setIdentity } from "./session";
import { forgetSignIn, signInKindFor } from "./signin-kind";
import { connectWalletConnect } from "./walletconnect";

afterEach(() => {
  forgetSignIn();
  setIdentity(null);
});

describe("each sign-in records its kind", () => {
  it("a browser wallet is web3", async () => {
    const { result } = renderHook(() => useAuth());
    await act(async () => void (await result.current.connect()));
    expect(signInKindFor(A)).toBe("web3");
  });

  it("an in-app (thirdweb) account is managed", async () => {
    const { result } = renderHook(() => useAuth());
    await act(async () => void (await result.current.completeInAppSession({ token: "t", walletAddress: A })));
    expect(signInKindFor(A)).toBe("managed");
  });

  it("WalletConnect is web3", async () => {
    const session = await connectWalletConnect();
    await session.connected;
    expect(signInKindFor(B)).toBe("web3");
  });
});
