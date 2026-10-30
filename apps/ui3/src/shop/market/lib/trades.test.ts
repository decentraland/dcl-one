import { readFileSync } from "node:fs";
import { join } from "node:path";

import {
  decodeFunctionData,
  encodeAbiParameters,
  encodeFunctionData,
  hashTypedData,
  keccak256,
  recoverTypedDataAddress,
  toBytes,
  toFunctionSelector,
  type Hex,
} from "viem";
import { afterEach, describe, expect, it } from "vitest";

import { CHAIN } from "../../model/chain";
import { fakeChain, fakeWallet, noWait } from "../testing";
import { COLLECTION_V2_ABI, COUPON_MANAGER_ABI, ERC721_ABI, latestOffChainMarketplace, MARKETPLACE_ABI, marketplaceForTrade } from "./contracts";
import { executeMetaTransactionData, metaTransactionTypedData, normalizeV } from "./metaTx";
import {
  cancelTrade,
  createBidTrade,
  createManaListing,
  createPrimaryUsdPeggedListing,
  createUsdPeggedListing,
  ensureApproval,
  GaslessCancelFailedError,
  generateTradeValues,
  isResaleTrade,
  onChainTrade,
  OFFCHAIN_MARKETPLACE_TYPES,
  tradePrice,
  tradeTypedData,
  TRADE_ASSET,
  type Trade,
} from "./trades";

const FIXTURES = join(__dirname, "../../../../../../services/catalyst/tests/fixtures/shop/offchain-marketplace-contract@87350c63e/derived");
const selectors = JSON.parse(readFileSync(join(FIXTURES, "selectors.json"), "utf8")) as Record<string, { functions: Record<string, string> }>;
const eip712 = JSON.parse(readFileSync(join(FIXTURES, "eip712.json"), "utf8")) as {
  domainTypeHash: string;
  typeHashes: { constant: string; typeString: string; computed: string }[];
  domains: { contract: string; name: string; version: string }[];
};

const NOW = 1_760_000_000_000;
const deps = { now: () => NOW, salt: () => `0x${"ab".repeat(32)}` as Hex, readIndexes: async () => ({ contractSignatureIndex: 2, signerSignatureIndex: 5 }) };
const COLLECTION = "0x0000000000000000000000000000000000000c11";

function typeHash(constant: string): string {
  return eip712.typeHashes.find((t) => t.constant === constant)!.computed;
}

function selectorOf(contract: string, prefix: string): string {
  const entry = Object.entries(selectors[contract]!.functions).find(([sig]) => sig.startsWith(`${prefix}(`));
  return entry![1];
}

let restore: (() => void) | null = null;
afterEach(() => {
  restore?.();
  restore = null;
});

describe("the ABIs match the pinned contracts", () => {
  it("encodes accept and cancelSignature with the marketplace's selectors", () => {
    const accept = MARKETPLACE_ABI.find((f) => f.type === "function" && f.name === "accept")!;
    const cancel = MARKETPLACE_ABI.find((f) => f.type === "function" && f.name === "cancelSignature")!;
    expect(toFunctionSelector(accept as never)).toBe(selectorOf("DecentralandMarketplacePolygon", "accept"));
    expect(toFunctionSelector(cancel as never)).toBe(selectorOf("DecentralandMarketplacePolygon", "cancelSignature"));
    expect(toFunctionSelector(MARKETPLACE_ABI.find((f) => f.type === "function" && f.name === "executeMetaTransaction") as never)).toBe(
      selectorOf("DecentralandMarketplacePolygon", "executeMetaTransaction"),
    );
    const couponCancel = COUPON_MANAGER_ABI.find((f) => f.type === "function" && f.name === "cancelSignature")!;
    expect(toFunctionSelector(couponCancel as never)).toBe(selectorOf("CouponManager", "cancelSignature"));
  });

  it("builds the EIP-712 type strings the contract hashes", () => {
    const hash = (s: string) => keccak256(toBytes(s));
    expect(hash("EIP712Domain(string name,string version,address verifyingContract,bytes32 salt)")).toBe(eip712.domainTypeHash);
    const encodeType = (primary: keyof typeof OFFCHAIN_MARKETPLACE_TYPES) => {
      const deps = new Set<string>();
      const walk = (name: string) => {
        for (const f of OFFCHAIN_MARKETPLACE_TYPES[name as keyof typeof OFFCHAIN_MARKETPLACE_TYPES]) {
          const base = f.type.replace(/\[\]$/, "");
          if (base in OFFCHAIN_MARKETPLACE_TYPES && !deps.has(base)) {
            deps.add(base);
            walk(base);
          }
        }
      };
      walk(primary);
      const one = (name: string) =>
        `${name}(${OFFCHAIN_MARKETPLACE_TYPES[name as keyof typeof OFFCHAIN_MARKETPLACE_TYPES].map((f) => `${f.type} ${f.name}`).join(",")})`;
      return one(primary) + [...deps].sort().map(one).join("");
    };
    expect(hash(encodeType("Trade"))).toBe(typeHash("TRADE_TYPE_HASH"));
    expect(hash(encodeType("Checks"))).toBe(typeHash("CHECKS_TYPE_HASH"));
    expect(hash(encodeType("Asset"))).toBe(typeHash("ASSET_TYPE_HASH"));
  });

  it("names the marketplace's EIP-712 domain as the contract does", () => {
    const polygon = eip712.domains.find((d) => d.contract === "DecentralandMarketplacePolygon")!;
    const market = latestOffChainMarketplace(CHAIN.POLYGON);
    expect([market.name, market.version]).toEqual([polygon.name, polygon.version]);
    const eth = eip712.domains.find((d) => d.contract === "DecentralandMarketplaceEthereum")!;
    expect(latestOffChainMarketplace(CHAIN.ETHEREUM).name).toBe(eth.name);
  });
});

describe("signing listings and bids", () => {
  it("a MANA resale listing: the ERC721 for ERC20 MANA, the seller as beneficiary, recoverable signature", async () => {
    const wallet = fakeWallet();
    const trade = await createManaListing(
      { provider: wallet, nft: { contractAddress: COLLECTION, tokenId: "42", chainId: CHAIN.POLYGON }, priceWei: 5n * 10n ** 18n, expiresAtMs: NOW + 86_400_000 },
      deps,
    );
    expect(trade.type).toBe("public_nft_order");
    expect(trade.network).toBe("MATIC");
    expect(trade.sent).toEqual([{ assetType: TRADE_ASSET.ERC721, contractAddress: COLLECTION, tokenId: "42", extra: "" }]);
    expect(trade.received[0]).toMatchObject({ assetType: TRADE_ASSET.ERC20, amount: "5000000000000000000", beneficiary: wallet.address });
    expect(trade.checks).toMatchObject({ uses: 1, effective: NOW, contractSignatureIndex: 2, signerSignatureIndex: 5 });
    const typed = tradeTypedData(trade, latestOffChainMarketplace(CHAIN.POLYGON));
    const signer = await recoverTypedDataAddress({ ...typed, types: OFFCHAIN_MARKETPLACE_TYPES, signature: trade.signature as Hex } as never);
    expect(signer.toLowerCase()).toBe(wallet.address);
    expect(isResaleTrade(trade)).toBe(true);
    expect(tradePrice(trade)).toEqual({ kind: "mana", wei: 5n * 10n ** 18n });
  });

  it("a USD-pegged resale stays a resale: seller beneficiary, never a treasury, MANA-only for buyers", async () => {
    const wallet = fakeWallet();
    const trade = await createUsdPeggedListing(
      { provider: wallet, nft: { contractAddress: COLLECTION, tokenId: "42", chainId: CHAIN.POLYGON }, usdCents: 150, expiresAtMs: NOW + 1000 },
      deps,
    );
    expect(trade.received[0]).toMatchObject({ assetType: TRADE_ASSET.USD_PEGGED_MANA, amount: "1500000000000000000", beneficiary: wallet.address });
    expect(isResaleTrade(trade)).toBe(true);
  });

  it("a primary listing sells a COLLECTION_ITEM for USD-pegged MANA to the creator, with its uses", async () => {
    const wallet = fakeWallet();
    const trade = await createPrimaryUsdPeggedListing(
      { provider: wallet, item: { contractAddress: COLLECTION, itemId: "7", chainId: CHAIN.POLYGON }, usdCents: 100, expiresAtMs: NOW + 1000, uses: 25 },
      deps,
    );
    expect(trade.type).toBe("public_item_order");
    expect(trade.checks.uses).toBe(25);
    expect(trade.sent[0]).toMatchObject({ assetType: TRADE_ASSET.COLLECTION_ITEM, itemId: "7" });
    expect(trade.received[0]?.beneficiary).toBe(wallet.address);
    expect(isResaleTrade(trade)).toBe(false);
  });

  it("a bid offers ERC20 MANA for a token or for an item", async () => {
    const wallet = fakeWallet();
    const onToken = await createBidTrade(
      { provider: wallet, asset: { contractAddress: COLLECTION, chainId: CHAIN.POLYGON, tokenId: "42" }, priceWei: 10n ** 18n, expiresAtMs: NOW + 1000 },
      deps,
    );
    expect(onToken.type).toBe("bid");
    expect(onToken.sent[0]).toMatchObject({ assetType: TRADE_ASSET.ERC20, amount: "1000000000000000000" });
    expect(onToken.received[0]).toMatchObject({ assetType: TRADE_ASSET.ERC721, tokenId: "42", beneficiary: wallet.address });
    const onItem = await createBidTrade(
      { provider: wallet, asset: { contractAddress: COLLECTION, chainId: CHAIN.POLYGON, itemId: "7" }, priceWei: 10n ** 18n, expiresAtMs: NOW + 1000 },
      deps,
    );
    expect(onItem.received[0]).toMatchObject({ assetType: TRADE_ASSET.COLLECTION_ITEM, itemId: "7" });
    expect(tradePrice(onItem)).toEqual({ kind: "mana", wei: 10n ** 18n });
  });

  it("refuses a zero price and a past expiration before asking the wallet", async () => {
    const wallet = fakeWallet();
    await expect(
      createManaListing({ provider: wallet, nft: { contractAddress: COLLECTION, tokenId: "1", chainId: CHAIN.POLYGON }, priceWei: 0n, expiresAtMs: NOW + 1 }, deps),
    ).rejects.toThrow(/positive/);
    await expect(
      createManaListing({ provider: wallet, nft: { contractAddress: COLLECTION, tokenId: "1", chainId: CHAIN.POLYGON }, priceWei: 1n, expiresAtMs: NOW }, deps),
    ).rejects.toThrow(/future/);
    expect(wallet.signed).toHaveLength(0);
  });

  it("the EIP-712 message uses seconds and 32-byte salts", () => {
    const values = generateTradeValues({
      signer: "0x1",
      type: "bid",
      network: "MATIC",
      chainId: CHAIN.POLYGON,
      checks: { uses: 1, expiration: 2_000_999, effective: 1_000_500, salt: "0x01", contractSignatureIndex: 0, signerSignatureIndex: 0, allowedRoot: "0x", externalChecks: [] },
      sent: [],
      received: [],
    });
    expect(values.checks.expiration).toBe(2000n);
    expect(values.checks.effective).toBe(1000n);
    expect(values.checks.salt).toBe(`0x${"0".repeat(63)}1`);
    expect(values.checks.allowedRoot).toBe(`0x${"0".repeat(64)}`);
  });
});

async function storedTrade(): Promise<Trade> {
  const wallet = fakeWallet();
  const creation = await createManaListing(
    { provider: wallet, nft: { contractAddress: COLLECTION, tokenId: "42", chainId: CHAIN.POLYGON }, priceWei: 10n ** 18n, expiresAtMs: NOW + 1000 },
    deps,
  );
  return { ...creation, id: "t-1", contract: latestOffChainMarketplace(CHAIN.POLYGON).address };
}

describe("the on-chain trade", () => {
  it("encodes accept([trade]) with the caller as the beneficiary of the sent assets", async () => {
    const trade = await storedTrade();
    const buyer = "0x00000000000000000000000000000000000000b0";
    const data = encodeFunctionData({ abi: MARKETPLACE_ABI, functionName: "accept", args: [[onChainTrade(trade, buyer)]] });
    expect(data.slice(0, 10)).toBe(selectorOf("DecentralandMarketplacePolygon", "accept"));
    const decoded = decodeFunctionData({ abi: MARKETPLACE_ABI, data });
    const [trades] = decoded.args as unknown as [ReturnType<typeof onChainTrade>[]];
    expect(trades[0]!.sent[0]!.beneficiary.toLowerCase()).toBe(buyer);
    expect(trades[0]!.received[0]!.beneficiary.toLowerCase()).toBe(trade.signer);
    expect(trades[0]!.checks.expiration).toBe(BigInt(Math.floor((NOW + 1000) / 1000)));
  });

  it("resolves only a marketplace deployed on the trade's own chain", () => {
    expect(marketplaceForTrade({ contract: latestOffChainMarketplace(CHAIN.POLYGON).address, chainId: CHAIN.POLYGON })?.name).toBe("DecentralandMarketplacePolygon");
    expect(marketplaceForTrade({ contract: latestOffChainMarketplace(CHAIN.POLYGON).address, chainId: CHAIN.ETHEREUM })).toBeNull();
  });
});

describe("cancelling a listing", () => {
  it("relays cancelSignature gasless on Polygon and waits for the receipt", async () => {
    const trade = await storedTrade();
    const wallet = fakeWallet();
    const posted: { url: string; body: string }[] = [];
    restore = fakeChain({ contracts: { [trade.contract]: { abi: MARKETPLACE_ABI, answer: () => 3n } } });
    const fetchImpl = async (url: string, init?: RequestInit) => {
      posted.push({ url, body: String(init?.body) });
      return new Response(JSON.stringify({ ok: true, txHash: `0x${"c".repeat(64)}` }));
    };
    const original = globalThis.fetch;
    globalThis.fetch = fetchImpl as typeof fetch;
    try {
      const hash = await cancelTrade({ provider: wallet, trade, wait: noWait });
      expect(hash).toBe(`0x${"c".repeat(64)}`);
    } finally {
      globalThis.fetch = original;
    }
    expect(posted[0]!.url).toMatch(/\/transactions-api\/v1\/transactions$/);
    const body = JSON.parse(posted[0]!.body) as { transactionData: { from: string; params: [string, Hex] } };
    expect(body.transactionData.params[0]).toBe(trade.contract);
    expect(body.transactionData.params[1].slice(0, 10)).toBe(selectorOf("DecentralandMarketplacePolygon", "executeMetaTransaction"));
    expect(wallet.sent).toHaveLength(0);
  });

  it("gasless-only reports a failed relay instead of paying gas; a stranger cannot cancel", async () => {
    const trade = await storedTrade();
    const wallet = fakeWallet();
    restore = fakeChain({ contracts: { [trade.contract]: { abi: MARKETPLACE_ABI, answer: () => 3n } } });
    const original = globalThis.fetch;
    globalThis.fetch = (async () => new Response(JSON.stringify({ ok: false, message: "relay down", code: "UNKNOWN" }), { status: 503 })) as typeof fetch;
    try {
      await expect(cancelTrade({ provider: wallet, trade, mode: "gasless-only", wait: noWait })).rejects.toBeInstanceOf(GaslessCancelFailedError);
      expect(wallet.sent).toHaveLength(0);
      const hash = await cancelTrade({ provider: wallet, trade, mode: "direct", wait: noWait });
      expect(wallet.sent).toHaveLength(1);
      expect(wallet.sent[0]!.data.slice(0, 10)).toBe(selectorOf("DecentralandMarketplacePolygon", "cancelSignature"));
      expect(hash).toMatch(/^0x/);
    } finally {
      globalThis.fetch = original;
    }
    const stranger = fakeWallet({ key: `0x${"5".repeat(64)}` });
    await expect(cancelTrade({ provider: stranger, trade, wait: noWait })).rejects.toThrow(/signed/);
  });
});

describe("approvals", () => {
  it("skips an existing approval and relays setApprovalForAll on a Polygon collection otherwise", async () => {
    const wallet = fakeWallet();
    let approved = true;
    restore = fakeChain({
      contracts: {
        [COLLECTION]: { abi: [...ERC721_ABI, ...COLLECTION_V2_ABI], answer: (fn) => (fn === "getNonce" ? 0n : approved) },
      },
    });
    expect(await ensureApproval({ provider: wallet, contractAddress: COLLECTION, chainId: CHAIN.POLYGON, wait: noWait })).toBeNull();
    approved = false;
    const original = globalThis.fetch;
    let relayed: Hex | null = null;
    globalThis.fetch = (async (_url: string, init?: RequestInit) => {
      relayed = (JSON.parse(String(init?.body)) as { transactionData: { params: [string, Hex] } }).transactionData.params[1];
      return new Response(JSON.stringify({ ok: true, txHash: `0x${"d".repeat(64)}` }));
    }) as typeof fetch;
    try {
      await ensureApproval({ provider: wallet, contractAddress: COLLECTION, chainId: CHAIN.POLYGON, wait: noWait });
    } finally {
      globalThis.fetch = original;
    }
    expect(relayed!.slice(0, 10)).toBe("0x0c53c51c");
  });
});

describe("meta-transactions", () => {
  it("signs the MetaTransaction type the marketplace hashes, and normalizes a Ledger v", () => {
    const hash = keccak256(toBytes("MetaTransaction(uint256 nonce,address from,bytes functionData)"));
    expect(hash).toBe(typeHash("META_TRANSACTION_TYPEHASH"));
    const market = latestOffChainMarketplace(CHAIN.POLYGON);
    const typed = metaTransactionTypedData(market, "offchain", "0x00000000000000000000000000000000000000b0", 7n, "0x1234");
    expect(Object.keys(typed.message)).toEqual(["nonce", "from", "functionData"]);
    expect(hashTypedData(typed as never)).toMatch(/^0x[0-9a-f]{64}$/);
    expect(normalizeV(0)).toBe(27);
    expect(normalizeV(28)).toBe(28);
    const sig = `0x${"11".repeat(32)}${"22".repeat(32)}01` as Hex;
    const data = executeMetaTransactionData("offchain", "0x00000000000000000000000000000000000000b0", sig, "0x1234");
    expect(data.slice(0, 10)).toBe("0xd8ed1acc");
    expect(data).toContain(`${"22".repeat(32)}1c`);
    const collectionData = executeMetaTransactionData("collection", "0x00000000000000000000000000000000000000b0", sig, "0x1234");
    expect(collectionData.slice(0, 10)).toBe("0x0c53c51c");
    expect(encodeAbiParameters([{ type: "uint8" }], [28])).toBe(`0x${"0".repeat(62)}1c`);
  });
});
