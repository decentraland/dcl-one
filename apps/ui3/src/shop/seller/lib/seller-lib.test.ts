import { readFileSync } from "node:fs";
import { join } from "node:path";

import { QueryClient, type InfiniteData } from "@tanstack/react-query";
import { concat, decodeFunctionData, encodeAbiParameters, keccak256, recoverTypedDataAddress, toBytes, type Hex } from "viem";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { AuthIdentity } from "../../../data/auth/identity";
import { CHAIN } from "../../model/chain";
import { BOOK } from "../../payment/fixtures";
import { COUPON_MANAGER_ABI, COUPON_MANAGERS, MARKETPLACE_ABI, latestOffChainMarketplace } from "../../market/lib/contracts";
import type { MyAsset } from "../../market/lib/api";
import { createPrimaryUsdPeggedListing, TRADE_ASSET, type Trade } from "../../market/lib/trades";
import { fakeChain, fakeShop, fakeWallet, noWait } from "../../market/testing";
import { fetchPublishableItems, isPublishable, RARITY_MAX_SUPPLY } from "./builder";
import { fetchCreatorSaleState, sanitizeCollectionName } from "./collections";
import {
  collectionsRoot,
  couponTypes,
  createCollectionSale,
  encodeCouponData,
  endSale,
  findCouponContracts,
  getCouponContracts,
  liveSaleStatus,
  SaleInputError,
  UNCAPPED_USES,
  validateSaleTerms,
  type CreatorSale,
} from "./coupons";
import { buildEntity, hashV1, signEntityId } from "./entity";
import { DEFAULT_CAPABILITIES, myStoreAccess, setShopCapabilities, shopCapabilities } from "./featureFlags";
import { countConfirmations, fetchImportable, ImportRestrictedError, importListing, postListingWithRetry, RelistFailedError, type ImportItem } from "./import";
import { patchManageCaches } from "./manage-cache";
import { standardLeafHash, standardMerkleRoot } from "./merkle";
import { fetchSellerSales } from "./sales";
import { buildStoreMetadata, draftFromStore, fetchStore, isValidLink, saveStore, storeUrn } from "./store";

const FIXTURES = join(__dirname, "../../../../../../services/catalyst/tests/fixtures/shop/offchain-marketplace-contract@87350c63e/derived");
const eip712 = JSON.parse(readFileSync(join(FIXTURES, "eip712.json"), "utf8")) as { typeHashes: { constant: string; computed: string }[] };
const selectors = JSON.parse(readFileSync(join(FIXTURES, "selectors.json"), "utf8")) as Record<string, { functions: Record<string, string> }>;

const NOW = 1_760_000_000_000;
const DAY = 86_400_000;
const originalFetch = globalThis.fetch;
let restore: (() => void) | null = null;

afterEach(() => {
  restore?.();
  restore = null;
  globalThis.fetch = originalFetch;
  setShopCapabilities(null);
});

describe("the collections Merkle root (OpenZeppelin StandardMerkleTree)", () => {
  const A = "0x4c09495cd2d4e3d3fa2808eb655d013de426157b";
  const B = "0xb0d0d31910da4a14d4e05a9d51b6e9a99a85d676";
  const C = "0x0000000000000000000000000000000000000c11";

  it("a single collection's root is its double-hashed leaf", () => {
    expect(collectionsRoot([A])).toBe(keccak256(keccak256(encodeAbiParameters([{ type: "address" }], [A]))));
  });

  it("is the same root whatever the order, casing or duplicates", () => {
    expect(collectionsRoot([B.toUpperCase().replace("0X", "0x"), A, A])).toBe(collectionsRoot([A, B]));
    expect(() => collectionsRoot([])).toThrow(SaleInputError);
  });

  it("every leaf proves into the root with sorted-pair hashing (what the contract verifies)", () => {
    const leaves = [A, B, C].map((c) => standardLeafHash(["address"], [c])).sort();
    const root = standardMerkleRoot([[A], [B], [C]], ["address"]);
    const pair = (x: Hex, y: Hex) => keccak256(concat([x, y].sort()));
    // OZ makeMerkleTree: sorted leaves fill the heap from its end, so the two smallest pair first.
    expect(root).toBe(pair(pair(leaves[0] as Hex, leaves[1] as Hex), leaves[2] as Hex));
    const proofOf = (leaf: Hex) => (leaf === leaves[2] ? [pair(leaves[0] as Hex, leaves[1] as Hex)] : [leaves[leaf === leaves[0] ? 1 : 0] as Hex, leaves[2] as Hex]);
    for (const leaf of leaves as Hex[]) expect(proofOf(leaf).reduce((acc, sib) => pair(acc, sib), leaf)).toBe(root);
  });
});

describe("creator sales (coupons)", () => {
  it("signs the Coupon type the CouponManager hashes, recoverably, with the V3 manager's domain", async () => {
    const hash = (types: ReturnType<typeof couponTypes>) => {
      const one = (name: keyof typeof types) => `${name}(${types[name].map((f) => `${f.type} ${f.name}`).join(",")})`;
      return keccak256(toBytes(one("Coupon") + one("Checks") + one("ExternalCheck")));
    };
    expect(hash(couponTypes())).toBe(eip712.typeHashes.find((t) => t.constant === "COUPON_TYPE_HASH")!.computed);
    const wallet = fakeWallet();
    const sale = await createCollectionSale(
      { provider: wallet, chainId: CHAIN.POLYGON, collections: [BOOK.collection], discountPct: 30, endsAtMs: NOW + 7 * DAY },
      { now: () => NOW, salt: () => `0x${"cd".repeat(32)}`, readIndexes: async () => ({ contractSignatureIndex: 0, signerSignatureIndex: 1 }) },
    );
    expect(sale).toMatchObject({ discount: 300_000, discountType: 1, network: "MATIC", collections: [BOOK.collection] });
    expect(sale.checks.uses).toBe(UNCAPPED_USES);
    const signed = wallet.signed[0] as { domain: { verifyingContract: string } };
    expect(signed.domain.verifyingContract).toBe(COUPON_MANAGERS.v3[CHAIN.POLYGON]!.address);
    const contracts = getCouponContracts(CHAIN.POLYGON)!;
    const { couponTypedValues, couponDomain } = await import("./coupons");
    const signer = await recoverTypedDataAddress({
      domain: couponDomain(CHAIN.POLYGON, contracts),
      types: couponTypes(),
      primaryType: "Coupon",
      message: couponTypedValues(sale.checks, sale.couponAddress, encodeCouponData(sale.discount, collectionsRoot(sale.collections))),
      signature: sale.signature as Hex,
    } as never);
    expect(signer.toLowerCase()).toBe(wallet.address);
  });

  it("refuses terms the contract or the product would refuse before signing", () => {
    const base = { collections: [BOOK.collection], discountPct: 30, endsAtMs: NOW + DAY };
    const problem = (terms: object) => {
      try {
        validateSaleTerms({ ...base, ...terms }, NOW);
        return null;
      } catch (e) {
        return (e as SaleInputError).problem;
      }
    };
    expect(problem({})).toBeNull();
    expect(problem({ discountPct: 4 })).toBe("pct");
    expect(problem({ discountPct: 71 })).toBe("pct");
    expect(problem({ endsAtMs: NOW })).toBe("window");
    expect(problem({ endsAtMs: NOW + 31 * DAY })).toBe("duration");
    expect(problem({ uses: 0 })).toBe("uses");
    expect(problem({ collections: [] })).toBe("collections");
  });

  it("ends a sale on the manager it was signed against, and refuses an unknown manager", async () => {
    const wallet = fakeWallet();
    restore = fakeChain({});
    const sale: CreatorSale = {
      id: "c1",
      signer: wallet.address,
      chainId: CHAIN.POLYGON,
      network: "MATIC",
      checks: { uses: 10, expiration: NOW + DAY, effective: NOW, salt: "0x01", contractSignatureIndex: 0, signerSignatureIndex: 0, allowedRoot: "0x", externalChecks: [] },
      couponManager: COUPON_MANAGERS.v2[CHAIN.POLYGON]!.address,
      couponAddress: "0xc914507fe297b2dddd1232ac3a8903f1c125e794",
      discountType: 1,
      discount: 200_000,
      root: `0x${"11".repeat(32)}`,
      collections: [BOOK.collection],
      signature: `0x${"22".repeat(65)}`,
      createdAt: NOW,
      state: null,
      status: "active",
    };
    await endSale({ provider: wallet, sale, wait: noWait });
    expect(wallet.sent[0]!.to).toBe(COUPON_MANAGERS.v2[CHAIN.POLYGON]!.address);
    expect(wallet.sent[0]!.data.slice(0, 10)).toBe(selectors.CouponManager!.functions[Object.keys(selectors.CouponManager!.functions).find((k) => k.startsWith("cancelSignature("))!]);
    expect(decodeFunctionData({ abi: COUPON_MANAGER_ABI, data: wallet.sent[0]!.data }).functionName).toBe("cancelSignature");
    expect(findCouponContracts(CHAIN.POLYGON, "0x0000000000000000000000000000000000000001")).toBeNull();
    await expect(endSale({ provider: wallet, sale: { ...sale, couponManager: "0x0000000000000000000000000000000000000001" } })).rejects.toThrow(/not one this build knows/);
    expect(liveSaleStatus({ ...sale, status: "active" }, NOW + 2 * DAY)).toBe("ended");
    expect(liveSaleStatus({ ...sale, checks: { ...sale.checks, effective: NOW + DAY } }, NOW)).toBe("scheduled");
    expect(liveSaleStatus({ ...sale, status: "cancelled" }, NOW)).toBe("cancelled");
  });
});

function importItem(listingType: "primary" | "secondary", over: Partial<ImportItem> = {}): ImportItem {
  return {
    oldTradeId: "old-1",
    listingType,
    contractAddress: BOOK.collection,
    itemId: listingType === "primary" ? "7" : null,
    tokenId: listingType === "secondary" ? "42" : null,
    name: "Neon Jacket",
    thumbnail: "",
    rarity: "rare",
    category: "wearable",
    wearableCategory: "upper_body",
    manaWei: "3000000000000000000",
    available: 25,
    network: "MATIC",
    chainId: CHAIN.POLYGON,
    suggestedCredits: 15,
    ...over,
  };
}

describe("listing import is restricted to eligible primary inventory", () => {
  it("splits creations from owned tokens and prices only creations, from the server's credit quote", async () => {
    const shop = fakeShop([
      {
        method: "GET",
        path: /^\/v3\/catalog\/importable$/,
        reply: () => ({ body: { data: [importItem("primary"), importItem("secondary")].map(({ suggestedCredits: _s, ...l }) => l) } }),
      },
      { method: "POST", path: /^\/v1\/credit-quotes$/, reply: ({ body }) => ({ body: { items: [], amounts: (body as { amounts: string[] }).amounts.map(() => "15") } }) },
    ]);
    const { creations, owned } = await fetchImportable(BOOK.creator, shop.client);
    expect(creations.map((c) => c.suggestedCredits)).toEqual([15]);
    expect(owned).toHaveLength(1);
    expect(owned[0]!.suggestedCredits).toBeNull();
    expect(shop.requests[1]!.body).toEqual({ amounts: ["3000000000000000000"] });
  });

  it("refuses a secondary token BEFORE touching its live listing: no read, no cancel, no signature", async () => {
    const wallet = fakeWallet();
    const shop = fakeShop([]);
    const { setShopClient } = await import("../../api/shop");
    setShopClient(shop.client);
    try {
      await expect(importListing(importItem("secondary"), 15, { provider: wallet, wait: noWait })).rejects.toBeInstanceOf(ImportRestrictedError);
    } finally {
      setShopClient(null);
    }
    expect(shop.requests).toHaveLength(0);
    expect(wallet.signed).toHaveLength(0);
    expect(wallet.sent).toHaveLength(0);
  });

  it("imports a creation: cancels the old trade, grants minting, signs a USD-pegged item order for the remaining supply, retries the 409", async () => {
    const wallet = fakeWallet();
    const market = latestOffChainMarketplace(CHAIN.POLYGON);
    restore = fakeChain({
      contracts: {
        [market.address]: { abi: MARKETPLACE_ABI, answer: () => 0n },
        [BOOK.collection]: { abi: (await import("../../market/lib/contracts")).COLLECTION_V2_ABI, answer: (fn) => (fn === "globalMinters" ? true : 0n) },
      },
    });
    const old = await createPrimaryUsdPeggedListing({ provider: wallet, item: { contractAddress: BOOK.collection, itemId: "7", chainId: CHAIN.POLYGON }, usdCents: 50, expiresAtMs: Date.now() + DAY });
    let posts = 0;
    const shop = fakeShop([
      { method: "GET", path: /^\/v1\/trades\/old-1$/, reply: () => ({ body: { ok: true, data: { ...old, id: "old-1", contract: market.address } } }) },
      {
        method: "POST",
        path: /^\/v1\/trades$/,
        reply: ({ body }) => (++posts === 1 ? { status: 409, body: { ok: false, message: "There is already an open order" } } : { status: 201, body: { ok: true, data: { ...(body as object), id: "new-1", contract: market.address } } }),
      },
    ]);
    globalThis.fetch = (async () => new Response(JSON.stringify({ ok: true, txHash: `0x${"a".repeat(64)}` }))) as typeof fetch;
    const phases: string[] = [];
    const { setShopClient } = await import("../../api/shop");
    setShopClient(shop.client);
    let created: Trade;
    try {
      created = await importListing(importItem("primary"), 15, { provider: wallet, onPhase: (p) => phases.push(p.step), wait: noWait, sleep: async () => {} });
    } finally {
      setShopClient(null);
    }
    expect(created!.id).toBe("new-1");
    expect(phases).toEqual(["cancelling", "confirming-cancel", "authorising", "signing", "publishing", "indexing"]);
    const posted = shop.requests.filter((r) => r.method === "POST")[1]!.body as Trade;
    expect(posted.type).toBe("public_item_order");
    expect(posted.checks.uses).toBe(25);
    expect(posted.received[0]).toMatchObject({ assetType: TRADE_ASSET.USD_PEGGED_MANA, amount: "1500000000000000000", beneficiary: wallet.address });
  });

  it("an import whose new listing fails after the old one was removed says so", async () => {
    const wallet = fakeWallet();
    const market = latestOffChainMarketplace(CHAIN.POLYGON);
    restore = fakeChain({ contracts: { [market.address]: { abi: MARKETPLACE_ABI, answer: () => 0n } } });
    const old = await createPrimaryUsdPeggedListing({ provider: wallet, item: { contractAddress: BOOK.collection, itemId: "7", chainId: CHAIN.POLYGON }, usdCents: 50, expiresAtMs: Date.now() + DAY });
    const shop = fakeShop([{ method: "GET", path: /^\/v1\/trades\/old-1$/, reply: () => ({ body: { ok: true, data: { ...old, id: "old-1", contract: market.address } } }) }]);
    globalThis.fetch = (async () => new Response(JSON.stringify({ ok: true, txHash: `0x${"a".repeat(64)}` }))) as typeof fetch;
    const { setShopClient } = await import("../../api/shop");
    setShopClient(shop.client);
    try {
      await expect(importListing(importItem("primary"), 15, { provider: wallet, wait: noWait })).rejects.toBeInstanceOf(RelistFailedError);
    } finally {
      setShopClient(null);
    }
  });

  it("postListingWithRetry retries only the conflict and stops when aborted", async () => {
    const conflict = Object.assign(new Error("already an open order"), {});
    const post = vi.fn().mockRejectedValueOnce(conflict).mockResolvedValue({ id: "t" });
    expect(await postListingWithRetry({} as never, { post, sleep: async () => {} })).toEqual({ id: "t" });
    const other = vi.fn().mockRejectedValue(new Error("bad signature"));
    await expect(postListingWithRetry({} as never, { post: other, sleep: async () => {} })).rejects.toThrow(/bad signature/);
    expect(other).toHaveBeenCalledTimes(1);
    const controller = new AbortController();
    controller.abort();
    await expect(postListingWithRetry({} as never, { post, signal: controller.signal })).rejects.toBeDefined();
  });

  it("counts two prompts per item plus one per collection without the needed right; null when a read fails", async () => {
    const items = [importItem("primary"), importItem("primary", { itemId: "8" }), importItem("primary", { contractAddress: "0x0000000000000000000000000000000000000c12" })];
    expect(await countConfirmations(items, BOOK.creator, { isMinter: async ({ contractAddress }) => contractAddress === BOOK.collection })).toEqual({ perItem: 2, approvals: 1, total: 7 });
    expect(await countConfirmations(items, BOOK.creator, { isMinter: async () => { throw new Error("rpc"); } })).toBeNull();
  });
});

describe("the store entity", () => {
  it("hashes like @dcl/hashing (the vectors sites pins)", async () => {
    expect(await hashV1(new TextEncoder().encode(""))).toBe("bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku");
    expect(await hashV1(new TextEncoder().encode("hello"))).toBe("bafkreibm6jg3ux5qumhcn2b3flc3tyu6dmlb4xa7u5bf44yegnrjhc4yeq");
    const gen = (size: number) => Uint8Array.from({ length: size }, (_, i) => ((i * 2654435761) >>> 0) & 255);
    expect(await hashV1(gen(262145))).toBe("bafybeiajcvhuc3qypyefmhw43blh7eswfkg2oif3wb6pcwz2mnvv3zcpbq");
    expect(await hashV1(gen(1_000_000))).toBe("bafybeibzkbowoigzk4wlvh2sybr3yn2ywbeuuxivvac6tt2matk4l5jvpi");
  });

  it("reads the active store entity, keeps only https links, and an empty store on failure", async () => {
    const peer = "https://node.test";
    const fetchImpl = vi.fn(async () =>
      Response.json([
        {
          content: [{ file: "cover/pic.png", hash: "bafkcover" }],
          metadata: { description: " Hi ", images: [{ name: "cover", file: "cover/pic.png" }], links: [{ name: "twitter", url: "https://www.twitter.com/me" }, { name: "website", url: "http://insecure" }] },
        },
      ]),
    );
    const store = await fetchStore(BOOK.creator, { base: peer, fetchImpl: fetchImpl as unknown as typeof fetch });
    expect(store).toEqual({ cover: `${peer}/content/contents/bafkcover`, coverHash: "bafkcover", description: "Hi", links: { website: "", twitter: "https://www.twitter.com/me", discord: "", facebook: "" } });
    expect(JSON.parse(String((fetchImpl.mock.calls[0] as unknown as [string, RequestInit])[1].body))).toEqual({ pointers: [storeUrn(BOOK.creator)] });
    expect(await fetchStore(BOOK.creator, { base: peer, fetchImpl: (async () => { throw new Error("down"); }) as typeof fetch })).toMatchObject({ description: "" });
    expect(draftFromStore(store).coverName).toBe("cover/bafkcover");
    expect(isValidLink("discord", "https://discord.gg/abc")).toBe(true);
    expect(isValidLink("discord", "https://evil.example")).toBe(false);
    expect(buildStoreMetadata(BOOK.creator.toUpperCase(), draftFromStore(store))).toMatchObject({ owner: BOOK.creator, links: [{ name: "twitter" }], version: 1 });
  });

  it("deploys a STORE entity signed on top of the owner's auth chain; refuses another account's identity", async () => {
    const { privateKeyToAccount } = await import("viem/accounts");
    const eph = privateKeyToAccount(`0x${"3".repeat(64)}`);
    const identity: AuthIdentity = {
      signer: BOOK.creator,
      ephemeral: { address: eph.address.toLowerCase(), privateKey: `0x${"3".repeat(64)}` },
      expiration: new Date(NOW + DAY).toISOString(),
      authChain: [{ type: "SIGNER", payload: BOOK.creator, signature: "" }],
    };
    const bodies: FormData[] = [];
    const fetchImpl = (async (url: string, init?: RequestInit) => {
      if (String(url).endsWith("/content/entities")) {
        bodies.push(init!.body as FormData);
        return new Response("{}", { status: 200 });
      }
      return new Response(new Uint8Array([1, 2, 3]));
    }) as typeof fetch;
    const draft = { cover: "https://img.test/c.png", coverName: "cover/c.png", coverHash: "", description: "My store", links: { website: "", twitter: "", discord: "", facebook: "" } };
    await saveStore(BOOK.creator, draft, identity, { base: "https://node.test", fetchImpl, now: () => NOW });
    const form = bodies[0]!;
    const entityId = String(form.get("entityId"));
    expect(form.get("authChain[1][type]")).toBe("ECDSA_SIGNED_ENTITY");
    expect(form.get("authChain[1][payload]")).toBe(entityId);
    const built = await buildEntity({ type: "store", pointers: [storeUrn(BOOK.creator)], files: new Map([["cover/c.png", new Uint8Array([1, 2, 3])]]), metadata: buildStoreMetadata(BOOK.creator, draft), timestamp: NOW });
    expect(entityId).toBe(built.entityId);
    expect((await signEntityId(identity, entityId))[1]!.signature).toBe(form.get("authChain[1][signature]"));
    await expect(saveStore(BOOK.reseller, draft, identity, { fetchImpl })).rejects.toThrow();
    expect(bodies).toHaveLength(1);
  });
});

describe("creations and sale state", () => {
  it("reads published items with their remaining supply by rarity, signed over the /builder-api path", async () => {
    const signed: string[] = [];
    const signer = { address: BOOK.creator, sign: async (_m: string, path: string) => (signed.push(path), {}) };
    const fetchImpl = (async (url: string) => {
      const path = new URL(url).pathname;
      if (path.endsWith("/collections"))
        return Response.json({ data: [{ id: "col-1", name: "Neon", eth_address: BOOK.creator, contract_address: BOOK.collection, is_published: true, is_approved: true }] });
      return Response.json({
        data: [
          { id: "i1", collection_id: "col-1", blockchain_item_id: "7", name: "Jacket", rarity: "legendary", total_supply: 40, is_published: true, is_approved: true, thumbnail: "thumbnail.png", contents: { "thumbnail.png": "bafthumb" } },
          { id: "i2", collection_id: "col-1", blockchain_item_id: "8", name: "Gone", rarity: "unique", total_supply: 1, is_published: true, is_approved: true },
        ],
      });
    }) as typeof fetch;
    const items = await fetchPublishableItems(BOOK.creator, { signer, base: "https://node.test", fetchImpl });
    expect(items.map((i) => [i.name, i.remainingSupply])).toEqual([["Jacket", RARITY_MAX_SUPPLY.legendary! - 40]]);
    expect(items[0]!.thumbnail).toBe("https://node.test/builder-api/v1/storage/contents/bafthumb");
    expect(signed[0]).toBe(`/builder-api/v1/${BOOK.creator}/collections`);
    const all = await fetchPublishableItems(BOOK.creator, { signer, base: "https://node.test", fetchImpl, includeSoldOut: true });
    expect(all).toHaveLength(2);
    expect(isPublishable(all[1]!)).toBe(false);
  });

  it("marks USD-pegged primary listings by their exact credits and classic ones by their MANA", async () => {
    const shop = fakeShop([
      {
        method: "GET",
        path: /^\/v3\/catalog\/shop$/,
        reply: () => ({ body: { data: [{ listingType: "primary", itemId: "7", contractAddress: BOOK.collection, priceCredits: 12, tradeId: "t7", compareAtCredits: 15, saleEndsAt: 1_900_000_000 }], total: 1 } }),
      },
      {
        method: "GET",
        path: /^\/v3\/catalog\/items$/,
        reply: () => ({
          body: {
            data: [
              { itemId: "7", contractAddress: BOOK.collection, isOnSale: true, price: "1" },
              { itemId: "8", contractAddress: BOOK.collection, isOnSale: true, price: "3000000000000000000", priceCredits: 16 },
              { itemId: "9", contractAddress: BOOK.collection, isOnSale: false, price: "0" },
            ],
            total: 3,
          },
        }),
      },
    ]);
    const state = await fetchCreatorSaleState(BOOK.creator, shop.client);
    expect(state[`${BOOK.collection}-7`]).toEqual({ isOnSale: true, priceCredits: 12, tradeId: "t7", compareAtCredits: 15, saleEndsAt: 1_900_000_000_000 });
    expect(state[`${BOOK.collection}-8`]).toEqual({ isOnSale: true, priceCredits: 16, manaWei: "3000000000000000000" });
    expect(state[`${BOOK.collection}-9`]).toBeUndefined();
    expect(sanitizeCollectionName("dcl://neon_nights")).toBe("Neon Nights");
  });

  it("caps a seller's sales read and says it was cut short", async () => {
    const rows = Array.from({ length: 3 }, (_, i) => ({ id: `s${i}`, itemId: "7", contractAddress: BOOK.collection, buyer: BOOK.buyer, chainId: 137, network: "MATIC", price: "1", seller: BOOK.creator, timestamp: NOW, tokenId: "1", txHash: "0x", type: "mint" }));
    const shop = fakeShop([{ method: "GET", path: /^\/v1\/sales$/, reply: ({ url }) => ({ body: { data: rows.slice(0, Number(url.searchParams.get("first"))), total: 10 } }) }]);
    const out = await fetchSellerSales({ seller: BOOK.creator }, { cap: 2 }, shop.client);
    expect(out).toMatchObject({ total: 10, truncated: true });
    expect(out.rows).toHaveLength(2);
  });
});

describe("capabilities and caches", () => {
  it("hosts declare capabilities; the store dashboard can be narrowed to addresses", () => {
    expect(shopCapabilities()).toEqual(DEFAULT_CAPABILITIES);
    setShopCapabilities({ creatorSales: false, myStoreOnly: [BOOK.creator] });
    expect(shopCapabilities().creatorSales).toBe(false);
    expect(myStoreAccess(shopCapabilities(), BOOK.creator.toUpperCase().replace("0X", "0x"))).toBe("on");
    expect(myStoreAccess(shopCapabilities(), BOOK.buyer)).toBe("off");
  });

  it("patches an owned token's listing with its MANA price, and drops a transferred one", () => {
    const qc = new QueryClient();
    const asset: MyAsset = { id: "a", contractAddress: BOOK.collection, tokenId: "42", itemId: "7", name: "J", category: "wearable", image: "", network: "MATIC", chainId: 137, isOnSale: false, openRentalId: null };
    qc.setQueryData(["my-assets", BOOK.buyer, "wearables"], { pages: [{ items: [asset], total: 1 }], pageParams: [0] });
    qc.setQueryData(["owned-token", BOOK.collection, "42", BOOK.buyer], asset);
    patchManageCaches(qc, { address: BOOK.buyer, contractAddress: BOOK.collection, tokenId: "42" }, { kind: "listed", price: { kind: "mana", wei: 5n as never }, tradeId: "t1" });
    expect(qc.getQueryData<MyAsset>(["owned-token", BOOK.collection, "42", BOOK.buyer])?.listing).toEqual({ tradeId: "t1", orderId: "", price: { kind: "mana", wei: 5n } });
    patchManageCaches(qc, { address: BOOK.buyer, contractAddress: BOOK.collection, tokenId: "42" }, { kind: "gone" });
    expect(qc.getQueryData<InfiniteData<{ items: MyAsset[]; total: number }>>(["my-assets", BOOK.buyer, "wearables"])?.pages[0]).toEqual({ items: [], total: 0 });
    expect(qc.getQueryData(["owned-token", BOOK.collection, "42", BOOK.buyer])).toBeNull();
  });
});
