import { decodeFunctionData, encodeFunctionResult, toFunctionSelector, type Abi, type Hex } from "viem";

import type { Eip1193Provider } from "../../data/auth/wallet";
import { createShopClient, type ShopClient } from "../api/shop";
import type { TransportRequest } from "../api/signer";

export type CallAnswer = (args: readonly unknown[], to: string) => unknown;

export type FakeWallet = Eip1193Provider & {
  account: string;
  chainId: number;
  sent: { to: string; data: Hex; fn: string; args: readonly unknown[] }[];
  signed: unknown[];
  calls: string[];
  receipts: Map<string, 0 | 1 | undefined>;
  reject: Set<string>;
  emit(event: string, ...args: unknown[]): void;
};

type AbiFn = { name: string; abi: Abi; answer: CallAnswer };

export function pollCountedWait(timeoutMs: number): { pollMs: number; timeoutMs: number; now: () => number } {
  let reads = 0;
  return { pollMs: 0, timeoutMs, now: () => reads++ };
}

export function lowerAddresses(value: unknown): unknown {
  if (typeof value === "string" && /^0x[0-9a-fA-F]{40}$/.test(value)) return value.toLowerCase();
  if (Array.isArray(value)) return value.map(lowerAddresses);
  if (value && typeof value === "object") return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, lowerAddresses(v)]));
  return value;
}

function selectorOf(abi: Abi, name: string): string {
  const fn = abi.find((f) => f.type === "function" && f.name === name);
  if (!fn || fn.type !== "function") throw new Error(`no ${name} in abi`);
  return toFunctionSelector(fn);
}

export function fakeWallet(opts: {
  account: string;
  chainId: number;
  reads?: { abi: Abi; name: string; answer: CallAnswer }[];
  writes?: Abi[];
  autoMine?: boolean;
  signature?: string;
}): FakeWallet {
  const reads = new Map<string, AbiFn>();
  for (const r of opts.reads ?? []) reads.set(selectorOf(r.abi, r.name), { name: r.name, abi: r.abi, answer: r.answer });
  const writeAbis = [...(opts.writes ?? []), ...(opts.reads ?? []).map((r) => r.abi)];
  const handlers = new Map<string, Set<(...a: unknown[]) => void>>();
  let n = 0;
  const wallet: FakeWallet = {
    account: opts.account.toLowerCase(),
    chainId: opts.chainId,
    sent: [],
    signed: [],
    calls: [],
    receipts: new Map(),
    reject: new Set(),
    async request({ method, params }) {
      wallet.calls.push(method);
      if (wallet.reject.has(method)) throw Object.assign(new Error("User rejected the request."), { code: 4001 });
      switch (method) {
        case "eth_chainId":
          return `0x${wallet.chainId.toString(16)}`;
        case "eth_accounts":
        case "eth_requestAccounts":
          return [wallet.account];
        case "wallet_switchEthereumChain":
          wallet.chainId = Number.parseInt((params![0] as { chainId: string }).chainId, 16);
          return null;
        case "eth_call": {
          const { to, data } = params![0] as { to: string; data: Hex };
          const read = reads.get(data.slice(0, 10));
          if (!read) throw new Error(`unscripted eth_call ${data.slice(0, 10)} to ${to}`);
          const { args } = decodeFunctionData({ abi: read.abi, data });
          const value = read.answer(lowerAddresses(args ?? []) as readonly unknown[], to.toLowerCase());
          return encodeFunctionResult({ abi: read.abi, functionName: read.name, result: value } as never);
        }
        case "eth_sendTransaction": {
          const tx = params![0] as { to: string; data: Hex };
          let fn = "unknown";
          let args: readonly unknown[] = [];
          for (const abi of writeAbis) {
            try {
              const decoded = decodeFunctionData({ abi, data: tx.data });
              fn = decoded.functionName;
              args = lowerAddresses(decoded.args ?? []) as readonly unknown[];
              break;
            } catch {
            }
          }
          wallet.sent.push({ to: tx.to.toLowerCase(), data: tx.data, fn, args });
          const hash = `0x${(++n).toString(16).padStart(64, "0")}`;
          wallet.receipts.set(hash, opts.autoMine === false ? undefined : 1);
          return hash;
        }
        case "eth_getTransactionReceipt": {
          const status = wallet.receipts.get(params![0] as string);
          return status === undefined ? null : { status: `0x${status}` };
        }
        case "eth_signTypedData_v4":
          wallet.signed.push(JSON.parse(params![1] as string));
          return opts.signature ?? `0x${"11".repeat(64)}1b`;
        default:
          throw Object.assign(new Error(`unsupported ${method}`), { code: 4200 });
      }
    },
    on(event, handler) {
      if (!handlers.has(event)) handlers.set(event, new Set());
      handlers.get(event)!.add(handler);
    },
    removeListener(event, handler) {
      handlers.get(event)?.delete(handler);
    },
    emit(event, ...args) {
      if (event === "accountsChanged") wallet.account = String((args[0] as string[])[0] ?? "").toLowerCase();
      if (event === "chainChanged") wallet.chainId = Number.parseInt(args[0] as string, 16);
      for (const h of handlers.get(event) ?? []) h(...args);
    },
  };
  return wallet;
}

export type Answer = "pending" | { status?: number; body: unknown } | ((req: TransportRequest, url: URL) => { status?: number; body: unknown });

export function stubClient(routes: Record<string, Answer>, opts: { signed?: boolean } = {}): ShopClient & { requests: TransportRequest[] } {
  const requests: TransportRequest[] = [];
  const client = createShopClient({
    base: "https://node.test",
    mode: "canonical",
    signer: opts.signed === false ? null : { address: "0x00000000000000000000000000000000000000b0", sign: async () => ({ "x-identity-auth-chain-0": "stub" }) },
    transport: async (req) => {
      requests.push(req);
      const url = new URL(req.url);
      const answer = routes[`${req.method} ${url.pathname}`];
      if (!answer) return { status: 501, body: JSON.stringify({ error: "unscripted" }) };
      if (answer === "pending") return new Promise(() => {});
      const out = typeof answer === "function" ? answer(req, url) : answer;
      return { status: out.status ?? 200, body: JSON.stringify(out.body) };
    },
  });
  return Object.assign(client, { requests });
}

export function fakeReader(wallet: FakeWallet) {
  return {
    chainId: wallet.chainId,
    async call(to: string, data: Hex) {
      return (await wallet.request({ method: "eth_call", params: [{ to, data }, "latest"] })) as Hex;
    },
  };
}
