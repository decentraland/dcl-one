export type HttpMethod = "GET" | "POST" | "PUT" | "PATCH" | "DELETE";

export type EndpointAuth = "signed" | "optional" | "public";

export type Endpoint = {
  method: HttpMethod;
  path: string;
  legacy: string | null;
  auth: EndpointAuth;
};

const e = (method: HttpMethod, path: string, legacy: string | null, auth: EndpointAuth = "public"): Endpoint => ({
  method,
  path,
  legacy,
  auth,
});
const mk = (method: HttpMethod, path: string, auth: EndpointAuth = "public") => e(method, path, `/marketplace-api${path}`, auth);

export const ENDPOINTS = {
  purchaseOffers: e("POST", "/v1/purchase-offers", null, "optional"),
  catalogShop: mk("GET", "/v3/catalog/shop"),
  catalogUnified: mk("GET", "/v3/catalog/unified"),
  catalogItems: mk("GET", "/v3/catalog/items"),
  catalogLegacy: mk("GET", "/v3/catalog/legacy"),
  catalogRelated: mk("GET", "/v3/catalog/related"),
  catalogTrending: mk("GET", "/v3/catalog/trending"),
  catalogSuggested: mk("GET", "/v3/catalog/suggested", "optional"),
  catalogSuggest: mk("GET", "/v3/catalog/suggest"),
  catalogCreators: mk("GET", "/v3/catalog/creators"),
  catalogCreatorsSearch: mk("GET", "/v3/catalog/creators/search"),
  catalogImportable: mk("GET", "/v3/catalog/importable"),
  catalogV1: mk("GET", "/v1/catalog", "optional"),
  catalogV2: mk("GET", "/v2/catalog", "optional"),

  items: mk("GET", "/v1/items", "optional"),
  nfts: mk("GET", "/v1/nfts", "optional"),
  orders: mk("GET", "/v1/orders"),
  ordersOpenByItems: mk("GET", "/v1/orders/open-by-items"),
  bids: mk("GET", "/v1/bids"),
  trades: mk("GET", "/v1/trades"),
  trade: mk("GET", "/v1/trades/{id}"),
  tradeAccepted: mk("GET", "/v1/trades/{hashedSignature}/accept"),
  tradesMirror: mk("GET", "/v1/trades-mirror"),
  createTrade: mk("POST", "/v1/trades", "signed"),
  coupons: mk("GET", "/v1/coupons"),
  coupon: mk("GET", "/v1/coupons/{id}"),
  createCoupon: mk("POST", "/v1/coupons", "signed"),
  accounts: mk("GET", "/v1/accounts"),
  collections: mk("GET", "/v1/collections"),
  contracts: mk("GET", "/v1/contracts"),
  owners: mk("GET", "/v1/owners"),
  sales: mk("GET", "/v1/sales"),
  salesSummary: mk("GET", "/v1/sales/summary"),
  activity: mk("GET", "/v1/activity", "signed"),
  prices: mk("GET", "/v1/prices"),
  trendings: mk("GET", "/v1/trendings"),
  stats: mk("GET", "/v1/stats/{category}/{stat}"),
  rankings: mk("GET", "/v1/rankings/{entity}/{timeframe}"),
  volume: mk("GET", "/v1/volume/{timeframe}"),
  userWearables: mk("GET", "/v1/users/{address}/wearables"),
  userWearablesGrouped: mk("GET", "/v1/users/{address}/wearables/grouped"),
  userWearablesUrnToken: mk("GET", "/v1/users/{address}/wearables/urn-token"),
  userEmotes: mk("GET", "/v1/users/{address}/emotes"),
  userEmotesGrouped: mk("GET", "/v1/users/{address}/emotes/grouped"),
  userEmotesUrnToken: mk("GET", "/v1/users/{address}/emotes/urn-token"),
  userNames: mk("GET", "/v1/users/{address}/names"),
  userNamesOnly: mk("GET", "/v1/users/{address}/names/names-only"),

  lists: mk("GET", "/v1/lists", "signed"),
  list: mk("GET", "/v1/lists/{id}", "optional"),
  createList: mk("POST", "/v1/lists", "signed"),
  updateList: mk("PUT", "/v1/lists/{id}", "signed"),
  deleteList: mk("DELETE", "/v1/lists/{id}", "signed"),
  listPicks: mk("GET", "/v1/lists/{id}/picks", "optional"),
  addListPick: mk("POST", "/v1/lists/{id}/picks", "signed"),
  deleteListPick: mk("DELETE", "/v1/lists/{id}/picks/{itemId}", "signed"),
  grantListAccess: mk("POST", "/v1/lists/{id}/access", "signed"),
  revokeListAccess: mk("DELETE", "/v1/lists/{id}/access", "signed"),
  itemPicks: mk("GET", "/v1/picks/{itemId}", "optional"),
  itemPickStats: mk("GET", "/v1/picks/{itemId}/stats", "optional"),
  picksStats: mk("GET", "/v1/picks/stats"),
  pickUnpick: mk("POST", "/v1/picks/{itemId}", "signed"),
  pickDelete: mk("DELETE", "/v1/picks/{itemId}", "signed"),

  transakWidgetUrl: mk("POST", "/v1/transak/widget-url"),
  transakOrder: mk("GET", "/v1/transak/orders/{id}", "optional"),
  wertSign: mk("POST", "/v1/wert/sign", "optional"),

  userCredits: e("GET", "/v1/users/{address}/credits", "/credits/users/{address}/credits", "signed"),
  userCreditsStream: e("GET", "/v1/users/{address}/credits/stream", "/credits/users/{address}/credits/stream", "signed"),
  userCreditOrders: e("GET", "/v1/users/{address}/credit-orders", "/credits/users/{address}/credit-orders", "signed"),
  userPurchases: e("GET", "/v1/users/{address}/purchases", "/credits/users/{address}/purchases", "signed"),
  userProgress: e("GET", "/v1/users/{address}/progress", "/credits/users/{address}/progress", "signed"),
  userStatus: e("GET", "/v1/users/{address}/status", "/credits/users/{address}/status", "signed"),
  registerUser: e("POST", "/v1/users", "/credits/users", "signed"),
  unregisterUser: e("DELETE", "/v1/users/{address}", "/credits/users/{address}", "signed"),
  captcha: e("GET", "/v1/captcha", "/credits/captcha", "signed"),
  verifyCaptcha: e("POST", "/v1/captcha", "/credits/captcha", "signed"),
  creditSeason: e("GET", "/v1/credit-program/season", "/credits/season"),
  creditSeasons: e("GET", "/v1/credit-program/seasons", "/credits/seasons"),
  creditSeasonsV2: e("GET", "/v2/credit-program/seasons", "/credits/v2/seasons"),
  nameCreditRoute: e("GET", "/v1/names/credit-route", "/credits/credits-name-route", "signed"),

  packs: e("GET", "/v1/packs", "/credits/credits/packs"),
  createPackOrder: e("POST", "/v1/pack-orders", "/credits/credits/checkout", "signed"),
  packOrder: e("GET", "/v1/pack-orders/{orderId}", "/credits/credits/orders/{orderId}", "signed"),
  cancelPackOrder: e("POST", "/v1/pack-orders/{orderId}/cancel", "/credits/credits/orders/{orderId}/cancel", "signed"),
  resumePackOrder: e("POST", "/v1/pack-orders/{orderId}/resume", "/credits/credits/orders/{orderId}/resume", "signed"),
  packOrderCancelReturn: e("GET", "/v1/pack-orders/{orderId}/cancel-return", "/credits/credits/orders/{orderId}/cancel-return"),
  packPaymentIntent: e("POST", "/v1/packs/{sku}/payment-intents", "/credits/packs/{sku}/intent", "signed"),

  creditQuotes: e("POST", "/v1/credit-quotes", "/credits/prices/quote"),
  authorizeCredit: e("POST", "/v1/credit-authorizations", "/credits/credits/authorize", "signed"),
  authorizeCreditBatch: e("POST", "/v1/credit-authorizations/batch", "/credits/credits/authorize/batch", "signed"),
  cancelCreditAuthorizations: e("POST", "/v1/credit-authorizations/cancel", "/credits/credits/authorize/cancel", "signed"),
  reportCreditSubmission: e("POST", "/v1/credit-authorizations/submitted", "/credits/credits/authorize/submitted", "signed"),
  authorizePublication: e("POST", "/v1/credit-authorizations/publication", "/credits/credits/authorize-publication", "signed"),

  manaTopup: e("POST", "/v1/credit-topups/mana", "/credits/topup/mana", "signed"),
  manaTopupQuote: e("GET", "/v1/credit-topups/mana/quote", "/credits/topup/mana/quote"),
  manaTopupConfig: e("GET", "/v1/credit-topups/mana/config", "/transactions-api/v1/payments/config"),
  manaTopupNonce: e("GET", "/v1/credit-topups/mana/nonce/{address}", "/transactions-api/v1/payments/nonce/{address}"),
  manaTopupBalance: e("GET", "/v1/credit-topups/mana/balance/{address}", "/transactions-api/v1/payments/balance/{address}"),

  iapQuote: e("POST", "/v1/iap/quote", "/credits/credits/iap/quote", "signed"),
  iapRegister: e("POST", "/v1/iap/register", "/credits/credits/iap/register", "signed"),
  iapVerify: e("POST", "/v1/iap/verify", "/credits/credits/iap/verify", "signed"),

  cart: e("GET", "/v1/cart", "/credits/cart", "signed"),
  addCartItem: e("POST", "/v1/cart/items", "/credits/cart/items", "signed"),
  removeCartItem: e("DELETE", "/v1/cart/items/{collection}/{itemId}", "/credits/cart/items/{collection}/{itemId}", "signed"),
  createCheckout: e("POST", "/v1/checkouts", "/credits/checkout", "signed"),
  checkout: e("GET", "/v1/checkouts/{id}", "/credits/checkout/{id}", "signed"),

  outfits: e("GET", "/v1/outfits", null),
  outfitsAll: e("GET", "/v1/outfits/all", null, "signed"),
  outfit: e("GET", "/v1/outfits/{id}", null, "optional"),
  createOutfit: e("POST", "/v1/outfits", null, "signed"),
  updateOutfit: e("PUT", "/v1/outfits/{id}", null, "signed"),
  deleteOutfit: e("DELETE", "/v1/outfits/{id}", null, "signed"),
  uploadOutfitThumbnail: e("POST", "/v1/outfits/thumbnails", null, "signed"),
  outfitThumbnail: e("GET", "/v1/outfits/thumbnails/{hash}", null),

  notifyRequests: e("GET", "/v1/notify-requests", null, "signed"),
  createNotifyRequest: e("POST", "/v1/notify-requests", null, "signed"),
  deleteNotifyRequest: e("DELETE", "/v1/notify-requests", null, "signed"),

  rentalListings: e("GET", "/v1/rentals-listings", "/signatures-api/v1/rentals-listings"),
  createRentalListing: e("POST", "/v1/rentals-listings", "/signatures-api/v1/rentals-listings", "signed"),
  updateRentalListing: e("PATCH", "/v1/rentals-listings/{id}", "/signatures-api/v1/rentals-listings/{id}", "signed"),
  rentalListingPrices: e("GET", "/v1/rental-listings/prices", "/signatures-api/v1/rental-listings/prices"),
} as const satisfies Record<string, Endpoint>;

export type EndpointName = keyof typeof ENDPOINTS;

export const SHOP_API_PREFIX = "/shop-api";

export function templateParams(template: string): string[] {
  return [...template.matchAll(/\{([A-Za-z]+)\}/g)].map((m) => m[1]!);
}
