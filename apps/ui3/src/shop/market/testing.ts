// Test doubles for the seller and marketplace flows: an EIP-1193 wallet backed by a real private key (so
// typed-data signatures recover to its address, as a wallet's would), recording every transaction it is
// asked to send, and an in-memory JSON-RPC answering the reads and receipts those flows make.

import { decodeFunctionData, encodeFunctionResult, type Abi, type Hex } from "viem";
import { privateKeyToAccount } from "viem/accounts";

import type { Eip1193Provider } from "../../data/auth/wallet";
import { createShopClient, type ShopClient } from "../api/shop";
import type { ChainId } from "../model/chain";
import { setRpcTransport } from "./lib/rpc";

export const TEST_KEY: Hex = "0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318";

export type SentTx = { from: string; to: string; data: Hex; value?: string };

export type FakeWallet = Eip1193Provider & {
  address: string;
  chainId: number;
  sent: SentTx[];
  signed: unknown[];
  /** Make the next wallet request of `method` fail with this error. */
  failNext(method: string, error: unknown): void;
};

export function fakeWallet(opts: { key?: Hex; chainId?: number } = {}): FakeWallet {
  const account = privateKeyToAccount(opts.key ?? TEST_KEY);
  const failures = new Map<string, unknown>();
  let nonce = 0;
  const wallet: FakeWallet = {
    address: account.address.toLowerCase(),
    chainId: opts.chainId ?? 137,
    sent: [],
    signed: [],
    failNext(method, error) {
      failures.set(method, error);
    },
    async request({ method, params }) {
      const failure = failures.get(method);
      if (failure) {
        failures.delete(method);
        throw failure;
      }
      switch (method) {
        case "eth_accounts":
        case "eth_requestAccounts":
          return [wallet.address];
        case "eth_chainId":
          return `0x${wallet.chainId.toString(16)}`;
        case "eth_signTypedData_v4": {
          const [, json] = params as [string, string];
          const data = JSON.parse(json) as { domain: Record<string, unknown>; types: Record<string, unknown>; primaryType: string; message: Record<string, unknown> };
          const { EIP712Domain: _domain, ...types } = data.types;
          wallet.signed.push(data);
          return account.signTypedData({ domain: data.domain, types, primaryType: data.primaryType, message: data.message } as never);
        }
        case "eth_sendTransaction": {
          const [tx] = params as [SentTx];
          wallet.sent.push(tx);
          nonce += 1;
          return `0x${nonce.toString(16).padStart(64, "0")}`;
        }
        case "wallet_switchEthereumChain": {
          const [{ chainId }] = params as [{ chainId: string }];
          wallet.chainId = Number.parseInt(chainId, 16);
          return null;
        }
        default:
          throw Object.assign(new Error(`unsupported ${method}`), { code: 4200 });
      }
    },
  };
  return wallet;
}

export type ContractStub = { abi: Abi; answer: (functionName: string, args: readonly unknown[]) => unknown };

/**
 * Installs an in-memory JSON-RPC: `eth_call` is decoded against the stubbed contract's ABI and answered
 * by `answer`; receipts are successful unless listed in `reverted` or `pending`. Returns the restore.
 */
export function fakeChain(opts: {
  contracts?: Record<string, ContractStub>;
  reverted?: Set<string>;
  pending?: Set<string>;
  calls?: { chainId: ChainId; method: string; params: unknown[] }[];
}): () => void {
  const contracts = Object.fromEntries(Object.entries(opts.contracts ?? {}).map(([k, v]) => [k.toLowerCase(), v]));
  return setRpcTransport(async (chainId, method, params) => {
    opts.calls?.push({ chainId, method, params });
    if (method === "eth_call") {
      const [{ to, data }] = params as [{ to: string; data: Hex }];
      const stub = contracts[to.toLowerCase()];
      if (!stub) throw new Error(`no stub for ${to}`);
      const decoded = decodeFunctionData({ abi: stub.abi, data });
      const result = stub.answer(decoded.functionName, decoded.args ?? []);
      return encodeFunctionResult({ abi: stub.abi, functionName: decoded.functionName, result } as never);
    }
    if (method === "eth_getTransactionReceipt") {
      const [hash] = params as [string];
      if (opts.pending?.has(hash)) return null;
      return { status: opts.reverted?.has(hash) ? "0x0" : "0x1", transactionHash: hash };
    }
    throw new Error(`unsupported ${method}`);
  });
}

export const noWait = { sleep: async () => {}, pollMs: 0, timeoutMs: 0 };

export type FakeReply = { status?: number; body: unknown };

/** A reply that never comes: the page's loading state stays on screen. */
export const PENDING = (): Promise<FakeReply> => new Promise<FakeReply>(() => {});

export type FakeRoute = {
  method: string;
  /** Matched against the path under /shop-api (canonical mode). */
  path: RegExp;
  reply: (req: { url: URL; body: unknown; headers: Record<string, string> }) => FakeReply | Promise<FakeReply>;
};

export type FakeShop = { client: ShopClient; requests: { method: string; url: URL; body: unknown; signed: boolean }[] };

/** A ShopClient over an in-memory transport: unmatched routes answer 501 (never an invented body). */
export function fakeShop(routes: FakeRoute[], opts: { signer?: string | null } = {}): FakeShop {
  const requests: FakeShop["requests"] = [];
  const signerAddress = opts.signer === undefined ? "0x0000000000000000000000000000000000000b0b" : opts.signer;
  const client = createShopClient({
    base: "https://node.test",
    mode: "canonical",
    signer: signerAddress ? { address: signerAddress, sign: async () => ({ "x-test-signed": "1" }) } : null,
    transport: async (req) => {
      const url = new URL(req.url);
      const body = req.body ? (JSON.parse(req.body) as unknown) : undefined;
      requests.push({ method: req.method, url, body, signed: req.signed });
      const path = url.pathname.replace(/^\/shop-api/, "");
      const route = routes.find((r) => r.method === req.method && r.path.test(path));
      if (!route) return { status: 501, body: JSON.stringify({ error: `no fake for ${req.method} ${path}` }) };
      const reply = await route.reply({ url, body, headers: req.headers });
      return { status: reply.status ?? 200, body: JSON.stringify(reply.body) };
    },
  });
  return { client, requests };
}
