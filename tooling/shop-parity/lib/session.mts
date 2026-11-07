// A signed-in session for both sides, as decentraland.org's dapps keep one: the shared
// `single-sign-on-<address>` identity (@dcl/single-sign-on-client's format, which both the
// reference and apps/sites read) and, for the reference's decentraland-connect, its stored
// injected connection and a `window.ethereum` that answers with the account and forwards every
// read to the network (so the recording holds it, as any other request).
//
// The wallet is a test wallet whose key comes from the environment (SHOP_PARITY_WALLET_KEY),
// never from the repository. The ephemeral key is derived from it, so the same wallet and clock
// give the same session on every run, and the captures do not change with it.
import type { BrowserContext } from "playwright-core";
import { keccak256, toHex, type Hex } from "viem";
import { privateKeyToAccount } from "viem/accounts";

export const WALLET_KEY_ENV = "SHOP_PARITY_WALLET_KEY";

/** The test wallet's key, or a refusal naming the variable: a signed-in page never runs signed out. */
export function walletKey(env: NodeJS.ProcessEnv = process.env): Hex {
  const key = env[WALLET_KEY_ENV]?.trim();
  if (!key || !/^0x[0-9a-fA-F]{64}$/.test(key)) {
    throw new Error(`${WALLET_KEY_ENV} must hold the test wallet's 0x-prefixed private key for the signed-in pages`);
  }
  return key as Hex;
}

export type SharedIdentity = {
  ephemeralIdentity: { address: string; publicKey: string; privateKey: string };
  expiration: string;
  authChain: { type: string; payload: string; signature: string }[];
};

/** The session `key` signs at `now` (epoch ms), valid for a day. */
export async function sessionFor(key: Hex, now: number): Promise<{ address: string; identity: SharedIdentity }> {
  const wallet = privateKeyToAccount(key);
  const ephemeralKey = keccak256(toHex(`shop-parity ephemeral ${key}`)) as Hex;
  const ephemeral = privateKeyToAccount(ephemeralKey);
  const expiration = new Date(now + 24 * 60 * 60 * 1000).toISOString();
  const login = ["Decentraland Login", `Ephemeral address: ${ephemeral.address}`, `Expiration: ${expiration}`].join("\n");
  const address = wallet.address.toLowerCase();
  return {
    address,
    identity: {
      ephemeralIdentity: { address: ephemeral.address, publicKey: ephemeral.publicKey, privateKey: ephemeralKey },
      expiration,
      authChain: [
        { type: "SIGNER", payload: address, signature: "" },
        { type: "ECDSA_EPHEMERAL", payload: login, signature: await wallet.signMessage({ message: login }) },
      ],
    },
  };
}

/** Signs `context` in before any page loads, on either side. */
export async function signIn(context: BrowserContext, key: Hex, now: number, rpcUrl = "https://rpc.decentraland.org/mainnet"): Promise<string> {
  const { address, identity } = await sessionFor(key, now);
  await context.addInitScript(
    ({ address, identity, rpcUrl }) => {
      try {
        localStorage.setItem(`single-sign-on-${address}`, JSON.stringify(identity));
        localStorage.setItem("decentraland-connect-storage-key", JSON.stringify({ providerType: "injected", chainId: 1 }));
      } catch {
        // A frame without storage stays signed out; the page's own frame has it.
      }
      let id = 0;
      const listeners: Record<string, ((...args: unknown[]) => void)[]> = {};
      const provider = {
        isMetaMask: true,
        selectedAddress: address,
        chainId: "0x1",
        networkVersion: "1",
        async request({ method, params }: { method: string; params?: unknown[] }) {
          switch (method) {
            case "eth_accounts":
            case "eth_requestAccounts":
              return [address];
            case "eth_chainId":
              return "0x1";
            case "net_version":
              return "1";
            case "wallet_switchEthereumChain":
            case "wallet_addEthereumChain":
              return null;
            default: {
              const res = await fetch(rpcUrl, {
                method: "POST",
                headers: { "content-type": "application/json" },
                body: JSON.stringify({ jsonrpc: "2.0", id: ++id, method, params: params ?? [] }),
              });
              const body = await res.json();
              if (body.error) throw Object.assign(new Error(body.error.message), body.error);
              return body.result;
            }
          }
        },
        on(event: string, fn: (...args: unknown[]) => void) {
          (listeners[event] ??= []).push(fn);
          return provider;
        },
        removeListener(event: string, fn: (...args: unknown[]) => void) {
          listeners[event] = (listeners[event] ?? []).filter((f) => f !== fn);
          return provider;
        },
        enable: async () => [address],
      };
      Object.defineProperty(window, "ethereum", { value: provider, configurable: true });
    },
    { address, identity, rpcUrl },
  );
  return address;
}

/** The `x-identity-*` headers for one request to `path`, signed by `key`'s session at `now` (the real time). */
export async function signedHeaders(key: Hex, method: string, path: string, now = Date.now(), metadata = "{}"): Promise<Record<string, string>> {
  const { identity } = await sessionFor(key, now);
  const ephemeral = privateKeyToAccount(identity.ephemeralIdentity.privateKey as Hex);
  const timestamp = String(now);
  const payload = `${`${method}:${path}`.toLowerCase()}:${timestamp}:${metadata}`;
  const chain = [...identity.authChain, { type: "ECDSA_SIGNED_ENTITY", payload, signature: await ephemeral.signMessage({ message: payload }) }];
  const headers: Record<string, string> = { "x-identity-timestamp": timestamp, "x-identity-metadata": metadata };
  chain.forEach((link, i) => (headers[`x-identity-auth-chain-${i}`] = JSON.stringify(link)));
  return headers;
}
