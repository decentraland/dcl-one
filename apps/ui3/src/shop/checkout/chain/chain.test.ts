import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  decodeAbiParameters,
  decodeFunctionData,
  domainSeparator,
  encodeAbiParameters,
  encodeFunctionData,
  getAbiItem,
  hashTypedData,
  recoverAddress,
  toFunctionSelector,
  type Hex,
} from "viem";
import { afterEach, describe, expect, it, vi } from "vitest";

import { CHAIN } from "@shop/model/chain";
import { COLLECTION_STORE_ABI, CREDITS_MANAGER_ABI, MARKETPLACE_ABI, NATIVE_META_TX_ABI, OFFCHAIN_META_TX_ABI } from "@shop/checkout/chain/abi";
import { chainContracts, contractAt, replacedCreditsManager, requireContracts } from "./contracts";
import {
  chainIdSalt,
  encodeExecuteMetaTransaction,
  GaslessUnavailableError,
  metaTxType,
  NonceMovedError,
  normalizeSignature,
  refusedBeforeBroadcast,
  relayMetaTransaction,
  relayRefusal,
  relaySignedMetaTransaction,
  SignatureDeclinedError,
} from "./metatx";
import { createRpcReader, waitForReceipt, type ChainReader } from "./rpc";
import { sendAndConfirm, TxPendingError, TxRevertedError } from "./wallet";

const FIXTURES = join(__dirname, "../../../../../../services/catalyst/tests/fixtures/shop");
const read = (p: string) => readFileSync(join(FIXTURES, p), "utf8");

const selectors = JSON.parse(read("offchain-marketplace-contract@87350c63e/derived/selectors.json")) as Record<
  string,
  { functions: Record<string, string> }
>;
const chainReads = JSON.parse(read("chain-reads.json")) as { selectors: Record<string, string> };

type Sample = { label: string; address: string; userAddress: string; nonce: bigint; callData: Hex };

const domains = JSON.parse(read("transactions-server@deae074c8/derived/polygon-domains.json")) as {
  contracts: { label: string; domainSeparator: string | null }[];
  executedMetaTransactions: { label: string; address: string; userAddress: string; nonce: string; callData: Hex }[];
};

function executedSamples(): Sample[] {
  return domains.executedMetaTransactions.map((s) => ({ label: s.label, address: s.address, userAddress: s.userAddress, nonce: BigInt(s.nonce), callData: s.callData }));
}

function recordedSeparator(label: string): Hex {
  const separator = domains.contracts.find((c) => c.label === label)?.domainSeparator;
  if (!separator) throw new Error(`no recorded separator for ${label}`);
  return separator as Hex;
}

describe("the ABIs are the deployed contracts' (pinned selectors)", () => {
  it("every function the rails call has the selector the contract source compiles to", () => {
    const market = selectors.DecentralandMarketplacePolygon!.functions;
    const credits = selectors.CreditsManagerPolygon!.functions;
    const store = selectors.ICollectionStore!.functions;
    const sel = (abi: readonly unknown[], name: string) => toFunctionSelector(getAbiItem({ abi: abi as never, name } as never) as never);
    expect(sel(MARKETPLACE_ABI, "accept")).toBe(market[Object.keys(market).find((k) => k.startsWith("accept("))!]);
    expect(sel(MARKETPLACE_ABI, "acceptWithCoupon")).toBe(market[Object.keys(market).find((k) => k.startsWith("acceptWithCoupon("))!]);
    expect(sel(MARKETPLACE_ABI, "cancelSignature")).toBe(market[Object.keys(market).find((k) => k.startsWith("cancelSignature("))!]);
    expect(sel(MARKETPLACE_ABI, "manaUsdAggregator")).toBe(market["manaUsdAggregator()"]);
    expect(sel(CREDITS_MANAGER_ABI, "useCredits")).toBe(credits[Object.keys(credits).find((k) => k.startsWith("useCredits("))!]);
    expect(sel(CREDITS_MANAGER_ABI, "useCredits")).toBe(chainReads.selectors[Object.keys(chainReads.selectors).find((k) => k.startsWith("useCredits("))!]);
    expect(sel(COLLECTION_STORE_ABI, "buy")).toBe(store[Object.keys(store).find((k) => k.startsWith("buy("))!]);
    expect(sel(OFFCHAIN_META_TX_ABI, "executeMetaTransaction")).toBe("0xd8ed1acc");
    expect(sel(NATIVE_META_TX_ABI, "executeMetaTransaction")).toBe("0x0c53c51c");
    expect(sel(OFFCHAIN_META_TX_ABI, "getNonce")).toBe(credits["getNonce(address)"]);
  });
});

describe("the EIP-712 domains are the ones the contracts verify", () => {
  it("rebuilds the recorded domain separators of MANA, the CollectionStore and the legacy order book", () => {
    const polygon = requireContracts(CHAIN.POLYGON);
    for (const [label, info] of [
      ["MANAToken", polygon.mana],
      ["CollectionStore", polygon.collectionStore!],
      ["MarketplaceV2", polygon.legacyMarketplace!],
    ] as const) {
      const separator = domainSeparator({
        domain: { name: info.name, version: info.version, verifyingContract: info.address, salt: chainIdSalt(CHAIN.POLYGON) },
      });
      expect(separator, label).toBe(recordedSeparator(label));
    }
  });

  it("recovers the executing user from every recorded Polygon meta-transaction (CreditsManager, marketplaces)", async () => {
    const samples = executedSamples();
    expect(samples.map((s) => s.label)).toEqual(expect.arrayContaining(["CreditsManager", "MarketplaceV3", "MarketplaceV4"]));
    for (const sample of samples) {
      const info = contractAt(CHAIN.POLYGON, sample.address);
      expect(info, sample.label).not.toBeNull();
      const { args } = decodeFunctionData({ abi: OFFCHAIN_META_TX_ABI, data: sample.callData });
      const [user, functionData, signature] = args as [Hex, Hex, Hex];
      expect(user.toLowerCase()).toBe(sample.userAddress);
      const hash = hashTypedData({
        domain: { name: info!.name, version: info!.version, verifyingContract: info!.address, salt: chainIdSalt(CHAIN.POLYGON) },
        types: { MetaTransaction: metaTxType(info!.metaTx!) as unknown as { name: string; type: string }[] },
        primaryType: "MetaTransaction",
        message: { nonce: sample.nonce, from: user, [info!.metaTx!]: functionData },
      });
      expect((await recoverAddress({ hash, signature })).toLowerCase(), sample.label).toBe(sample.userAddress);
      const ours = decodeFunctionData({ abi: OFFCHAIN_META_TX_ABI, data: encodeExecuteMetaTransaction("functionData", user, functionData, signature) });
      expect(ours.args).toEqual(args);
    }
  });

  it("re-encodes a real relayed useCredits(accept(...)) byte for byte", () => {
    const sample = executedSamples().find((s) => s.label === "CreditsManager")!;
    const { args } = decodeFunctionData({ abi: OFFCHAIN_META_TX_ABI, data: sample.callData });
    const functionData = args[1] as Hex;
    const decoded = decodeFunctionData({ abi: CREDITS_MANAGER_ABI, data: functionData });
    expect(decoded.functionName).toBe("useCredits");
    expect(encodeFunctionData({ abi: CREDITS_MANAGER_ABI, functionName: "useCredits", args: decoded.args as never })).toBe(functionData);
    const call = (decoded.args[0] as { externalCall: { selector: Hex; data: Hex; target: Hex } }).externalCall;
    expect(call.selector).toBe("0x961a547e");
    const accept = getAbiItem({ abi: MARKETPLACE_ABI, name: "accept" });
    const trades = decodeAbiParameters(accept.inputs, call.data);
    expect(encodeAbiParameters(accept.inputs, trades as never)).toBe(call.data);
  });
});

describe("the address book", () => {
  it("names the settlement contracts of each chain, lowercase", () => {
    const polygon = chainContracts(CHAIN.POLYGON)!;
    expect(polygon.creditsManager!.address).toBe("0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5");
    expect(polygon.offChainMarketplaces[0]!.address).toBe("0xe38ef22abe871513555cba89adfe45ab4f548ada");
    expect(chainContracts(CHAIN.ETHEREUM)!.creditsManager).toBeNull();
    expect(chainContracts(5)).toBeNull();
    expect(() => requireContracts(5)).toThrow();
  });

  describe("with the deployment's own CreditsManager configured", () => {
    const OWN = "0xB65FD692D15C30C3ADCACDD5CCF80E1FACECFC7C";
    const REGISTRY_AMOY = "0x8052a560e6e6ac86eeb7e711a4497f639b322fb3";
    afterEach(() => vi.unstubAllEnvs());

    it("names it on the chain the build settles on, with the registry manager's EIP-712 domain", () => {
      vi.stubEnv("VITE_SHOP_CHAIN_ID", String(CHAIN.AMOY));
      vi.stubEnv("VITE_SHOP_CREDITS_MANAGER", ` ${OWN} `);
      expect(requireContracts(CHAIN.AMOY).creditsManager).toMatchObject({
        address: OWN.toLowerCase(),
        chainId: CHAIN.AMOY,
        name: "Decentraland Credits",
        version: "1.0.0",
        metaTx: "functionData",
      });
      expect(contractAt(CHAIN.AMOY, OWN)?.address).toBe(OWN.toLowerCase());
    });

    it("keeps the replaced registry manager listed, for its approvals only", () => {
      vi.stubEnv("VITE_SHOP_CHAIN_ID", String(CHAIN.AMOY));
      vi.stubEnv("VITE_SHOP_CREDITS_MANAGER", OWN);
      expect(replacedCreditsManager(CHAIN.AMOY)).toMatchObject({ address: REGISTRY_AMOY, label: "CreditsManager (previous)" });
      expect(contractAt(CHAIN.AMOY, REGISTRY_AMOY)?.label).toBe("CreditsManager (previous)");
      expect(replacedCreditsManager(CHAIN.POLYGON)).toBeNull();
      vi.unstubAllEnvs();
      expect(replacedCreditsManager(CHAIN.AMOY)).toBeNull();
    });

    it("leaves the other chains' managers alone", () => {
      vi.stubEnv("VITE_SHOP_CHAIN_ID", String(CHAIN.AMOY));
      vi.stubEnv("VITE_SHOP_CREDITS_MANAGER", OWN);
      expect(requireContracts(CHAIN.POLYGON).creditsManager!.address).toBe("0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5");
      expect(requireContracts(CHAIN.ETHEREUM).creditsManager).toBeNull();
    });

    it("applies only to a chain the build names explicitly, never to one picked at runtime", () => {
      vi.stubEnv("VITE_SHOP_CREDITS_MANAGER", OWN);
      vi.stubEnv("VITE_SHOP_CHAIN_ID", "");
      expect(requireContracts(CHAIN.POLYGON).creditsManager!.address).toBe("0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5");
      expect(requireContracts(CHAIN.AMOY).creditsManager!.address).toBe(REGISTRY_AMOY);
      vi.stubEnv("VITE_SHOP_CHAIN_ID", String(CHAIN.AMOY));
      const win = window as Window & { __SHOP_CHAIN__?: { settlementChainId?: number } };
      win.__SHOP_CHAIN__ = { settlementChainId: CHAIN.POLYGON };
      try {
        expect(requireContracts(CHAIN.POLYGON).creditsManager!.address).toBe("0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5");
      } finally {
        delete win.__SHOP_CHAIN__;
      }
    });

    it("keeps the registry manager when it is unset or not an address", () => {
      vi.stubEnv("VITE_SHOP_CHAIN_ID", String(CHAIN.AMOY));
      vi.stubEnv("VITE_SHOP_CREDITS_MANAGER", "");
      expect(requireContracts(CHAIN.AMOY).creditsManager!.address).toBe(REGISTRY_AMOY);
      vi.stubEnv("VITE_SHOP_CREDITS_MANAGER", "0xnot-an-address");
      expect(requireContracts(CHAIN.AMOY).creditsManager!.address).toBe(REGISTRY_AMOY);
    });
  });
});

describe("signatures", () => {
  it("normalises a 0/1 recovery id to 27/28 and leaves contract signatures alone", () => {
    const rs = "11".repeat(64);
    expect(normalizeSignature(`0x${rs}00` as Hex)).toBe(`0x${rs}1b`);
    expect(normalizeSignature(`0x${rs}01` as Hex)).toBe(`0x${rs}1c`);
    expect(normalizeSignature(`0x${rs}1c` as Hex)).toBe(`0x${rs}1c`);
    expect(normalizeSignature("0xabcdef")).toBe("0xabcdef");
  });
});

function fakeReader(over: Partial<ChainReader> = {}): ChainReader {
  return {
    call: async () => `0x${"0".repeat(63)}7` as Hex,
    receipt: async () => null,
    balance: async () => 0n,
    code: async () => "0x",
    ...over,
  };
}

describe("relaying a meta-transaction", () => {
  const target = { ...requireContracts(CHAIN.POLYGON).creditsManager!, metaTx: "functionData" as const };
  const from = "0x3f6b1d01b6823ab235fc343069b62b6472774cd1";

  it("signs against the contract's nonce and posts the transactions-server shape", async () => {
    const signed: unknown[] = [];
    const provider = {
      request: async ({ method, params }: { method: string; params?: unknown[] }) => {
        signed.push({ method, params });
        return `0x${"22".repeat(64)}01`;
      },
    };
    const posts: { url: string; body: unknown }[] = [];
    const fetchImpl = vi.fn(async (url: string, init?: RequestInit) => {
      posts.push({ url, body: JSON.parse(String(init?.body)) });
      return new Response(JSON.stringify({ ok: true, txHash: `0x${"ab".repeat(32)}` }), { status: 200 });
    }) as unknown as typeof fetch;
    const onSigned = vi.fn();
    const out = await relayMetaTransaction({
      provider,
      reader: fakeReader(),
      target,
      from,
      functionData: "0x1234",
      relayerUrl: "https://node.example/transactions-api/v1",
      fetchImpl,
      onSigned,
    });
    expect(out).toEqual({ txHash: `0x${"ab".repeat(32)}`, nonce: 7n });
    expect(onSigned).toHaveBeenCalledOnce();
    const typed = JSON.parse((signed[0] as { params: string[] }).params[1]!);
    expect(typed.domain).toEqual({ name: "Decentraland Credits", version: "1.0.0", verifyingContract: target.address, salt: chainIdSalt(137) });
    expect(typed.message).toEqual({ nonce: "7", from, functionData: "0x1234" });
    expect(posts[0]!.url).toBe("https://node.example/transactions-api/v1/transactions");
    const body = posts[0]!.body as { transactionData: { from: string; params: [string, Hex] } };
    expect(body.transactionData.params[0]).toBe(target.address);
    const decoded = decodeFunctionData({ abi: OFFCHAIN_META_TX_ABI, data: body.transactionData.params[1] });
    expect((decoded.args[2] as Hex).slice(-2)).toBe("1c");
  });

  it("sends an execution already signed as the relay request it was, and tells a refusal from a lost answer", async () => {
    const posts: unknown[] = [];
    const answer = { body: { ok: true, txHash: `0x${"ab".repeat(32)}` } as unknown, status: 200 };
    const fetchImpl = (async (url: string, init?: RequestInit) => {
      posts.push({ url, body: JSON.parse(String(init?.body)) });
      return new Response(JSON.stringify(answer.body), { status: answer.status });
    }) as unknown as typeof fetch;
    const data = encodeExecuteMetaTransaction("functionData", from, "0x1234", `0x${"22".repeat(64)}1c`);
    const sent = { from, to: target.address, data, relayerUrl: "https://relay.example/v1/", fetchImpl };
    expect(await relaySignedMetaTransaction(sent)).toBe(`0x${"ab".repeat(32)}`);
    expect(posts[0]).toEqual({ url: "https://relay.example/v1/transactions", body: { transactionData: { from, params: [target.address, data] } } });
    answer.body = { ok: false, not_sent: true, message: "quota" };
    answer.status = 429;
    await expect(relaySignedMetaTransaction(sent)).rejects.toMatchObject({ reason: "relayer-rejected" });
    answer.body = { ok: false, message: "boom" };
    answer.status = 500;
    await expect(relaySignedMetaTransaction(sent)).rejects.toMatchObject({ reason: "relayer-unreachable" });
  });

  it("a declined prompt is a cancellation; a refusal and a dead relay are told apart", async () => {
    const declineProvider = { request: async () => Promise.reject(Object.assign(new Error("User rejected the request"), { code: 4001 })) };
    await expect(
      relayMetaTransaction({ provider: declineProvider, reader: fakeReader(), target, from, functionData: "0x12" }),
    ).rejects.toBeInstanceOf(SignatureDeclinedError);
    const provider = { request: async () => `0x${"22".repeat(65)}` };
    const refused = (async () => new Response(JSON.stringify({ ok: false, message: "gas estimation failed", not_sent: true }), { status: 400 })) as unknown as typeof fetch;
    await expect(
      relayMetaTransaction({ provider, reader: fakeReader(), target, from, functionData: "0x12", fetchImpl: refused }),
    ).rejects.toMatchObject({ reason: "relayer-rejected" });
    const dead = (async () => {
      throw new TypeError("Failed to fetch");
    }) as unknown as typeof fetch;
    await expect(
      relayMetaTransaction({ provider, reader: fakeReader(), target, from, functionData: "0x12", fetchImpl: dead }),
    ).rejects.toMatchObject({ reason: "relayer-unreachable" });
    await expect(
      relayMetaTransaction({ provider, reader: fakeReader({ code: async () => "0x6080" }), target, from, functionData: "0x12" }),
    ).rejects.toBeInstanceOf(GaslessUnavailableError);
  });

  it("with an exact nonce, a nonce that reads as any other (behind or ahead) asks the wallet nothing and sends nothing", async () => {
    const asked: string[] = [];
    const provider = {
      request: async ({ method }: { method: string }) => {
        asked.push(method);
        return `0x${"22".repeat(65)}`;
      },
    };
    const posts: unknown[] = [];
    const fetchImpl = (async (_url: string, init?: RequestInit) => {
      posts.push(init?.body);
      return new Response(JSON.stringify({ ok: true, txHash: `0x${"ab".repeat(32)}` }), { status: 200 });
    }) as unknown as typeof fetch;
    // The contract's nonce reads 7.
    for (const exactNonce of [6n, 8n]) {
      const moved = relayMetaTransaction({ provider, reader: fakeReader(), target, from, functionData: "0x12", fetchImpl, exactNonce });
      await expect(moved).rejects.toBeInstanceOf(NonceMovedError);
      await expect(moved).rejects.toMatchObject({ expected: exactNonce, read: 7n });
    }
    expect(asked).toEqual([]);
    expect(posts).toEqual([]);
    expect(await relayMetaTransaction({ provider, reader: fakeReader(), target, from, functionData: "0x12", fetchImpl, exactNonce: 7n })).toMatchObject({ nonce: 7n });
    expect(asked).toEqual(["eth_signTypedData_v4"]);
  });

  it("keeps the relay's own word for a refusal: its error code, and when a spent quota frees up", async () => {
    const provider = { request: async () => `0x${"22".repeat(65)}` };
    const answering = (status: number, body: unknown) => (async () => new Response(JSON.stringify(body), { status })) as unknown as typeof fetch;
    const relay = (fetchImpl: typeof fetch) => relayMetaTransaction({ provider, reader: fakeReader(), target, from, functionData: "0x12", fetchImpl });
    const quota = { ok: false, error: "quota_reached", message: "Max amount of transactions reached for address. Quota: 30", not_sent: true, retryAt: 1_767_230_000, quota: 30 };
    await expect(relay(answering(429, quota))).rejects.toMatchObject({ reason: "relayer-rejected", refusal: { error: "quota_reached", retryAt: 1_767_230_000, quota: 30 } });
    await expect(relay(answering(429, { ok: false, error: "relay_budget_spent", message: "spent", not_sent: true }))).rejects.toMatchObject({ refusal: { error: "relay_budget_spent" } });
    // Each field only in its own shape: no code without a string, no time or count but a positive whole number.
    const odd = { ok: false, error: 7, retryAt: "soon", quota: -1, not_sent: true };
    await expect(relay(answering(429, odd))).rejects.toMatchObject({ refusal: {} });
    expect(relayRefusal({ error: "", retryAt: 1.5, quota: -1 })).toEqual({});
    // A quota of none is kept: the account gets no fee-free transactions.
    expect(relayRefusal({ error: "quota_reached", quota: 0 })).toEqual({ error: "quota_reached", quota: 0 });
    // The code from `code` (upstream's, and dcl-one's relay too), else `error` (dcl-one's, before it answered `code`).
    expect(relayRefusal({ code: "quota_reached" })).toEqual({ error: "quota_reached" });
    expect(relayRefusal({ code: "quota_reached", error: "invalid_transaction" })).toEqual({ error: "quota_reached" });
    expect(relayRefusal({ code: "", error: "relay_budget_spent" })).toEqual({ error: "relay_budget_spent" });
    // Marked as not sent, a code-only refusal (upstream's shape) is a refusal like any; unmarked, a forwarded 429
    // proves nothing was sent no more than any other status does.
    await expect(relay(answering(429, { ok: false, code: "quota_reached", message: "quota", not_sent: true }))).rejects.toMatchObject({ reason: "relayer-rejected", refusal: { error: "quota_reached" } });
    await expect(relay(answering(429, { ok: false, code: "quota_reached", message: "quota" }))).rejects.toMatchObject({ reason: "relayer-unreachable" });
    expect(relayRefusal(null)).toEqual({});
  });

  it("only the relay's explicit evidence proves nothing was sent", async () => {
    const provider = { request: async () => `0x${"22".repeat(65)}` };
    const answering = (status: number, body: unknown) =>
      (async () => new Response(typeof body === "string" ? body : JSON.stringify(body), { status })) as unknown as typeof fetch;
    const relay = (fetchImpl: typeof fetch) => relayMetaTransaction({ provider, reader: fakeReader(), target, from, functionData: "0x12", fetchImpl });
    // The relay marks its refusals before any send, whatever their status.
    for (const [status, error] of [
      [400, "invalid_transaction"],
      [429, "quota_reached"],
      [500, "unknown"],
      [503, "high_congestion"],
    ] as const) {
      await expect(relay(answering(status, { ok: false, error, message: error, not_sent: true })), `${status}`).rejects.toMatchObject({ reason: "relayer-rejected" });
    }
    // The same statuses without that evidence, as a relay of another version or a transactions-server
    // answers them, can follow a send: a 400 for a send the node answered with a revert, a 500 when the
    // hash could not be recorded or the relayer failed after signing.
    for (const status of [400, 429, 500, 503, 501, 502, 504, 520, 524]) {
      await expect(relay(answering(status, { ok: false, error: "unknown", message: "Unknown error" })), `${status}`).rejects.toMatchObject({
        reason: "relayer-unreachable",
      });
    }
    // Evidence that is not exactly `true`, or a body that is not the relay's JSON, is no evidence.
    for (const body of [
      { ok: false, not_sent: "true" },
      { ok: false, not_sent: false },
      { ok: true, not_sent: true },
      "<html>Service Unavailable</html>",
    ]) {
      await expect(relay(answering(503, body)), JSON.stringify(body)).rejects.toMatchObject({ reason: "relayer-unreachable" });
    }
    await expect(relay(answering(200, { ok: true }))).rejects.toMatchObject({ reason: "relayer-unreachable" });
    expect(refusedBeforeBroadcast({ ok: false, not_sent: true })).toBe(true);
    expect(refusedBeforeBroadcast({ ok: false })).toBe(false);
    expect(refusedBeforeBroadcast(null)).toBe(false);
  });

  it("reads the relays' real answers by the evidence they carry", async () => {
    const provider = { request: async () => `0x${"22".repeat(65)}` };
    const answering = (status: number, body: unknown) =>
      (async () => new Response(JSON.stringify(body), { status })) as unknown as typeof fetch;
    const relay = (fetchImpl: typeof fetch) => relayMetaTransaction({ provider, reader: fakeReader(), target, from, functionData: "0x12", fetchImpl });
    // This relay (services/catalyst/shop, evm/tests.rs): refusals before any send carry the evidence.
    const estimateReverted = { ok: false, error: "invalid_transaction", message: "gas estimation reverted: execution reverted", not_sent: true };
    await expect(relay(answering(400, estimateReverted))).rejects.toMatchObject({ reason: "relayer-rejected" });
    const aboveTheFeeCap = { ok: false, error: "high_congestion", message: "the fee per gas 2000 wei exceeds RELAYER_MAX_FEE_PER_GAS_WEI 1000: not sent", not_sent: true };
    await expect(relay(answering(503, aboveTheFeeCap))).rejects.toMatchObject({ reason: "relayer-rejected" });
    // This relay after a send attempt: a 504 without it.
    const afterTheSend = {
      ok: false,
      error: "unknown",
      message: `transaction 0x${"ab".repeat(32)} may have been broadcast: eth_sendRawTransaction failed (rpc error 3: execution reverted)`,
    };
    await expect(relay(answering(504, afterTheSend))).rejects.toMatchObject({ reason: "relayer-unreachable" });
    // An upstream's answer passed through, or a transactions-server this client is pointed at: a
    // failure after its submission is a 500 without the evidence.
    const broadcastFailed = { ok: false, message: "Transaction failed: execution reverted", code: "expectation_failed" };
    await expect(relay(answering(500, broadcastFailed))).rejects.toMatchObject({ reason: "relayer-unreachable" });
    // An earlier version of this relay, after the send: 500 (the hash not recorded) or 400 (a revert
    // answered to the send), without the evidence.
    await expect(relay(answering(500, { ok: false, error: "unknown", message: "Unknown error" }))).rejects.toMatchObject({ reason: "relayer-unreachable" });
    await expect(relay(answering(400, { ok: false, error: "invalid_transaction", message: "transaction reverted: execution reverted" }))).rejects.toMatchObject({
      reason: "relayer-unreachable",
    });
  });
});

describe("the buyer-submitted rail", () => {
  const provider = (chainId: number) => ({
    request: async ({ method }: { method: string }) => {
      if (method === "eth_chainId") return `0x${chainId.toString(16)}`;
      if (method === "eth_sendTransaction") return `0x${"cd".repeat(32)}`;
      throw new Error(method);
    },
  });
  const opts = { from: "0x0000000000000000000000000000000000000b0b", to: "0x0000000000000000000000000000000000000c0c", data: "0x" as Hex, chainId: 137 };

  it("refuses the wrong network before prompting", async () => {
    await expect(sendAndConfirm({ ...opts, provider: provider(1), reader: fakeReader() })).rejects.toMatchObject({ name: "WrongNetworkError" });
  });

  it("reports the broadcast, then resolves only on a successful receipt", async () => {
    const onBroadcast = vi.fn();
    const ok = fakeReader({ receipt: async (_c, h) => ({ transactionHash: h as Hex, status: "success", blockNumber: 1n }) });
    await expect(sendAndConfirm({ ...opts, provider: provider(137), reader: ok, onBroadcast })).resolves.toBe(`0x${"cd".repeat(32)}`);
    expect(onBroadcast).toHaveBeenCalledWith(`0x${"cd".repeat(32)}`);
    const reverted = fakeReader({ receipt: async (_c, h) => ({ transactionHash: h as Hex, status: "reverted", blockNumber: 1n }) });
    await expect(sendAndConfirm({ ...opts, provider: provider(137), reader: reverted })).rejects.toBeInstanceOf(TxRevertedError);
    const sleep = async () => {};
    let t = 0;
    await expect(
      sendAndConfirm({ ...opts, provider: provider(137), reader: fakeReader(), wait: { timeoutMs: 10, now: () => (t += 20), sleep } }),
    ).rejects.toBeInstanceOf(TxPendingError);
  });
});

describe("the RPC reader", () => {
  it("speaks JSON-RPC to the chain's own endpoint and keeps polling through read errors", async () => {
    const calls: { url: string; method: string }[] = [];
    const fetchImpl = (async (url: string, init?: RequestInit) => {
      const { method } = JSON.parse(String(init?.body)) as { method: string };
      calls.push({ url, method });
      if (method === "eth_getTransactionReceipt") {
        return new Response(JSON.stringify({ result: { transactionHash: "0x01", status: "0x1", blockNumber: "0x10" } }));
      }
      return new Response(JSON.stringify({ result: "0x05" }));
    }) as unknown as typeof fetch;
    const reader = createRpcReader({ url: (c) => `https://rpc.example/${c}`, fetchImpl });
    expect(await reader.balance(137, "0x00")).toBe(5n);
    expect(await reader.receipt(137, "0x01")).toEqual({ transactionHash: "0x01", status: "success", blockNumber: 16n });
    expect(calls[0]).toEqual({ url: "https://rpc.example/137", method: "eth_getBalance" });
    let n = 0;
    const flaky = fakeReader({
      receipt: async () => {
        n += 1;
        if (n < 3) throw new Error("rate limited");
        return { transactionHash: "0x02", status: "success", blockNumber: 1n };
      },
    });
    expect(await waitForReceipt(flaky, 137, "0x02", { sleep: async () => {} })).toMatchObject({ transactionHash: "0x02" });
  });

  it("lets an error answer's body go instead of leaving it open, so polling through errors leaks nothing", async () => {
    let cancelled = false;
    const body = new ReadableStream({ pull: () => new Promise<void>(() => {}), cancel: () => void (cancelled = true) });
    const fetchImpl = (async () => new Response(body, { status: 502 })) as unknown as typeof fetch;
    const reader = createRpcReader({ url: () => "https://rpc.example", fetchImpl });
    await expect(reader.balance(137, "0x00")).rejects.toThrow(/HTTP 502/);
    await vi.waitFor(() => expect(cancelled).toBe(true));
  });

  it("polls every 500 ms for the first 15 s, then every 2 s, unless the caller sets the interval", async () => {
    const waits = async (pollMs?: number) => {
      let clock = 0;
      const slept: number[] = [];
      const reader = fakeReader({ receipt: async () => (clock >= 20_000 ? { transactionHash: "0x03", status: "success", blockNumber: 1n } : null) });
      const sleep = async (ms: number) => {
        slept.push(ms);
        clock += ms;
      };
      expect(await waitForReceipt(reader, 137, "0x03", { pollMs, now: () => clock, sleep })).toMatchObject({ transactionHash: "0x03" });
      return slept;
    };
    expect(await waits()).toEqual([...Array(30).fill(500), 2_000, 2_000, 2_000]);
    expect(new Set(await waits(5_000))).toEqual(new Set([5_000]));
  });
});
