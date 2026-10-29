// In-memory stand-ins for the Shop's three outside worlds (the /shop-api node, the chain's RPC and the
// buyer's wallet), shared by this area's tests and stories so both drive the real rails and pages. Built
// on the foundation's table and fixture offers: requests are matched to ENDPOINTS by method and path
// (canonical or legacy alias), and trades are shaped for payment/fixtures' BOOK assets.

import { decodeFunctionData, encodeFunctionResult, type Hex } from "viem";

import type { Eip1193Provider } from "../../data/auth/wallet";
import { ENDPOINTS, SHOP_API_PREFIX, type EndpointName } from "../api/endpoints";
import { createShopClient, type ShopClient } from "../api/shop";
import type { ShopTransport, TransportRequest } from "../api/signer";
import { CHAIN } from "../model/chain";
import { BOOK, ITEM, TOKEN } from "../payment/fixtures";
import { AGGREGATOR_ABI, ERC20_ABI, ERC721_ABI, MARKETPLACE_ABI, NATIVE_META_TX_ABI, OFFCHAIN_META_TX_ABI } from "./chain/abi";
import type { ChainReader, TxReceipt } from "./chain/rpc";
import { ASSET_TYPE, type SignedTrade } from "./purchase/encoding";

export type FakeRequest = {
  name: EndpointName;
  params: Record<string, string>;
  query: URLSearchParams;
  body: unknown;
  req: TransportRequest;
};

export type FakeAnswer = { status?: number; body?: unknown };

export type FakeHandlers = Partial<Record<EndpointName, (r: FakeRequest) => FakeAnswer | unknown | Promise<FakeAnswer | unknown>>>;

function matcher(template: string): { re: RegExp; names: string[] } {
  const names: string[] = [];
  const re = new RegExp(
    `^${template
      .replace(/[.*+?^$()|[\]\\]/g, "\\$&")
      .replace(/\{([A-Za-z]+)\}/g, (_, n: string) => {
        names.push(n);
        return "([^/]+)";
      })}$`,
  );
  return { re, names };
}

const TABLE = Object.entries(ENDPOINTS).flatMap(([name, e]) => {
  const paths = [SHOP_API_PREFIX + e.path, ...(e.legacy ? [e.legacy] : [])];
  return paths.map((p) => ({ name: name as EndpointName, method: e.method, ...matcher(p) }));
});

/** Which endpoint a request is for (canonical path or legacy alias). */
export function endpointFor(method: string, url: string): { name: EndpointName; params: Record<string, string> } | null {
  const path = new URL(url, "http://shop.invalid").pathname;
  const candidates = TABLE.filter((t) => t.method === method);
  const exact = candidates
    .map((t) => ({ t, m: t.re.exec(path) }))
    .filter((x): x is { t: (typeof TABLE)[number]; m: RegExpExecArray } => !!x.m)
    .sort((a, b) => a.t.names.length - b.t.names.length)[0];
  if (!exact) return null;
  const params: Record<string, string> = {};
  exact.t.names.forEach((n, i) => (params[n] = decodeURIComponent(exact.m[i + 1]!)));
  return { name: exact.t.name, params };
}

export type FakeShop = { client: ShopClient; transport: ShopTransport; log: FakeRequest[] };

/** A ShopClient whose requests are answered by `handlers`; anything unhandled answers 501. */
export function createFakeShop(handlers: FakeHandlers, opts: { signed?: boolean } = {}): FakeShop {
  const log: FakeRequest[] = [];
  const transport: ShopTransport = async (req) => {
    const hit = endpointFor(req.method, req.url);
    if (!hit) return { status: 404, body: JSON.stringify({ error: "not a Shop route" }) };
    let body: unknown = null;
    try {
      body = req.body ? JSON.parse(req.body) : null;
    } catch {
      body = req.body;
    }
    const request: FakeRequest = { name: hit.name, params: hit.params, query: new URL(req.url, "http://shop.invalid").searchParams, body, req };
    log.push(request);
    const handler = handlers[hit.name];
    if (!handler) return { status: 501, body: JSON.stringify({ error: `no fake for ${hit.name}` }) };
    const out = await handler(request);
    const isAnswer = !!out && typeof out === "object" && typeof (out as FakeAnswer).status === "number" && Object.keys(out).every((k) => k === "status" || k === "body");
    const answer = isAnswer ? (out as FakeAnswer) : { status: 200, body: out };
    return { status: answer.status ?? 200, body: answer.body === undefined ? "" : JSON.stringify(answer.body) };
  };
  const signer = opts.signed === false ? null : { address: BOOK.buyer, sign: async () => ({ "x-identity-auth-chain-0": "fake" }) };
  const client = createShopClient({ base: "https://node.test", transport, signer });
  return { client, transport, log };
}

/** A signed creator order for BOOK's collection item on the Polygon marketplace: $1 (USD-pegged). */
export function creatorTrade(overrides: Partial<SignedTrade> = {}): SignedTrade {
  return {
    id: "trade-creator-1",
    chainId: CHAIN.POLYGON,
    contract: BOOK.marketplace.contract,
    signer: BOOK.creator,
    signature: `0x${"aa".repeat(65)}`,
    network: "MATIC",
    type: "public_item_order",
    checks: {
      uses: 10,
      expiration: Date.now() + 86_400_000,
      effective: Date.now() - 1_000,
      salt: "0x01",
      contractSignatureIndex: 0,
      signerSignatureIndex: 0,
      allowedRoot: "0x",
      externalChecks: [],
    },
    sent: [{ assetType: ASSET_TYPE.COLLECTION_ITEM, contractAddress: BOOK.collection, itemId: String(ITEM), extra: "0x" }],
    received: [{ assetType: ASSET_TYPE.USD_PEGGED_MANA, contractAddress: BOOK.mana, amount: (10n ** 18n).toString(), extra: "0x", beneficiary: BOOK.creator }],
    ...overrides,
  };
}

/** A resale of BOOK's minted token on the Polygon marketplace: $1 (USD-pegged), MANA only. */
export function resaleTrade(overrides: Partial<SignedTrade> = {}): SignedTrade {
  return creatorTrade({
    id: "trade-resale-1",
    signer: BOOK.reseller,
    type: "public_nft_order",
    sent: [{ assetType: ASSET_TYPE.ERC721, contractAddress: BOOK.collection, tokenId: String(TOKEN), extra: "0x" }],
    received: [{ assetType: ASSET_TYPE.USD_PEGGED_MANA, contractAddress: BOOK.mana, amount: (10n ** 18n).toString(), extra: "0x", beneficiary: BOOK.reseller }],
    ...overrides,
  });
}

export type FakeChain = ChainReader & {
  /** Receipts by hash; a hash with no entry has none (pending). */
  receipts: Map<string, TxReceipt["status"]>;
  nonces: Map<string, bigint>;
  allowances: Map<string, bigint>;
  /** `${chainId}:${collection}:${owner}:${operator}` -> approved. */
  approvals: Map<string, boolean>;
  manaBalances: Map<string, bigint>;
  calls: { chainId: number; to: string; data: Hex }[];
};

/** A chain that answers the reads the rails make: getNonce, allowance, balanceOf, the MANA oracle. */
export function createFakeChain(opts: { rate?: bigint; decimals?: number } = {}): FakeChain {
  const receipts = new Map<string, TxReceipt["status"]>();
  const nonces = new Map<string, bigint>();
  const allowances = new Map<string, bigint>();
  const manaBalances = new Map<string, bigint>();
  const approvals = new Map<string, boolean>();
  const calls: FakeChain["calls"] = [];
  const nowSeconds = () => BigInt(Math.floor(Date.now() / 1000));
  return {
    receipts,
    nonces,
    allowances,
    approvals,
    manaBalances,
    calls,
    async call(chainId, to, data) {
      calls.push({ chainId, to, data });
      const selector = data.slice(0, 10);
      if (selector === "0x2d0335ab") {
        const { args } = decodeFunctionData({ abi: OFFCHAIN_META_TX_ABI, data });
        return encodeFunctionResult({ abi: OFFCHAIN_META_TX_ABI, functionName: "getNonce", result: nonces.get(`${to.toLowerCase()}:${String(args[0]).toLowerCase()}`) ?? 0n });
      }
      if (selector === "0xdd62ed3e") {
        const { args } = decodeFunctionData({ abi: ERC20_ABI, data });
        const key = `${chainId}:${String(args[0]).toLowerCase()}:${String(args[1]).toLowerCase()}`;
        return encodeFunctionResult({ abi: ERC20_ABI, functionName: "allowance", result: allowances.get(key) ?? 0n });
      }
      if (selector === "0xe985e9c5") {
        const { args } = decodeFunctionData({ abi: ERC721_ABI, data });
        const key = `${chainId}:${to.toLowerCase()}:${String(args[0]).toLowerCase()}:${String(args[1]).toLowerCase()}`;
        return encodeFunctionResult({ abi: ERC721_ABI, functionName: "isApprovedForAll", result: approvals.get(key) ?? false });
      }
      if (selector === "0x70a08231") {
        const { args } = decodeFunctionData({ abi: ERC20_ABI, data });
        return encodeFunctionResult({ abi: ERC20_ABI, functionName: "balanceOf", result: manaBalances.get(`${chainId}:${String(args[0]).toLowerCase()}`) ?? 0n });
      }
      if (selector === "0x05dec3e4") {
        return encodeFunctionResult({ abi: MARKETPLACE_ABI, functionName: "manaUsdAggregator", result: "0x00000000000000000000000000000000000a99e9" });
      }
      if (selector === "0x313ce567") return encodeFunctionResult({ abi: AGGREGATOR_ABI, functionName: "decimals", result: opts.decimals ?? 8 });
      if (selector === "0xfeaf968c") {
        return encodeFunctionResult({ abi: AGGREGATOR_ABI, functionName: "latestRoundData", result: [1n, opts.rate ?? 50_000_000n, nowSeconds(), nowSeconds(), 1n] });
      }
      throw new Error(`fake chain: unanswered call ${selector} to ${to}`);
    },
    async receipt(_chainId, txHash) {
      const status = receipts.get(txHash.toLowerCase());
      return status ? { transactionHash: txHash as Hex, status, blockNumber: 1n } : null;
    },
    balance: async () => 10n ** 18n,
    code: async () => "0x",
  };
}

export type FakeWallet = Eip1193Provider & {
  chainId: number;
  account: string;
  requests: { method: string; params?: unknown[] }[];
  /** What the next signature/transaction request does. */
  next: "ok" | "reject" | "fail";
  sent: { to: string; data: Hex }[];
  emit(event: string, ...args: unknown[]): void;
};

/** A wallet that signs and sends; `next` makes the following prompt be declined or fail. */
export function createFakeWallet(opts: { chainId?: number; account?: string; onSend?: (tx: { to: string; data: Hex }) => Hex } = {}): FakeWallet {
  const handlers = new Map<string, Set<(...a: unknown[]) => void>>();
  let n = 0;
  const wallet: FakeWallet = {
    chainId: opts.chainId ?? CHAIN.POLYGON,
    account: opts.account ?? BOOK.buyer,
    requests: [],
    next: "ok",
    sent: [],
    async request({ method, params }) {
      wallet.requests.push({ method, params });
      if (method === "eth_chainId") return `0x${wallet.chainId.toString(16)}`;
      if (method === "eth_accounts" || method === "eth_requestAccounts") return [wallet.account];
      if (method === "wallet_switchEthereumChain") {
        wallet.chainId = Number.parseInt((params![0] as { chainId: string }).chainId, 16);
        for (const h of handlers.get("chainChanged") ?? []) h(`0x${wallet.chainId.toString(16)}`);
        return null;
      }
      if (method === "eth_signTypedData_v4" || method === "eth_sendTransaction") {
        const mode = wallet.next;
        wallet.next = "ok";
        if (mode === "reject") throw Object.assign(new Error("User rejected the request."), { code: 4001 });
        if (mode === "fail") throw new Error("The wallet could not sign");
        if (method === "eth_signTypedData_v4") return `0x${"11".repeat(64)}1b`;
        const tx = (params![0] as { to: string; data: Hex });
        wallet.sent.push({ to: tx.to, data: tx.data });
        return opts.onSend?.(tx) ?? (`0x${(++n).toString(16).padStart(64, "0")}` as Hex);
      }
      throw Object.assign(new Error(`unsupported ${method}`), { code: 4200 });
    },
    on(event, handler) {
      if (!handlers.has(event)) handlers.set(event, new Set());
      handlers.get(event)!.add(handler);
    },
    removeListener(event, handler) {
      handlers.get(event)?.delete(handler);
    },
    emit(event, ...args) {
      if (event === "accountsChanged") wallet.account = ((args[0] as string[])[0] ?? "").toLowerCase();
      if (event === "chainChanged") wallet.chainId = Number.parseInt(args[0] as string, 16);
      for (const h of handlers.get(event) ?? []) h(...args);
    },
  };
  return wallet;
}

/** The MANA approve a relayed NativeMetaTransaction carries, if any: (owner, spender, amount). */
export function relayedApproval(body: unknown): { owner: string; spender: string; amount: bigint } | null {
  const data = (body as { transactionData?: { params?: [string, Hex] } })?.transactionData?.params?.[1];
  if (!data || !data.startsWith("0x0c53c51c")) return null;
  const { args } = decodeFunctionData({ abi: NATIVE_META_TX_ABI, data });
  const inner = args[1] as Hex;
  if (!inner.startsWith("0x095ea7b3")) return null;
  const approve = decodeFunctionData({ abi: ERC20_ABI, data: inner });
  return { owner: String(args[0]).toLowerCase(), spender: String(approve.args[0]).toLowerCase(), amount: approve.args[1] as bigint };
}

/**
 * A relay endpoint (POST {base}/transactions): hashes in order, or the refusal/outage asked for. With a
 * `chain`, a relayed MANA approve raises that allowance and every relayed hash gets the receipt `confirm`.
 */
export function createFakeRelay(opts: { mode?: "ok" | "refuse" | "down"; onRelay?: (hash: Hex, body: unknown) => void; chain?: FakeChain; confirm?: TxReceipt["status"] | "none"; chainId?: number } = {}) {
  let n = 0;
  const posts: unknown[] = [];
  const state = { mode: opts.mode ?? "ok", confirm: opts.confirm ?? "success" };
  const fetchImpl = (async (_url: string, init?: RequestInit) => {
    const body = JSON.parse(String(init?.body ?? "null")) as unknown;
    posts.push(body);
    if (state.mode === "down") throw new TypeError("Failed to fetch");
    if (state.mode === "refuse") return new Response(JSON.stringify({ ok: false, message: "execution reverted" }), { status: 400 });
    const hash = `0x${"f".repeat(56)}${(++n).toString(16).padStart(8, "0")}` as Hex;
    if (opts.chain) {
      const approval = relayedApproval(body);
      if (approval) {
        opts.chain.allowances.set(`${opts.chainId ?? CHAIN.POLYGON}:${approval.owner}:${approval.spender}`, approval.amount);
        opts.chain.receipts.set(hash.toLowerCase(), "success");
      } else if (state.confirm !== "none") opts.chain.receipts.set(hash.toLowerCase(), state.confirm);
    }
    opts.onRelay?.(hash, body);
    return new Response(JSON.stringify({ ok: true, txHash: hash }), { status: 200 });
  }) as unknown as typeof fetch;
  return { fetchImpl, posts, state };
}

/** A batch authorization answer shaped like credits-server's (one credit, one salt, per-line charges). */
export function batchAuthorization(lines: { usdPriceCents: number; tradeId?: string; contractAddress?: string; itemId?: string }[], salt = `0x${"a1".repeat(32)}`) {
  const charged = lines.map((l) => Math.ceil(l.usdPriceCents / 10) * 10);
  return {
    credit: {
      id: salt,
      amount: "3000000000000000000",
      availableAmount: "3000000000000000000",
      expiresAt: Math.floor(Date.now() / 1000) + 900,
      signature: `0x${"cc".repeat(65)}`,
      contract: "0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5",
    },
    maxCreditedValue: "3000000000000000000",
    usdCents: charged.reduce((a, b) => a + b, 0),
    oracleRate: "50000000",
    lines: lines.map((l, i) => ({ ...l, usdCents: charged[i] })),
  };
}
