// A testnet wallet for the trade journey: an EIP-1193 provider whose key stays in this Node process.
// The page gets a thin `window.ethereum` that forwards each request over a Playwright binding; signing
// and sending happen here, with viem, and every other call goes to the chain's public RPC.
//
// It refuses any chain but Amoy and Sepolia, typed data whose domain (chainId or salt) names another
// chain or none, transactions for a chain other than the current one, and transactions to a contract
// outside `allowedTo`.
import { createPublicClient, createWalletClient, http, type Chain, type Hex, type Transport } from "viem";
import { privateKeyToAccount } from "viem/accounts";
import { polygonAmoy, sepolia } from "viem/chains";

export const TESTNETS: Record<number, Chain> = { [polygonAmoy.id]: polygonAmoy, [sepolia.id]: sepolia };

export const DEFAULT_RPC: Record<number, string> = {
  [polygonAmoy.id]: "https://rpc.decentraland.org/amoy",
  [sepolia.id]: "https://rpc.decentraland.org/sepolia",
};

/** A request the wallet will not carry out, with its EIP-1193 code: 4902 for a chain it does not know,
 * 4200 for a method it does not support, 4001 (rejected) for everything else. */
export class WalletRefusal extends Error {
  code: 4001 | 4200 | 4902;
  constructor(message: string, code: 4001 | 4200 | 4902 = 4001) {
    super(message);
    this.code = code;
  }
}

type Request = { method: string; params?: unknown[] };

export type TestnetWallet = {
  address: Hex;
  chainId: () => number;
  request: (req: Request) => Promise<unknown>;
  /** Hashes of the transactions this wallet broadcast, in order. */
  sent: Hex[];
  /** Called after wallet_switchEthereumChain, so the page can be told. */
  onChainChanged?: (chainId: number) => void;
};

function testnet(chainId: number): Chain {
  const chain = TESTNETS[chainId];
  if (!chain) throw new WalletRefusal(`refused: chain ${chainId} is not a testnet this journey may use (Amoy 80002, Sepolia 11155111)`, 4902);
  return chain;
}

/**
 * The chain a typed-data domain binds its signature to. Decentraland's own domains (trades, meta
 * transactions) carry it in `salt` as a uint256 rather than in `chainId`; a domain with neither is valid
 * on every chain, so it is refused too.
 */
export function domainChain(domain: { chainId?: unknown; salt?: unknown }): number {
  // viem hashes chainId only when it is a number or bigint and drops any other type from the domain, so a
  // string would pass the check below yet sign a domain that names no chain.
  if (domain.chainId != null && typeof domain.chainId !== "number" && typeof domain.chainId !== "bigint") {
    throw new WalletRefusal(`refused: the domain's chainId is a ${typeof domain.chainId}, not a number`);
  }
  if (domain.salt != null && (typeof domain.salt !== "string" || !/^0x[0-9a-fA-F]{64}$/.test(domain.salt))) {
    throw new WalletRefusal("refused: the domain's salt is not a 32-byte hex string");
  }
  const ids = [domain.chainId != null ? Number(domain.chainId) : null, domain.salt != null ? Number(BigInt(domain.salt)) : null].filter((x): x is number => x != null);
  if (!ids.length) throw new WalletRefusal("refused: typed data whose domain names no chain (neither chainId nor salt) is valid on mainnet too");
  for (const id of ids) testnet(id);
  if (new Set(ids).size > 1) throw new WalletRefusal(`refused: the domain's chainId and salt disagree (${ids.join(" vs ")})`);
  return ids[0]!;
}

export function testnetWallet(opts: {
  key: Hex;
  chainId?: number;
  transport?: (chainId: number) => Transport;
  /** The contracts transactions may go to (any case); unset, any. */
  allowedTo?: string[];
}): TestnetWallet {
  const allowedTo = opts.allowedTo && new Set(opts.allowedTo.map((a) => a.toLowerCase()));
  const account = privateKeyToAccount(opts.key);
  const transport = opts.transport ?? ((id: number) => http(DEFAULT_RPC[id]));
  let current = testnet(opts.chainId ?? polygonAmoy.id).id;
  const clients = new Map<number, { pub: ReturnType<typeof createPublicClient>; wallet: ReturnType<typeof createWalletClient> }>();
  const client = (id: number) => {
    let c = clients.get(id);
    if (!c) {
      const chain = testnet(id);
      c = { pub: createPublicClient({ chain, transport: transport(id) }), wallet: createWalletClient({ account, chain, transport: transport(id) }) };
      clients.set(id, c);
    }
    return c;
  };
  const own = (from: unknown) => {
    if (typeof from === "string" && from.toLowerCase() !== account.address.toLowerCase()) throw new WalletRefusal(`refused: asked to act for ${from}`);
  };

  const wallet: TestnetWallet = {
    address: account.address,
    chainId: () => current,
    sent: [],
    async request({ method, params = [] }) {
      switch (method) {
        case "eth_accounts":
        case "eth_requestAccounts":
          return [account.address];
        case "eth_chainId":
          return `0x${current.toString(16)}`;
        case "net_version":
          return String(current);
        case "wallet_switchEthereumChain": {
          const [{ chainId }] = params as [{ chainId: string }];
          current = testnet(Number.parseInt(chainId, 16)).id;
          wallet.onChainChanged?.(current);
          return null;
        }
        case "wallet_addEthereumChain":
        case "wallet_watchAsset":
          return null;
        case "wallet_requestPermissions":
        case "wallet_getPermissions":
          return [{ parentCapability: "eth_accounts" }];
        case "personal_sign": {
          const [message, from] = params as [Hex, string];
          own(from);
          return account.signMessage({ message: { raw: message } });
        }
        case "eth_signTypedData_v4": {
          const [from, json] = params as [string, string];
          own(from);
          const data = JSON.parse(json) as { domain: { chainId?: unknown; salt?: unknown }; types: Record<string, unknown>; primaryType: string; message: Record<string, unknown> };
          domainChain(data.domain);
          const { EIP712Domain: _domain, ...types } = data.types;
          return account.signTypedData({ domain: data.domain, types, primaryType: data.primaryType, message: data.message } as never);
        }
        case "eth_sendTransaction": {
          const [tx] = params as [{ from?: string; to?: Hex; data?: Hex; value?: Hex; gas?: Hex; chainId?: Hex }];
          own(tx.from);
          if (allowedTo && !(tx.to && allowedTo.has(tx.to.toLowerCase()))) throw new WalletRefusal(`refused: a transaction to ${tx.to ?? "no contract"}, which is not one the journey trades with`);
          if (tx.chainId != null && Number(tx.chainId) !== current) {
            testnet(Number(tx.chainId));
            throw new WalletRefusal(`refused: a transaction for chain ${Number(tx.chainId)} while the wallet is on ${current}`);
          }
          const { wallet: w } = client(current);
          const hash = await w.sendTransaction({
            account,
            chain: testnet(current),
            to: tx.to,
            data: tx.data,
            value: tx.value ? BigInt(tx.value) : undefined,
            gas: tx.gas ? BigInt(tx.gas) : undefined,
          });
          wallet.sent.push(hash);
          return hash;
        }
        case "eth_sign":
        case "eth_signTransaction":
        case "eth_sendRawTransaction":
          throw new WalletRefusal(`refused: ${method} is not something the Shop should ask for`, 4200);
        default:
          return client(current).pub.request({ method, params } as never);
      }
    },
  };
  return wallet;
}

/**
 * The page side: a `window.ethereum` and an EIP-6963 announcement that forward to `__e2eWallet`.
 * Runs before any page script (Playwright addInitScript), so the Shop finds it like a real extension.
 */
export const PAGE_PROVIDER = String.raw`(() => {
  const listeners = new Map();
  const emit = (event, value) => (listeners.get(event) || []).forEach((fn) => { try { fn(value); } catch {} });
  const provider = {
    isMetaMask: false,
    isE2eWallet: true,
    request: async ({ method, params }) => {
      const answer = await window.__e2eWallet({ method, params: params ?? [] });
      if (answer && answer.error) {
        const error = new Error(answer.error.message);
        error.code = answer.error.code;
        throw error;
      }
      return answer.result;
    },
    on: (event, fn) => { listeners.set(event, [...(listeners.get(event) || []), fn]); return provider; },
    removeListener: (event, fn) => { listeners.set(event, (listeners.get(event) || []).filter((f) => f !== fn)); return provider; },
  };
  window.__e2eWalletEmit = emit;
  Object.defineProperty(window, "ethereum", { value: provider, configurable: true });
  const info = { uuid: "6f1f2c1e-0000-4000-8000-00000000e2e0", name: "E2E testnet wallet", icon: "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg'/%3E", rdns: "org.decentraland.e2e" };
  const announce = () => window.dispatchEvent(new CustomEvent("eip6963:announceProvider", { detail: Object.freeze({ info, provider }) }));
  window.addEventListener("eip6963:requestProvider", announce);
  announce();
})();`;

/** The binding's answer: a result, or an error the page rethrows with its EIP-1193 code. */
export async function answer(wallet: TestnetWallet, req: Request): Promise<{ result?: unknown; error?: { code: number; message: string } }> {
  try {
    return { result: await wallet.request(req) };
  } catch (e) {
    const err = e as { code?: number; shortMessage?: string; message?: string };
    return { error: { code: typeof err.code === "number" ? err.code : -32603, message: err.shortMessage ?? err.message ?? String(e) } };
  }
}

/** Reads a 0x-prefixed 32-byte key from an environment variable, without ever printing it. */
export function keyFrom(env: NodeJS.ProcessEnv, name: string): Hex {
  const value = env[name]?.trim();
  if (!value) throw new Error(`${name} is not set: export it, or put it in the file E2E_ENV_FILE names (see README)`);
  const key = (value.startsWith("0x") ? value : `0x${value}`) as Hex;
  if (!/^0x[0-9a-fA-F]{64}$/.test(key)) throw new Error(`${name} is not a 32-byte hex private key`);
  return key;
}
