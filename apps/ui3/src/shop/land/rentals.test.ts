import { hashTypedData, recoverTypedDataAddress, type Hex } from "viem";
import { describe, expect, it } from "vitest";

import type { RentalListing } from "@catalyst-api/market/RentalListing";
import { CHAIN } from "../model/chain";
import { WrongNetworkError } from "../wallet/chain";
import { LAND_NETWORKS } from "./contracts";
import { erc20Abi } from "./evm";
import { estateRegistryAbi } from "./estate";
import {
  acceptRentalListing,
  canBeClaimed,
  canCreateANewRental,
  claimAsset,
  generateECDSASignatureWithValidV,
  isLandLocked,
  isOwnedBy,
  listingPeriods,
  maxPriceOfPeriods,
  PeriodOption,
  pricePerRent,
  removeRental,
  RentalInputError,
  rentalEndDate,
  rentalsAbi,
  rentalTypedData,
  upsertRental,
} from "./rentals";
import { fakeWallet, pollCountedWait, stubClient } from "./testkit";

const MAINNET = LAND_NETWORKS[CHAIN.ETHEREUM]!;
const LESSOR = "0x19e7e376e7c213b7e7e7e46cc70a5dd086daff2a";
const TENANT = "0x00000000000000000000000000000000000000aa";
const DAY = 86_400_000;

function listing(over: Partial<RentalListing> = {}): RentalListing {
  return {
    id: "rental-1",
    nftId: `${MAINNET.land}-42`,
    category: "parcel",
    searchText: "12,-4",
    network: "ETHEREUM",
    chainId: 1,
    expiration: 1893456000000,
    signature: `0x${"ab".repeat(64)}1c`,
    nonces: ["0", "0", "0"],
    tokenId: "42",
    contractAddress: MAINNET.land,
    rentalContractAddress: MAINNET.rentals,
    lessor: LESSOR,
    tenant: null,
    status: "open",
    createdAt: 0,
    updatedAt: 0,
    startedAt: null,
    periods: [
      { minDays: 7, maxDays: 7, pricePerDay: "2000000000000000000" },
      { minDays: 30, maxDays: 30, pricePerDay: "1000000000000000000" },
    ],
    target: "0x0000000000000000000000000000000000000000",
    rentedDays: null,
    ...over,
  };
}

describe("the signed rental listing", () => {
  const vector = {
    signer: LESSOR,
    contractAddress: MAINNET.land,
    tokenId: "42",
    expiration: 1893456000 * 1000,
    nonces: ["0", "0", "0"],
    periods: [{ minDays: 1, maxDays: 30, pricePerDay: "1000000000000000000" }],
  };
  const signature = "0xab333677b5572585e44bc94d70e60eef3468f9c08c896f44deb920a00599be71531f4a7bbbcff3d06de00a0c399ed09eeb342fb80a2eb898e887568c54ae20071c" as Hex;

  it("hashes to the digest the v2 signatures server verifies (src/signatures/signature.rs digest_matches_captured_vector)", () => {
    const { types, ...typed } = rentalTypedData(1, vector);
    const { EIP712Domain: _domain, ...listingTypes } = types;
    expect(hashTypedData({ ...typed, types: listingTypes } as never)).toBe("0xd6c00455cf6c7c140ed004ba43dcb049b121ea69c690043c69431815343cbc82");
  });

  it("recovers the lessor from the captured signature", async () => {
    const { types, ...typed } = rentalTypedData(1, vector);
    const { EIP712Domain: _domain, ...listingTypes } = types;
    const signer = await recoverTypedDataAddress({ ...typed, types: listingTypes, signature } as never);
    expect(signer.toLowerCase()).toBe(LESSOR);
  });

  it("repairs a v of 0/1 to 27/28 and keeps a valid one", () => {
    expect(generateECDSASignatureWithValidV(`0x${"ab".repeat(64)}00`)).toBe(`0x${"ab".repeat(64)}1b`);
    expect(generateECDSASignatureWithValidV(`0x${"ab".repeat(64)}01`)).toBe(`0x${"ab".repeat(64)}1c`);
    expect(generateECDSASignatureWithValidV(`0x${"ab".repeat(64)}1c`)).toBe(`0x${"ab".repeat(64)}1c`);
  });
});

describe("rental periods and prices (MANA only)", () => {
  it("publishes one fixed period per option at one exact price, sorted by days", () => {
    expect(listingPeriods("12.5", [PeriodOption.ONE_MONTH, PeriodOption.ONE_WEEK])).toEqual([
      { minDays: 7, maxDays: 7, pricePerDay: "12500000000000000000" },
      { minDays: 30, maxDays: 30, pricePerDay: "12500000000000000000" },
    ]);
  });

  it("refuses a price under 1 MANA, a bad price and no periods", () => {
    expect(() => listingPeriods("0.5", [PeriodOption.ONE_DAY])).toThrow("cannot be less than 1 MANA");
    expect(() => listingPeriods("abc", [PeriodOption.ONE_DAY])).toThrow(RentalInputError);
    expect(() => listingPeriods("5", [])).toThrow("at least one rent period");
  });

  it("prices a period as price per day times its days, exactly", () => {
    expect(pricePerRent({ minDays: 30, maxDays: 30, pricePerDay: "1000000000000000001" })).toBe(30000000000000000030n);
    expect(maxPriceOfPeriods(listing())).toBe("2000000000000000000");
  });
});

describe("rental state", () => {
  const asset = { chainId: 1, contractAddress: MAINNET.land, tokenId: "42", owner: MAINNET.rentals };
  const started = Date.UTC(2026, 0, 1);

  it("ends a started rental after its rented days", () => {
    const r = listing({ status: "executed", startedAt: started, rentedDays: 30, tenant: TENANT });
    expect(rentalEndDate(r)?.getTime()).toBe(started + 30 * DAY);
    expect(rentalEndDate(listing())).toBeNull();
  });

  it("lets the lessor claim only after the rental ended, or while the contract still holds an open listing's LAND", () => {
    const executed = listing({ status: "executed", startedAt: started, rentedDays: 30, tenant: TENANT });
    expect(canBeClaimed(LESSOR, executed, asset, started + 29 * DAY)).toBe(false);
    expect(canBeClaimed(LESSOR, executed, asset, started + 30 * DAY)).toBe(true);
    expect(canBeClaimed(LESSOR, listing({ status: "cancelled" }), asset)).toBe(true);
    expect(canBeClaimed(LESSOR, listing({ status: "open" }), { ...asset, owner: LESSOR })).toBe(false);
  });

  it("locks the LAND while rented or waiting to be claimed", () => {
    const executed = listing({ status: "executed", startedAt: started, rentedDays: 30, tenant: TENANT });
    expect(isLandLocked(LESSOR, executed, asset, started + DAY)).toBe(true);
    expect(isLandLocked(LESSOR, listing(), { ...asset, owner: LESSOR })).toBe(false);
  });

  it("treats the lessor as the owner while the Rentals contract holds the LAND", () => {
    expect(isOwnedBy(asset, LESSOR, listing())).toBe(true);
    expect(isOwnedBy(asset, TENANT, listing())).toBe(false);
    expect(isOwnedBy({ ...asset, owner: LESSOR }, LESSOR)).toBe(true);
  });

  it("allows a new listing when none exists, after a cancel, or after the rental ended", () => {
    expect(canCreateANewRental(null)).toBe(true);
    expect(canCreateANewRental(listing({ status: "cancelled" }))).toBe(true);
    expect(canCreateANewRental(listing())).toBe(false);
  });
});

const nonceReads = [
  { abi: rentalsAbi, name: "getContractIndex", answer: () => 3n },
  { abi: rentalsAbi, name: "getSignerIndex", answer: () => 4n },
  { abi: rentalsAbi, name: "getAssetIndex", answer: () => 5n },
];

describe("UPSERT_RENTAL: sign and post a listing", () => {
  it("signs the listing with the on-chain nonces and posts it to /shop-api/v1/rentals-listings", async () => {
    const wallet = fakeWallet({ account: LESSOR, chainId: 1, reads: nonceReads, signature: `0x${"22".repeat(64)}00` });
    const client = stubClient({ "POST /shop-api/v1/rentals-listings": { body: { ok: true, data: listing({ id: "new" }) } } });
    const expiresAt = Date.now() + 30 * DAY;
    const created = await upsertRental({
      provider: wallet,
      account: LESSOR,
      asset: { chainId: 1, contractAddress: MAINNET.land, tokenId: "42" },
      pricePerDay: "3",
      periods: [PeriodOption.ONE_WEEK],
      expiresAt,
      client,
    });
    expect(created.id).toBe("new");
    const typed = wallet.signed[0] as ReturnType<typeof rentalTypedData>;
    expect(typed.domain).toEqual({ name: "Rentals", version: "1", chainId: 1, verifyingContract: MAINNET.rentals });
    expect(typed.message.indexes).toEqual(["3", "4", "5"]);
    const body = JSON.parse(client.requests[0]!.body!);
    expect(body).toMatchObject({
      network: "ETHEREUM",
      chainId: 1,
      expiration: expiresAt,
      nonces: ["3", "4", "5"],
      periods: [{ minDays: 7, maxDays: 7, pricePerDay: "3000000000000000000" }],
      rentalContractAddress: MAINNET.rentals,
      target: "0x0000000000000000000000000000000000000000",
    });
    expect(body.signature.endsWith("1b")).toBe(true);
    expect(client.requests[0]!.signed).toBe(true);
  });

  it("refuses to sign from the wrong network instead of switching it", async () => {
    const wallet = fakeWallet({ account: LESSOR, chainId: 137, reads: nonceReads });
    await expect(
      upsertRental({
        provider: wallet,
        account: LESSOR,
        asset: { chainId: 1, contractAddress: MAINNET.land, tokenId: "42" },
        pricePerDay: "3",
        periods: [PeriodOption.ONE_WEEK],
        expiresAt: Date.now() + DAY,
        client: stubClient({}),
      }),
    ).rejects.toBeInstanceOf(WrongNetworkError);
    expect(wallet.calls).not.toContain("wallet_switchEthereumChain");
  });

  it("surfaces the server's refusal", async () => {
    const wallet = fakeWallet({ account: LESSOR, chainId: 1, reads: nonceReads });
    const client = stubClient({
      "POST /shop-api/v1/rentals-listings": { status: 409, body: { ok: false, error: "There is already an open rental listing for the asset" } },
    });
    await expect(
      upsertRental({
        provider: wallet,
        account: LESSOR,
        asset: { chainId: 1, contractAddress: MAINNET.land, tokenId: "42" },
        pricePerDay: "3",
        periods: [PeriodOption.ONE_DAY],
        expiresAt: Date.now() + DAY,
        client,
      }),
    ).rejects.toThrow("There is already an open rental listing for the asset");
  });
});

describe("ACCEPT_RENTAL_LISTING: rent with MANA", () => {
  const poll = { retries: 2, delayMs: 0 };
  const wait = pollCountedWait(50);

  it("approves exactly the rent when the allowance is short, then accepts the chosen period and waits for the index", async () => {
    const wallet = fakeWallet({
      account: TENANT,
      chainId: 1,
      reads: [{ abi: erc20Abi, name: "allowance", answer: () => 0n }],
      writes: [rentalsAbi],
    });
    const client = stubClient({ "PATCH /shop-api/v1/rentals-listings/rental-1": { body: { ok: true, data: listing({ status: "executed", tenant: TENANT }) } } });
    const stages: string[] = [];
    const out = await acceptRentalListing({
      provider: wallet,
      account: TENANT,
      asset: { chainId: 1, contractAddress: MAINNET.land, tokenId: "42", openRentalId: "rental-1" },
      rental: listing(),
      periodIndex: 1,
      operator: TENANT,
      agreedWei: 30n * 10n ** 18n,
      client,
      poll,
      wait,
      onStage: (s) => stages.push(s),
    });
    expect(out.rental.status).toBe("executed");
    expect(wallet.sent.map((t) => t.fn)).toEqual(["approve", "acceptListing"]);
    expect(wallet.sent[0]).toMatchObject({ to: MAINNET.mana, args: [MAINNET.rentals, 30n * 10n ** 18n] });
    const [tuple, operator, index, days, fingerprint] = wallet.sent[1]!.args as [Record<string, unknown>, string, bigint, bigint, string];
    expect(wallet.sent[1]!.to).toBe(MAINNET.rentals);
    expect(tuple).toMatchObject({ signer: LESSOR, tokenId: 42n, expiration: 1893456000n, indexes: [0n, 0n, 0n], maxDays: [7n, 30n] });
    expect([operator.toLowerCase(), index, days, fingerprint]).toEqual([TENANT, 1n, 30n, `0x${"00".repeat(32)}`]);
    expect(stages).toEqual(["checking", "awaiting-approval", "approving", "awaiting-confirmation", "confirming", "indexing"]);
  });

  it("skips the approval when the allowance covers the rent and binds an estate to its fingerprint", async () => {
    const fp = `0x${"ef".repeat(32)}`;
    const wallet = fakeWallet({
      account: TENANT,
      chainId: 1,
      reads: [
        { abi: erc20Abi, name: "allowance", answer: () => 10n ** 30n },
        { abi: estateRegistryAbi, name: "getFingerprintV2", answer: () => fp },
      ],
      writes: [rentalsAbi],
    });
    const reader = { chainId: 1, call: (to: string, data: Hex) => wallet.request({ method: "eth_call", params: [{ to, data }] }) as Promise<Hex> };
    const client = stubClient({ "PATCH /shop-api/v1/rentals-listings/r": { body: { ok: true, data: listing({ status: "executed" }) } } });
    await acceptRentalListing({
      provider: wallet,
      account: TENANT,
      asset: { chainId: 1, contractAddress: MAINNET.estate, tokenId: "9", openRentalId: "r" },
      rental: listing({ contractAddress: MAINNET.estate, tokenId: "9" }),
      periodIndex: 0,
      operator: TENANT,
      agreedWei: 14n * 10n ** 18n,
      reader,
      client,
      poll,
      wait,
    });
    expect(wallet.sent.map((t) => t.fn)).toEqual(["acceptListing"]);
    expect((wallet.sent[0]!.args as unknown[])[4]).toBe(fp);
  });

  it("refuses a rent above the price the buyer agreed to", async () => {
    const wallet = fakeWallet({ account: TENANT, chainId: 1 });
    await expect(
      acceptRentalListing({
        provider: wallet,
        account: TENANT,
        asset: { chainId: 1, contractAddress: MAINNET.land, tokenId: "42", openRentalId: "rental-1" },
        rental: listing(),
        periodIndex: 1,
        operator: TENANT,
        agreedWei: 29n * 10n ** 18n,
        client: stubClient({}),
      }),
    ).rejects.toThrow("The rental price changed");
    expect(wallet.sent).toEqual([]);
  });

  it("does not send from another account than the one that started", async () => {
    const wallet = fakeWallet({ account: TENANT, chainId: 1, reads: [{ abi: erc20Abi, name: "allowance", answer: () => 10n ** 30n }], writes: [rentalsAbi] });
    wallet.emit("accountsChanged", ["0x00000000000000000000000000000000000000bb"]);
    await expect(
      acceptRentalListing({
        provider: wallet,
        account: TENANT,
        asset: { chainId: 1, contractAddress: MAINNET.land, tokenId: "42", openRentalId: "rental-1" },
        rental: listing(),
        periodIndex: 0,
        operator: TENANT,
        agreedWei: 14n * 10n ** 18n,
        reader: { chainId: 1, call: (to, data) => wallet.request({ method: "eth_call", params: [{ to, data }] }) as Promise<Hex> },
        client: stubClient({}),
      }),
    ).rejects.toThrow("different account");
  });
});

describe("CLAIM_ASSET and REMOVE_RENTAL", () => {
  const poll = { retries: 2, delayMs: 0 };
  const wait = pollCountedWait(50);

  it("claims the LAND back and waits for the listing to show claimed", async () => {
    const wallet = fakeWallet({ account: LESSOR, chainId: 1, writes: [rentalsAbi] });
    const client = stubClient({ "PATCH /shop-api/v1/rentals-listings/rental-1": { body: { ok: true, data: listing({ status: "claimed" }) } } });
    const out = await claimAsset({ provider: wallet, account: LESSOR, asset: { chainId: 1, contractAddress: MAINNET.land, tokenId: "42" }, rental: listing(), client, poll, wait });
    expect(out.rental.status).toBe("claimed");
    expect(wallet.sent[0]).toMatchObject({ fn: "claim", args: [[MAINNET.land], [42n]] });
  });

  it("removes a listing by bumping the asset nonce, and says so when the index lags", async () => {
    const wallet = fakeWallet({ account: LESSOR, chainId: 1, writes: [rentalsAbi] });
    const client = stubClient({ "PATCH /shop-api/v1/rentals-listings/rental-1": { body: { ok: true, data: listing({ status: "open" }) } } });
    await expect(
      removeRental({ provider: wallet, account: LESSOR, asset: { chainId: 1, contractAddress: MAINNET.land, tokenId: "42", openRentalId: "rental-1" }, client, poll, wait }),
    ).rejects.toThrow("did not show as cancelled");
    expect(wallet.sent[0]).toMatchObject({ fn: "bumpAssetIndex", args: [MAINNET.land, 42n] });
  });

  it("reports a still-pending transaction as pending, not failed", async () => {
    const wallet = fakeWallet({ account: LESSOR, chainId: 1, writes: [rentalsAbi], autoMine: false });
    await expect(
      claimAsset({ provider: wallet, account: LESSOR, asset: { chainId: 1, contractAddress: MAINNET.land, tokenId: "42" }, rental: listing(), client: stubClient({}), poll, wait }),
    ).rejects.toMatchObject({ name: "TxPendingError" });
  });
});

describe("the rentals API through the one Shop client", () => {
  it("reads listings and the price histogram from the signatures envelopes", async () => {
    const { fetchRentalListings, fetchRentalListingPrices } = await import("./rentals");
    const client = stubClient({
      "GET /shop-api/v1/rentals-listings": { body: { ok: true, data: { results: [listing()], total: 1, page: 0, pages: 1, limit: 100 } } },
      "GET /shop-api/v1/rental-listings/prices": { body: { ok: true, data: { "1000000000000000000": 3 } } },
    });
    expect(await fetchRentalListings({ nftIds: [`${MAINNET.land}-42`], status: ["open"] }, { client })).toEqual({ results: [listing()], total: 1 });
    const q = new URL(client.requests[0]!.url).searchParams;
    expect([q.getAll("nftIds"), q.getAll("status")]).toEqual([[`${MAINNET.land}-42`], ["open"]]);
    expect(await fetchRentalListingPrices({ category: "parcel", rentalDays: [7] }, { client })).toEqual({ "1000000000000000000": 3 });
    const failing = stubClient({ "GET /shop-api/v1/rentals-listings": { status: 400, body: { ok: false, error: "Invalid status", message: "Invalid status" } } });
    await expect(fetchRentalListings({ status: ["open"] }, { client: failing })).rejects.toThrow("Invalid status");
  });
});
