// Typed functions per endpoint family over the one client. Area modules (lib/api.ts and the pages)
// adapt upstream's `app/src/lib/api.ts` mappers on top of these; nothing else in ui3/src/shop builds a
// Shop URL. Response types are the frozen generated v2 types where v2 serves the route today, the
// pinned fixture shapes in ./types for the credits routes, and `unknown` where neither pins one yet.

import type { QueryParams } from "../../data/catalyst/client";
import type { CartOut } from "@catalyst-api/credits/CartOut";
import type { CheckoutOut } from "@catalyst-api/credits/CheckoutOut";
import type { CheckoutStartOut } from "@catalyst-api/credits/CheckoutStartOut";
import type { CreditsProgramProgressResponse } from "@catalyst-api/credits/CreditsProgramProgressResponse";
import type { ManaTopupOut } from "@catalyst-api/credits/ManaTopupOut";
import type { ManaTopupQuoteOut } from "@catalyst-api/credits/ManaTopupQuoteOut";
import type { PackIntentOut } from "@catalyst-api/credits/PackIntentOut";
import type { PriceQuotesOut } from "@catalyst-api/credits/PriceQuotesOut";
import type { SeasonsData } from "@catalyst-api/credits/SeasonsData";
import type { UnityCreditPacksResponse } from "@catalyst-api/credits/UnityCreditPacksResponse";
import type { UserCreditsResponse } from "@catalyst-api/credits/UserCreditsResponse";
import type { Account } from "@catalyst-api/market/Account";
import type { BidsEnvelope } from "@catalyst-api/market/BidsEnvelope";
import type { Collection } from "@catalyst-api/market/Collection";
import type { Contract } from "@catalyst-api/market/Contract";
import type { DataTotal } from "@catalyst-api/market/DataTotal";
import type { ImportableResponseBody } from "@catalyst-api/market/ImportableResponseBody";
import type { Item } from "@catalyst-api/market/Item";
import type { ListsEnvelope } from "@catalyst-api/market/ListsEnvelope";
import type { NftsResponseBody } from "@catalyst-api/market/NftsResponseBody";
import type { Order } from "@catalyst-api/market/Order";
import type { Owner } from "@catalyst-api/market/Owner";
import type { PicksEnvelope } from "@catalyst-api/market/PicksEnvelope";
import type { PicksStatsEnvelope } from "@catalyst-api/market/PicksStatsEnvelope";
import type { PickUnpickEnvelope } from "@catalyst-api/market/PickUnpickEnvelope";
import type { RelatedResponseBody } from "@catalyst-api/market/RelatedResponseBody";
import type { RentalListing } from "@catalyst-api/market/RentalListing";
import type { Sale } from "@catalyst-api/market/Sale";
import type { SalesSummary } from "@catalyst-api/market/SalesSummary";
import type { ShopListing } from "@catalyst-api/market/ShopListing";
import type { SuggestionsResponse } from "@catalyst-api/market/SuggestionsResponse";
import type { TopCreatorsResponseBody } from "@catalyst-api/market/TopCreatorsResponseBody";
import type { TrendingResponseBody } from "@catalyst-api/market/TrendingResponseBody";
import type { UnifiedListing } from "@catalyst-api/market/UnifiedListing";
import { createShopApi, type RequestInput, type ShopApi, type ShopApiOptions } from "./client";
import { ENDPOINTS as E } from "./endpoints";
import { storedIdentitySigner } from "./signer";
import type {
  CancelCreditsRequest,
  CancelCreditsResult,
  CreditAuthorization,
  CreditBatchAuthorization,
  CreditBatchRequest,
  CreditLineRequest,
  CreditSubmissionRequest,
  CreditSubmissionResult,
  PackOrderCreated,
  PackOrderRequest,
  PackOrderStatus,
} from "./types";

type Opts = Pick<RequestInput, "signal" | "headers">;
type Q = QueryParams;

export function createShopClient(options: ShopApiOptions | ShopApi = {}) {
  const api: ShopApi = "request" in options ? options : createShopApi(options);
  const get = <T>(endpoint: (typeof E)[keyof typeof E], input: RequestInput = {}) => api.request<T>(endpoint, input);

  return {
    api,
    catalog: {
      shop: <T = { data: ShopListing[]; total?: number }>(query?: Q, o?: Opts) => get<T>(E.catalogShop, { query, ...o }),
      unified: <T = { data: UnifiedListing[]; total?: number }>(query?: Q, o?: Opts) => get<T>(E.catalogUnified, { query, ...o }),
      items: <T = unknown>(query?: Q, o?: Opts) => get<T>(E.catalogItems, { query, ...o }),
      legacy: <T = unknown>(query?: Q, o?: Opts) => get<T>(E.catalogLegacy, { query, ...o }),
      related: <T = RelatedResponseBody>(query?: Q, o?: Opts) => get<T>(E.catalogRelated, { query, ...o }),
      trending: <T = TrendingResponseBody>(query?: Q, o?: Opts) => get<T>(E.catalogTrending, { query, ...o }),
      suggested: <T = SuggestionsResponse>(query?: Q, o?: Opts) => get<T>(E.catalogSuggested, { query, ...o }),
      suggest: <T = unknown>(query?: Q, o?: Opts) => get<T>(E.catalogSuggest, { query, ...o }),
      creators: <T = TopCreatorsResponseBody>(query?: Q, o?: Opts) => get<T>(E.catalogCreators, { query, ...o }),
      searchCreators: <T = unknown>(query?: Q, o?: Opts) => get<T>(E.catalogCreatorsSearch, { query, ...o }),
      importable: <T = ImportableResponseBody>(query?: Q, o?: Opts) => get<T>(E.catalogImportable, { query, ...o }),
      v1: <T = unknown>(query?: Q, o?: Opts) => get<T>(E.catalogV1, { query, ...o }),
      v2: <T = unknown>(query?: Q, o?: Opts) => get<T>(E.catalogV2, { query, ...o }),
    },
    market: {
      items: <T = DataTotal<Item>>(query?: Q, o?: Opts) => get<T>(E.items, { query, ...o }),
      nfts: <T = NftsResponseBody>(query?: Q, o?: Opts) => get<T>(E.nfts, { query, ...o }),
      orders: <T = DataTotal<Order>>(query?: Q, o?: Opts) => get<T>(E.orders, { query, ...o }),
      openOrdersByItems: <T = unknown>(query?: Q, o?: Opts) => get<T>(E.ordersOpenByItems, { query, ...o }),
      bids: <T = BidsEnvelope>(query?: Q, o?: Opts) => get<T>(E.bids, { query, ...o }),
      trades: <T = unknown>(query?: Q, o?: Opts) => get<T>(E.trades, { query, ...o }),
      trade: <T = unknown>(id: string, o?: Opts) => get<T>(E.trade, { params: { id }, ...o }),
      tradeAccepted: <T = unknown>(hashedSignature: string, o?: Opts) => get<T>(E.tradeAccepted, { params: { hashedSignature }, ...o }),
      tradesMirror: <T = unknown>(query?: Q, o?: Opts) => get<T>(E.tradesMirror, { query, ...o }),
      /** marketplace-server requires the `dcl:create-trade` intent in the signed metadata. */
      createTrade: <T = unknown>(trade: unknown, signer: "dcl:marketplace" | "dcl:builder" = "dcl:marketplace", o?: Opts) =>
        get<T>(E.createTrade, { body: trade, metadata: { signer, intent: "dcl:create-trade" }, ...o }),
      coupons: <T = unknown>(query?: Q, o?: Opts) => get<T>(E.coupons, { query, ...o }),
      coupon: <T = unknown>(id: string, o?: Opts) => get<T>(E.coupon, { params: { id }, ...o }),
      createCoupon: <T = unknown>(coupon: unknown, signer: "dcl:marketplace" | "dcl:builder" = "dcl:marketplace", o?: Opts) =>
        get<T>(E.createCoupon, { body: coupon, metadata: { signer, intent: "dcl:create-coupon" }, ...o }),
      accounts: <T = DataTotal<Account>>(query?: Q, o?: Opts) => get<T>(E.accounts, { query, ...o }),
      collections: <T = DataTotal<Collection>>(query?: Q, o?: Opts) => get<T>(E.collections, { query, ...o }),
      contracts: <T = DataTotal<Contract>>(query?: Q, o?: Opts) => get<T>(E.contracts, { query, ...o }),
      owners: <T = DataTotal<Owner>>(query?: Q, o?: Opts) => get<T>(E.owners, { query, ...o }),
      sales: <T = DataTotal<Sale>>(query?: Q, o?: Opts) => get<T>(E.sales, { query, ...o }),
      salesSummary: <T = SalesSummary>(query?: Q, o?: Opts) => get<T>(E.salesSummary, { query, ...o }),
      activity: <T = unknown>(query?: Q, o?: Opts) => get<T>(E.activity, { query, metadata: { signer: "dcl:marketplace" }, ...o }),
      prices: <T = unknown>(query?: Q, o?: Opts) => get<T>(E.prices, { query, ...o }),
      trendings: <T = unknown>(query?: Q, o?: Opts) => get<T>(E.trendings, { query, ...o }),
      stats: <T = unknown>(category: string, stat: string, query?: Q, o?: Opts) =>
        get<T>(E.stats, { params: { category, stat }, query, ...o }),
      rankings: <T = unknown>(entity: string, timeframe: string, query?: Q, o?: Opts) =>
        get<T>(E.rankings, { params: { entity, timeframe }, query, ...o }),
      volume: <T = unknown>(timeframe: string, o?: Opts) => get<T>(E.volume, { params: { timeframe }, ...o }),
      userAssets: <T = unknown>(
        address: string,
        kind: "wearables" | "wearablesGrouped" | "wearablesUrnToken" | "emotes" | "emotesGrouped" | "emotesUrnToken" | "names" | "namesOnly",
        query?: Q,
        o?: Opts,
      ) => {
        const endpoint = {
          wearables: E.userWearables,
          wearablesGrouped: E.userWearablesGrouped,
          wearablesUrnToken: E.userWearablesUrnToken,
          emotes: E.userEmotes,
          emotesGrouped: E.userEmotesGrouped,
          emotesUrnToken: E.userEmotesUrnToken,
          names: E.userNames,
          namesOnly: E.userNamesOnly,
        }[kind];
        return get<T>(endpoint, { params: { address: address.toLowerCase() }, query, ...o });
      },
    },
    lists: {
      lists: <T = ListsEnvelope>(query?: Q, o?: Opts) => get<T>(E.lists, { query, ...o }),
      list: <T = unknown>(id: string, o?: Opts) => get<T>(E.list, { params: { id }, ...o }),
      create: <T = unknown>(body: unknown, o?: Opts) => get<T>(E.createList, { body, ...o }),
      update: <T = unknown>(id: string, body: unknown, o?: Opts) => get<T>(E.updateList, { params: { id }, body, ...o }),
      remove: <T = unknown>(id: string, o?: Opts) => get<T>(E.deleteList, { params: { id }, ...o }),
      picks: <T = PicksEnvelope>(id: string, query?: Q, o?: Opts) => get<T>(E.listPicks, { params: { id }, query, ...o }),
      addPick: <T = unknown>(id: string, body: unknown, o?: Opts) => get<T>(E.addListPick, { params: { id }, body, ...o }),
      removePick: <T = unknown>(id: string, itemId: string, o?: Opts) => get<T>(E.deleteListPick, { params: { id, itemId }, ...o }),
      grantAccess: <T = unknown>(id: string, body: unknown, o?: Opts) => get<T>(E.grantListAccess, { params: { id }, body, ...o }),
      revokeAccess: <T = unknown>(id: string, body: unknown, o?: Opts) => get<T>(E.revokeListAccess, { params: { id }, body, ...o }),
      itemPicks: <T = unknown>(itemId: string, query?: Q, o?: Opts) => get<T>(E.itemPicks, { params: { itemId }, query, ...o }),
      itemPickStats: <T = unknown>(itemId: string, query?: Q, o?: Opts) => get<T>(E.itemPickStats, { params: { itemId }, query, ...o }),
      picksStats: <T = PicksStatsEnvelope>(query?: Q, o?: Opts) => get<T>(E.picksStats, { query, ...o }),
      pickUnpick: <T = PickUnpickEnvelope>(itemId: string, body: unknown, o?: Opts) => get<T>(E.pickUnpick, { params: { itemId }, body, ...o }),
      unpick: <T = unknown>(itemId: string, o?: Opts) => get<T>(E.pickDelete, { params: { itemId }, ...o }),
    },
    credits: {
      balance: <T = UserCreditsResponse>(address: string, o?: Opts) => get<T>(E.userCredits, { params: { address: address.toLowerCase() }, ...o }),
      /** The SSE endpoint's URL (an EventSource cannot send auth headers; the caller signs a fetch stream). */
      streamUrl: (address: string) => api.url(E.userCreditsStream, { params: { address: address.toLowerCase() } }),
      creditOrders: <T = unknown>(address: string, query?: Q, o?: Opts) =>
        get<T>(E.userCreditOrders, { params: { address: address.toLowerCase() }, query, ...o }),
      purchases: <T = unknown>(address: string, query?: Q, o?: Opts) =>
        get<T>(E.userPurchases, { params: { address: address.toLowerCase() }, query, ...o }),
      progress: <T = CreditsProgramProgressResponse>(address: string, o?: Opts) =>
        get<T>(E.userProgress, { params: { address: address.toLowerCase() }, ...o }),
      status: <T = unknown>(address: string, o?: Opts) => get<T>(E.userStatus, { params: { address: address.toLowerCase() }, ...o }),
      register: <T = unknown>(body: unknown = {}, o?: Opts) => get<T>(E.registerUser, { body, ...o }),
      unregister: <T = unknown>(address: string, o?: Opts) => get<T>(E.unregisterUser, { params: { address: address.toLowerCase() }, ...o }),
      captcha: <T = unknown>(o?: Opts) => get<T>(E.captcha, { ...o }),
      verifyCaptcha: <T = unknown>(body: unknown, o?: Opts) => get<T>(E.verifyCaptcha, { body, ...o }),
      season: <T = unknown>(o?: Opts) => get<T>(E.creditSeason, { ...o }),
      seasons: <T = SeasonsData>(o?: Opts) => get<T>(E.creditSeasons, { ...o }),
      seasonsV2: <T = unknown>(o?: Opts) => get<T>(E.creditSeasonsV2, { ...o }),
      nameRoute: <T = unknown>(query?: Q, o?: Opts) => get<T>(E.nameCreditRoute, { query, ...o }),
      quotes: <T = PriceQuotesOut>(body: { items?: { itemId: string; collection: string }[]; amounts?: string[] }, o?: Opts) =>
        get<T>(E.creditQuotes, { body, ...o }),
    },
    packs: {
      list: <T = UnityCreditPacksResponse>(o?: Opts) => get<T>(E.packs, { ...o }),
      order: (body: PackOrderRequest, o?: Opts) => get<PackOrderCreated>(E.createPackOrder, { body, ...o }),
      orderStatus: (orderId: string, o?: Opts) => get<PackOrderStatus>(E.packOrder, { params: { orderId }, ...o }),
      cancelOrder: <T = unknown>(orderId: string, o?: Opts) => get<T>(E.cancelPackOrder, { params: { orderId }, ...o }),
      resumeOrder: <T = unknown>(orderId: string, o?: Opts) => get<T>(E.resumePackOrder, { params: { orderId }, ...o }),
      cancelReturnUrl: (orderId: string) => api.url(E.packOrderCancelReturn, { params: { orderId } }),
      paymentIntent: <T = PackIntentOut>(sku: string, o?: Opts) => get<T>(E.packPaymentIntent, { params: { sku }, ...o }),
    },
    authorizations: {
      authorize: (line: CreditLineRequest, o?: Opts) => get<CreditAuthorization>(E.authorizeCredit, { body: line, ...o }),
      authorizeGroup: (body: CreditBatchRequest, o?: Opts) => get<CreditBatchAuthorization>(E.authorizeCreditBatch, { body, ...o }),
      cancel: (body: CancelCreditsRequest, o?: Opts) => get<CancelCreditsResult>(E.cancelCreditAuthorizations, { body, ...o }),
      submitted: (body: CreditSubmissionRequest, o?: Opts) => get<CreditSubmissionResult>(E.reportCreditSubmission, { body, ...o }),
      publication: <T = unknown>(body: unknown, o?: Opts) => get<T>(E.authorizePublication, { body, ...o }),
    },
    topups: {
      mana: <T = ManaTopupOut>(body: unknown, o?: Opts) => get<T>(E.manaTopup, { body, ...o }),
      quote: <T = ManaTopupQuoteOut>(query?: Q, o?: Opts) => get<T>(E.manaTopupQuote, { query, ...o }),
      config: <T = unknown>(o?: Opts) => get<T>(E.manaTopupConfig, { ...o }),
      nonce: <T = unknown>(address: string, o?: Opts) => get<T>(E.manaTopupNonce, { params: { address: address.toLowerCase() }, ...o }),
      balance: <T = unknown>(address: string, o?: Opts) => get<T>(E.manaTopupBalance, { params: { address: address.toLowerCase() }, ...o }),
    },
    iap: {
      quote: <T = unknown>(body: unknown, o?: Opts) => get<T>(E.iapQuote, { body, ...o }),
      register: <T = unknown>(body: unknown, o?: Opts) => get<T>(E.iapRegister, { body, ...o }),
      verify: <T = unknown>(body: unknown, o?: Opts) => get<T>(E.iapVerify, { body, ...o }),
    },
    checkout: {
      cart: <T = CartOut>(o?: Opts) => get<T>(E.cart, { ...o }),
      addItem: <T = unknown>(body: { collection: string; itemId: string; qty?: number }, o?: Opts) => get<T>(E.addCartItem, { body, ...o }),
      removeItem: <T = unknown>(collection: string, itemId: string, o?: Opts) =>
        get<T>(E.removeCartItem, { params: { collection, itemId }, ...o }),
      create: <T = CheckoutStartOut>(body: unknown, idempotencyKey: string, o?: Opts) =>
        get<T>(E.createCheckout, { body, ...o, headers: { ...(o?.headers ?? {}), "Idempotency-Key": idempotencyKey } }),
      status: <T = CheckoutOut>(id: string | number, o?: Opts) => get<T>(E.checkout, { params: { id }, ...o }),
    },
    outfits: {
      list: <T = unknown>(query?: Q, o?: Opts) => get<T>(E.outfits, { query, ...o }),
      all: <T = unknown>(o?: Opts) => get<T>(E.outfitsAll, { ...o }),
      get: <T = unknown>(id: string, o?: Opts) => get<T>(E.outfit, { params: { id }, ...o }),
      create: <T = unknown>(body: unknown, o?: Opts) => get<T>(E.createOutfit, { body, ...o }),
      update: <T = unknown>(id: string, body: unknown, o?: Opts) => get<T>(E.updateOutfit, { params: { id }, body, ...o }),
      remove: <T = unknown>(id: string, o?: Opts) => get<T>(E.deleteOutfit, { params: { id }, ...o }),
      uploadThumbnail: <T = unknown>(body: unknown, o?: Opts) => get<T>(E.uploadOutfitThumbnail, { body, ...o }),
      thumbnailUrl: (hash: string) => api.url(E.outfitThumbnail, { params: { hash } }),
    },
    notify: {
      list: <T = unknown>(query?: Q, o?: Opts) => get<T>(E.notifyRequests, { query, ...o }),
      create: <T = unknown>(body: unknown, o?: Opts) => get<T>(E.createNotifyRequest, { body, ...o }),
      remove: <T = unknown>(query: Q, o?: Opts) => get<T>(E.deleteNotifyRequest, { query, ...o }),
    },
    rentals: {
      listings: <T = DataTotal<RentalListing>>(query?: Q, o?: Opts) => get<T>(E.rentalListings, { query, ...o }),
      create: <T = unknown>(body: unknown, o?: Opts) => get<T>(E.createRentalListing, { body, ...o }),
      update: <T = unknown>(id: string, body: unknown, o?: Opts) => get<T>(E.updateRentalListing, { params: { id }, body, ...o }),
      prices: <T = unknown>(query?: Q, o?: Opts) => get<T>(E.rentalListingPrices, { query, ...o }),
    },
    onramp: {
      transakWidgetUrl: <T = unknown>(body: unknown, o?: Opts) => get<T>(E.transakWidgetUrl, { body, ...o }),
      transakOrder: <T = unknown>(id: string, o?: Opts) => get<T>(E.transakOrder, { params: { id }, ...o }),
      wertSign: <T = unknown>(body: unknown, o?: Opts) => get<T>(E.wertSign, { body, ...o }),
    },
  };
}

export type ShopClient = ReturnType<typeof createShopClient>;

let shared: ShopClient | null = null;

/** The document's client: canonical paths (or the legacy flag), signed with ui3's stored identity. */
export function shopClient(): ShopClient {
  shared ??= createShopClient({ signer: () => storedIdentitySigner() });
  return shared;
}

/** Hosts that own the session (sites, the native contract) install their own client once. */
export function setShopClient(client: ShopClient | null): void {
  shared = client;
}
