// from: shop@2d3bf62:app/src/pages/ItemDetail.tsx, app/src/pages/ItemDetail.styles.ts;
//       marketplace@f3956826:webapp/src/components/Routes/Routes.tsx (`/contracts/:contractAddress/items/:itemId`,
//       `/contracts/:contractAddress/tokens/:tokenId`), webapp/src/modules/{item,nft}/actions.ts (FETCH_ITEM_REQUEST, FETCH_NFT_REQUEST),
//       webapp/src/modules/asset/actions.ts (FETCH_SMART_WEARABLE_REQUIRED_PERMISSIONS_REQUEST, FETCH_SMART_WEARABLE_VIDEO_HASH_REQUEST);
//       v2 ui3/src/marketplace/pages/MkAssetDetailView.tsx, MkAssetPage.tsx (superseded)
// Licence of the Marketplace parts: Apache-2.0, Copyright 2020 Metaverse Holdings Ltd.

import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { Link, useLocation, useNavigate, useParams } from "react-router";

import Button from "../../atoms/Button";
import Tooltip from "../../atoms/Tooltip";
import EmptyState from "../../components/EmptyState";
import { useCart } from "../cart/store";
import { itemAsset, tokenAsset, type AssetId } from "../model/asset";
import { parseEvmAddress, shortAddress, sameAddress } from "../model/address";
import { parseManaWei, parseUsdCents, type ManaWei, type UsdCents } from "../model/money";
import { offerQuote, offerSettlementKey, type CartListingRef, type PurchaseOffer } from "../model/purchase";
import type { CartAddInput } from "../cart/store";
import type { MixedPayment } from "../payment/present";
import type { PaymentMethod } from "../model/settlement";
import { SHOP_BASE, shopPath } from "../routes";
import { useShopWallet } from "../wallet/context";
import FavoriteButton from "../market/components/FavoriteButton";
import { shopClient } from "../api/shop";
import { fetchCatalogItems, fetchCollection, fetchCollectionItems, fetchRelatedItems, type UnifiedListing } from "../discovery/api";
import { cardEligibility } from "../discovery/cardPrice";
import { cartInputFor } from "../discovery/components/AssetCard";
import { CreatorBadge } from "../discovery/components/Hero";
import Icon from "../discovery/components/Icon";
import IndexNotice from "../discovery/components/IndexNotice";
import Rail from "../discovery/components/Rail";
import SaleTag from "../discovery/components/SaleTag";
import { OfferStock, SaleTimer } from "../discovery/components/SaleTimer";
import SuggestedForYouRow, { useSuggestedForYou } from "../discovery/components/SuggestedForYouRow";
import { suggestedHiddenReason } from "../discovery/lib/suggestionEvents";
import { indexUnavailable } from "../discovery/freshness";
import { shopFlags } from "../discovery/flags";
import { useSaleActive, useSeo } from "../discovery/hooks";
import { t } from "../discovery/intl";
import type { CatalogItem } from "../discovery/lib/catalogItem";
import { categoryHref, rarityHref, smartHref } from "../discovery/lib/chip-links";
import { categoryIcon, genderIcon } from "../discovery/lib/itemIcons";
import { isListingForSale } from "../discovery/lib/pricing";
import { MAX_SUPPLY, rarityColor, rarityDescription, rarityGlowCoreRgb, rarityGlowRgb, rarityLabel } from "../discovery/lib/rarity";
import { recordViewed } from "../discovery/lib/recently-viewed";
import { itemRoute, routeSegment, tokenRoute } from "../discovery/lib/routes";
import { saleDiscountPct, saleUnitsHint } from "../discovery/lib/sale";
import { mergeSuggestions, SUGGESTIONS_TARGET } from "../discovery/lib/suggestions";
import { invalidateShopQueries, useShopQuery } from "../discovery/query";
import {
  fetchClassicItemOrders,
  fetchItemDescription,
  fetchItemMeta,
  fetchItemResales,
  fetchOwnedItemCount,
  fetchOwnedToken,
  fetchTokenById,
  fetchUnifiedListingForItem,
} from "./api";
import type { LegacyOrderRef } from "./api";
import { fetchItemVideoUrl, fetchSmartWearableRequiredPermissions } from "./builder";
import ItemPreview from "./components/ItemPreview";
import LandBranch from "./components/LandBranch";
import CombinedBuy from "./components/CombinedBuy";
import LegacyOrderBuy from "./components/LegacyOrderBuy";
import NotifyMe from "./components/NotifyMe";
import { pdpText } from "./copy";
import PaymentPanel from "./components/PaymentPanel";
import ResaleOffers from "./components/ResaleOffers";
import { itemIdFromTokenId } from "./lib/token-id";
import { itemUrn } from "./lib/urn";
import { fetchVrmExportBlocked } from "./lib/wearable-rules";
import { offerQueryKey, offersRevalidation, useOffers, type OfferQuery } from "./offers";
import { useShopServices } from "../checkout/services";
import { readManaBalanceWei } from "../checkout/lib/mana";
import { usePurchaseLauncher } from "./purchase";
import { serviceBase } from "../../data/catalyst/client";
import "./itemdetail.css";

type RouteState = { item?: CatalogItem } | null;

const PERSONAL_RAIL_SIZE = 8;

const NOT_FOR_SALE_HINT = "This item isn't listed for sale right now. Get notified when it becomes available.";
function rarityTip(rarity: string): string {
  const max = MAX_SUPPLY[rarity.toLowerCase()];
  return max ? `${rarityLabel(rarity)} rarity \u2014 only ${max.toLocaleString("en-US")} will ever exist` : rarityDescription(rarity);
}

function genderLabel(gender: CatalogItem["gender"]): string | null {
  if (gender === "male") return t("itemDetail.genderMale");
  if (gender === "female") return t("itemDetail.genderFemale");
  if (gender === "unisex") return t("itemDetail.genderUnisex");
  return null;
}

function categoryLabel(item: Pick<CatalogItem, "category" | "wearableCategory">): string {
  if (item.wearableCategory) return item.wearableCategory.replace(/_/g, " ");
  return item.category === "emote" ? t("itemDetail.categoryEmote") : item.category === "wearable" ? t("itemDetail.categoryWearable") : item.category;
}

function stub(contractAddress: string, itemId: string | null, tokenId: string | null, chainId: number): CatalogItem {
  return {
    id: `${contractAddress}-${tokenId ?? itemId}`,
    name: "",
    creator: "",
    contractAddress,
    itemId,
    category: "wearable",
    rarity: "common",
    network: "MATIC",
    chainId,
    thumbnail: "",
    priceCredits: 0,
    gender: null,
    isSmart: false,
    ...(tokenId ? { tokenId } : {}),
  };
}

function balanceCents(body: unknown): UsdCents | null {
  const usd = (body as { usd?: { balanceCents?: unknown } } | null)?.usd?.balanceCents;
  const n = typeof usd === "bigint" ? Number(usd) : typeof usd === "string" ? Number(usd) : usd;
  return parseUsdCents(n);
}

type ReelPhoto = { id: string; url: string; thumbnailUrl: string; userName: string; userAddress: string; place: string; placeId: string; dateTime: string; people: number };
type ReelImage = {
  id: string;
  url: string;
  thumbnailUrl: string;
  metadata?: { userName?: string; userAddress?: string; dateTime?: string; placeId?: string; scene?: { name?: string }; visiblePeople?: Array<{ userName?: string; userAddress?: string; wearables?: string[] }> };
};
const REEL_PAGE = 50;
const REEL_SHOWN = 10;
const REEL_MIN = 2;
const REEL_TEXT = { title: "Spotted in Decentraland", subtitle: "See how the community styles it", prev: "Previous photos", next: "More photos" };

function rankReel(photos: ReelPhoto[]): ReelPhoto[] {
  const byAuthor = new Map<string, number>();
  const byPlace = new Map<string, number>();
  const seen = new Set<string>();
  const out: ReelPhoto[] = [];
  for (const p of [...photos].sort((a, b) => a.people - b.people || Number(b.dateTime) - Number(a.dateTime))) {
    const author = p.userAddress.toLowerCase();
    const place = p.placeId || p.place;
    if (seen.has(`${author}@${place}`) || (byAuthor.get(author) ?? 0) >= 2 || (byPlace.get(place) ?? 0) >= 2) continue;
    seen.add(`${author}@${place}`);
    byAuthor.set(author, (byAuthor.get(author) ?? 0) + 1);
    byPlace.set(place, (byPlace.get(place) ?? 0) + 1);
    out.push(p);
    if (out.length === REEL_SHOWN) break;
  }
  return out;
}

async function fetchItemReel(key: string, signal?: AbortSignal): Promise<ReelPhoto[]> {
  const res = await fetch(`${serviceBase("cameraReel")}/api/wearables/${key}/images?limit=${REEL_PAGE}`, { signal });
  if (!res.ok) throw new Error(`fetchItemReel ${res.status}`);
  const body = (await res.json()) as { images?: ReelImage[] };
  const photos = (body.images ?? []).flatMap((img): ReelPhoto[] => {
    const m = img.metadata;
    if (!m) return [];
    return [{ id: img.id, url: img.url, thumbnailUrl: img.thumbnailUrl, userName: m.userName ?? "", userAddress: m.userAddress ?? "", place: m.scene?.name ?? "", placeId: m.placeId ?? "", dateTime: m.dateTime ?? "", people: m.visiblePeople?.length ?? 0 }];
  });
  return rankReel(photos);
}

function photoDate(dateTime: string): string {
  const seconds = Number(dateTime);
  return Number.isFinite(seconds) && seconds > 0 ? new Intl.DateTimeFormat("en-US", { dateStyle: "medium" }).format(new Date(seconds * 1000)) : "";
}

function PhotoReel({ contractAddress, itemId }: { contractAddress: string; itemId: string | null }) {
  const key = contractAddress && itemId ? `${contractAddress.toLowerCase()}-${itemId}` : null;
  const q = useShopQuery(key ? ["item-reel", key] : null, (signal) => fetchItemReel(key!, signal), { staleMs: 10 * 60_000 });
  const track = useRef<HTMLDivElement>(null);
  const photos = q.data ?? [];
  if (photos.length < REEL_MIN) return null;
  const scroll = (dir: 1 | -1) => track.current?.scrollBy({ left: dir * track.current.clientWidth * 0.8, behavior: "smooth" });
  return (
    <section className="shop-reel" data-testid="photo-reel">
      <div className="shop-reel__head">
        <h2 className="shop-reel__title">{REEL_TEXT.title}</h2>
        <p className="shop-reel__sub">{REEL_TEXT.subtitle}</p>
      </div>
      <div className="shop-reel__strip">
        <div className="shop-reel__track" ref={track} data-testid="photo-reel-track">
          {photos.map((p) => {
            const date = photoDate(p.dateTime);
            return (
              <a key={p.id} className="shop-reel__tile" href={p.url} target="_blank" rel="noopener noreferrer" data-testid="photo-reel-shot" aria-label={`${p.userName} ${p.place}`.trim()}>
                <img src={p.thumbnailUrl} alt="" loading="lazy" />
                <span className="shop-reel__meta">
                  <span className="shop-reel__face" aria-hidden>{(p.userName.trim()[0] || "?").toUpperCase()}</span>
                  <span className="shop-reel__names">
                    <span className="shop-reel__who">{p.userName}</span>
                    <span className="shop-reel__where">
                      {p.place}
                      {date ? ` · ${date}` : ""}
                    </span>
                  </span>
                </span>
              </a>
            );
          })}
        </div>
        <button type="button" className="shop-reel__nav" data-side="left" onClick={() => scroll(-1)} aria-label={REEL_TEXT.prev}>
          <Icon name="carousel-arrow" size={48} />
        </button>
        <button type="button" className="shop-reel__nav" data-side="right" onClick={() => scroll(1)} aria-label={REEL_TEXT.next}>
          <Icon name="carousel-arrow" size={48} />
        </button>
      </div>
    </section>
  );
}

function ClampedDescription({ text }: { text: string }) {
  const ref = useRef<HTMLParagraphElement>(null);
  const [expanded, setExpanded] = useState(false);
  const [overflows, setOverflows] = useState(false);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () => {
      if (!el.hasAttribute("data-expanded")) setOverflows(el.scrollHeight > el.clientHeight + 1);
    };
    if (typeof ResizeObserver === "undefined") {
      measure();
      return;
    }
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  return (
    <>
      <p ref={ref} data-expanded={expanded || undefined}>{text}</p>
      {overflows || expanded ? (
        <button type="button" className="shop-detail__more" onClick={() => setExpanded((v) => !v)}>
          {expanded ? t("itemDetail.showLess") : t("itemDetail.readMore")}
        </button>
      ) : null}
    </>
  );
}

export function ItemDetailRoute() {
  const { pathname } = useLocation();
  return <ItemDetail key={pathname} />;
}

export default function ItemDetail() {
  const params = useParams<{ contractAddress: string; itemId?: string; tokenId?: string }>();
  const location = useLocation();
  const navigate = useNavigate();
  const wallet = useShopWallet();
  const { session, account } = wallet;
  const launch = usePurchaseLauncher();
  const flags = shopFlags();

  const contractAddress = (routeSegment(params.contractAddress) ?? "").toLowerCase();
  const routeItemId = routeSegment(params.itemId) ?? null;
  const routeTokenId = routeSegment(params.tokenId) ?? null;
  const isTokenRoute = !!routeTokenId;
  const pageItemId = routeItemId ?? itemIdFromTokenId(routeTokenId);
  const validContract = !!parseEvmAddress(contractAddress);

  const seed = useMemo<CatalogItem>(() => {
    const passed = (location.state as RouteState)?.item;
    const base = passed && sameAddress(passed.contractAddress, contractAddress) ? passed : stub(contractAddress, pageItemId, routeTokenId, 137);
    return isTokenRoute ? { ...base, tokenId: routeTokenId!, itemId: base.itemId ?? pageItemId } : { ...base, tokenId: undefined, itemId: routeItemId ?? base.itemId };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const listingQ = useShopQuery(!isTokenRoute && validContract && pageItemId ? ["shop-item", contractAddress, pageItemId] : null, (signal) => fetchUnifiedListingForItem(contractAddress, pageItemId!, { signal }), { staleMs: 0 });
  const metaQ = useShopQuery(validContract && pageItemId ? ["item-meta", contractAddress, pageItemId] : null, (signal) => fetchItemMeta(contractAddress, pageItemId!, { signal }), { staleMs: 5 * 60_000 });
  const tokenQ = useShopQuery(isTokenRoute && validContract ? ["token", contractAddress, routeTokenId] : null, (signal) => fetchTokenById(contractAddress, routeTokenId!, { signal }), { staleMs: 0 });
  const ownedQ = useShopQuery(isTokenRoute && validContract && account ? ["owned-token", contractAddress, routeTokenId, account] : null, (signal) => fetchOwnedToken(account!, contractAddress, routeTokenId!, { signal }), { staleMs: 0 });
  const descQ = useShopQuery(validContract && pageItemId ? ["item-desc", contractAddress, pageItemId] : null, (signal) => fetchItemDescription(contractAddress, pageItemId!, { signal }), { staleMs: 5 * 60_000 });
  const collectionQ = useShopQuery(validContract ? ["collection-meta", contractAddress] : null, (signal) => fetchCollection(contractAddress, { signal }), { staleMs: 5 * 60_000 });
  const resalesQ = useShopQuery(flags.secondarySales && validContract && pageItemId ? ["item-resales", contractAddress, pageItemId] : null, (signal) => fetchItemResales(contractAddress, pageItemId!, { signal }), { staleMs: 0 });
  const classicQ = useShopQuery(flags.secondarySales && validContract && pageItemId ? ["classic-orders", contractAddress, pageItemId] : null, (signal) => fetchClassicItemOrders(contractAddress, pageItemId!, { signal }), { staleMs: 0 });
  const ownedCountQ = useShopQuery(!isTokenRoute && validContract && account && routeItemId ? ["owned-count", contractAddress, routeItemId, account] : null, (signal) => fetchOwnedItemCount(account!, contractAddress, routeItemId!, { signal }), { staleMs: 30_000 });

  const listing = isTokenRoute ? null : (listingQ.data ?? null);
  const meta = metaQ.data ?? null;
  const token = ownedQ.data ?? tokenQ.data ?? null;
  const tokenResale: UnifiedListing | null = isTokenRoute ? ((resalesQ.data ?? []).find((r) => r.tokenId === routeTokenId) ?? null) : null;

  const current: CatalogItem = useMemo(() => {
    let item: CatalogItem = { ...seed };
    if (listing) item = { ...item, ...listing, tokenId: undefined, itemId: routeItemId ?? listing.itemId };
    if (token) {
      item = {
        ...item,
        id: token.id,
        name: item.name || token.name,
        contractAddress: token.contractAddress,
        itemId: token.itemId ?? item.itemId,
        tokenId: token.tokenId,
        issuedId: token.issuedId ?? item.issuedId,
        category: token.category || item.category,
        rarity: token.rarity ?? item.rarity,
        network: token.network,
        chainId: token.chainId,
        thumbnail: item.thumbnail || token.image,
        urn: item.urn ?? token.urn,
        seller: token.owner,
        tradeId: tokenResale?.tradeId ?? token.tradeId ?? undefined,
        listingType: "secondary",
        manaWei: tokenResale?.manaWei ?? token.orderPriceWei ?? null,
        priceCredits: 0,
        onSale: token.isOnSale || !!tokenResale,
        available: undefined,
      };
    } else if (tokenResale) {
      item = {
        ...item,
        id: tokenResale.id,
        name: item.name || tokenResale.name,
        contractAddress: tokenResale.contractAddress,
        itemId: tokenResale.itemId ?? item.itemId,
        tokenId: routeTokenId!,
        issuedId: tokenResale.issuedId ?? item.issuedId,
        category: tokenResale.category || item.category,
        rarity: tokenResale.rarity ?? item.rarity,
        network: tokenResale.network,
        chainId: tokenResale.chainId,
        thumbnail: item.thumbnail || tokenResale.thumbnail,
        urn: item.urn ?? tokenResale.urn,
        seller: tokenResale.seller,
        tradeId: tokenResale.tradeId,
        listingType: "secondary",
        manaWei: tokenResale.manaWei,
        priceCredits: 0,
        onSale: true,
        available: undefined,
      };
    }
    if (meta) {
      item = {
        ...item,
        name: item.name || meta.name || "",
        thumbnail: item.thumbnail || meta.thumbnail || "",
        isSmart: meta.isSmart || item.isSmart,
        urn: item.urn ?? meta.urn ?? undefined,
        wearableCategory: item.wearableCategory ?? meta.wearableCategory ?? undefined,
        gender: item.gender ?? meta.gender,
        creator: item.creator || meta.creator || "",
        rarity: item.rarity !== "common" ? item.rarity : (meta.rarity ?? item.rarity),
        category: meta.category && item.category === "wearable" && !item.name ? meta.category : item.category,
        chainId: token ? item.chainId : (meta.chainId ?? item.chainId),
        network: token ? item.network : (meta.network ?? item.network),
        available: isTokenRoute ? undefined : (item.available ?? meta.available ?? undefined),
      };
    }
    if (!item.creator && collectionQ.data?.creator) item = { ...item, creator: collectionQ.data.creator };
    return item;
  }, [seed, listing, token, tokenResale, meta, collectionQ.data, routeItemId, routeTokenId, isTokenRoute]);

  const asset: AssetId | null = useMemo(() => {
    const contract = parseEvmAddress(current.contractAddress);
    if (!contract) return null;
    try {
      if (isTokenRoute) return tokenAsset(current.chainId, contract, routeTokenId!);
      return pageItemId ? itemAsset(current.chainId, contract, pageItemId) : null;
    } catch {
      return null;
    }
  }, [current.contractAddress, current.chainId, isTokenRoute, routeTokenId, pageItemId]);

  const legacyOrder = isTokenRoute && !tokenResale?.tradeId ? (token?.legacyOrder ?? null) : null;
  const listingRef: CartListingRef = useMemo(
    () => ({
      ...(current.tradeId ? { tradeId: current.tradeId } : {}),
      ...(current.acquisition ? { acquisition: current.acquisition } : {}),
      ...(legacyOrder ? { orderId: legacyOrder.id } : {}),
    }),
    [current.tradeId, current.acquisition, legacyOrder],
  );
  const forSale = isTokenRoute ? !!current.onSale : isListingForSale(current);
  const queries: OfferQuery[] = useMemo(
    () => (asset && forSale ? [{ key: offerQueryKey(asset, listingRef), asset, listing: listingRef }] : []),
    [asset, forSale, listingRef],
  );
  const offers = useOffers(queries, session, wallet.revision);
  const services = useShopServices();
  const offer = queries[0] ? (offers.offers[queries[0].key] ?? null) : null;

  const creditsQ = useShopQuery(account && session.kind !== "signedOut" ? ["credits-balance", account] : null, (signal) => shopClient().credits.balance(account!, { signal }), { staleMs: 30_000 });
  const creditsBalance = creditsQ.data ? balanceCents(creditsQ.data) : null;
  const manaChain = (offer ? offerSettlementKey(offer, "mana")?.chainId : null) ?? current.chainId;
  const manaQ = useShopQuery(
    session.kind === "web3" && account ? ["mana-balance", account, manaChain] : null,
    () => readManaBalanceWei(account!, manaChain, services.reader),
    { staleMs: 30_000 },
  );
  const manaBalance = manaQ.data != null ? (manaQ.data as ManaWei) : null;

  const urnForTraits = meta?.urn ?? current.urn ?? itemUrn(current);
  const vrmQ = useShopQuery(urnForTraits && current.category === "wearable" ? ["item-vrm", urnForTraits] : null, (signal) => fetchVrmExportBlocked(urnForTraits!, signal), { staleMs: 30 * 60_000 });
  const videoQ = useShopQuery(current.isSmart && pageItemId ? ["item-video", contractAddress, pageItemId] : null, (signal) => fetchItemVideoUrl(contractAddress, pageItemId!, signal), { staleMs: 30 * 60_000 });
  const permissionsQ = useShopQuery(current.isSmart && urnForTraits ? ["item-permissions", urnForTraits] : null, (signal) => fetchSmartWearableRequiredPermissions(urnForTraits!, signal), { staleMs: 30 * 60_000 });

  const railAnchor = useMemo(() => ({ id: current.id, contractAddress, itemId: pageItemId, tokenId: routeTokenId ?? undefined }), [current.id, contractAddress, pageItemId, routeTokenId]);
  const collectionItemsQ = useShopQuery(validContract ? ["rail-collection", contractAddress] : null, (signal) => fetchCollectionItems(contractAddress, { first: 20 }, { signal }), { staleMs: 60_000 });
  const siblingsSettled = !!collectionItemsQ.data || !!collectionItemsQ.error;
  const collectionCount = useMemo(() => mergeSuggestions({ collection: collectionItemsQ.data?.items ?? [] }, railAnchor).items.length, [collectionItemsQ.data, railAnchor]);
  const needsPadding = siblingsSettled && collectionCount < SUGGESTIONS_TARGET;
  const railCreator = current.creator || null;
  const creatorItemsQ = useShopQuery(needsPadding && railCreator ? ["rail-creator", railCreator.toLowerCase()] : null, (signal) => fetchCatalogItems({ creator: railCreator!, isOnSale: true, first: 30 }, { signal }), { staleMs: 5 * 60_000 });
  const creatorSettled = !railCreator || !!creatorItemsQ.data || !!creatorItemsQ.error;
  const withCreatorCount = useMemo(
    () => mergeSuggestions({ collection: collectionItemsQ.data?.items ?? [], creator: creatorItemsQ.data?.items ?? [] }, railAnchor).items.length,
    [collectionItemsQ.data, creatorItemsQ.data, railAnchor],
  );
  const relatedQ = useShopQuery(
    needsPadding && creatorSettled && withCreatorCount < SUGGESTIONS_TARGET && validContract && pageItemId ? ["rail-related", contractAddress, pageItemId, SUGGESTIONS_TARGET] : null,
    (signal) => fetchRelatedItems(contractAddress, pageItemId!, { first: SUGGESTIONS_TARGET }, { signal }),
    { staleMs: 60_000 },
  );
  const rail = useMemo(
    () => mergeSuggestions({ collection: collectionItemsQ.data?.items ?? [], creator: creatorItemsQ.data?.items ?? [], related: relatedQ.data ?? [] }, railAnchor),
    [collectionItemsQ.data, creatorItemsQ.data, relatedQ.data, railAnchor],
  );

  const personalExclude = useMemo(() => (pageItemId ? [`${contractAddress}-${pageItemId}`] : undefined), [contractAddress, pageItemId]);
  const personal = useSuggestedForYou(PERSONAL_RAIL_SIZE, { exclude: personalExclude });
  const personalHidden = suggestedHiddenReason({
    enabled: personal.enabled,
    hasSignal: personal.hasSignal,
    isLoading: personal.isLoading,
    isError: personal.isError,
    personalized: personal.result?.personalized,
    rowCount: personal.result?.data?.length ?? 0,
  });

  const cartLines = useCart((s) => s.lines);
  const cardInput = cartInputFor(current, "item_detail");
  const cartInput = cardInput ? { ...cardInput, listing: listingRef } : null;
  const cartLine = cartInput ? cartLines.find((l) => l.asset.contract === cartInput.asset.contract && JSON.stringify(l.asset.token) === JSON.stringify(cartInput.asset.token) && (l.listing.tradeId ?? "") === (cartInput.listing.tradeId ?? "")) : undefined;
  const stockCap = isTokenRoute ? 1 : typeof current.available === "number" ? current.available : Infinity;
  const atStockCap = !!cartLine && cartLine.quantity >= stockCap;

  const saleActive = useSaleActive({ priceCredits: current.priceCredits, compareAtCredits: current.compareAtCredits, saleEndsAt: current.saleEndsAt });
  const [resaleListOpen, setResaleListOpen] = useState(false);
  const [buyingOrder, setBuyingOrder] = useState<{ order: LegacyOrderRef; quoted: ManaWei | null } | null>(null);
  const [buyingMixed, setBuyingMixed] = useState<{ line: CartAddInput; offer: PurchaseOffer; mixed: MixedPayment } | null>(null);
  const viewed = useRef<string | null>(null);
  useEffect(() => {
    if (!current.name || viewed.current === current.id) return;
    viewed.current = current.id;
    recordViewed(current);
  }, [current]);

  const identityError = isTokenRoute ? tokenQ.error : listingQ.error && metaQ.error ? metaQ.error : null;
  const unavailable = indexUnavailable(identityError) ?? indexUnavailable(listingQ.error);
  const stillResolving = (isTokenRoute ? tokenQ.isLoading : listingQ.isLoading || metaQ.isLoading) && !current.name;
  const notFound = !validContract || (!current.name && !stillResolving && !identityError);
  useSeo({ title: notFound ? t("itemDetail.notAvailableTitle") : current.name || null });

  if (notFound) {
    return (
      <div className="shop-page" data-testid="item-not-found">
        <EmptyState
          icon={<Icon name="cart" size={44} />}
          title={t("itemDetail.notAvailableTitle")}
          subtitle={t("itemDetail.notAvailableBody")}
          actions={[{ label: t("notFound.cta"), onClick: () => navigate(`${SHOP_BASE}/items`) }]}
        />
      </div>
    );
  }
  if (identityError && !current.name) {
    return (
      <div className="shop-page" data-testid="item-error">
        {unavailable ? (
          <IndexNotice freshness={unavailable} what={t("itemDetail.itemFallback")} onRetry={() => (isTokenRoute ? tokenQ.refetch() : (listingQ.refetch(), metaQ.refetch()))} />
        ) : (
          <div className="shop-page__error" role="alert">
            <p>{t("itemDetail.loadError")}</p>
            <Button variant="secondary" size="sm" onClick={() => (isTokenRoute ? tokenQ.refetch() : (listingQ.refetch(), metaQ.refetch()))}>
              {t("loadMore.retry")}
            </Button>
          </div>
        )}
      </div>
    );
  }

  const rarity = current.rarity || "common";
  const gender = genderLabel(current.gender);
  const catIco = categoryIcon(current);
  const genderIco = genderIcon(current.gender);
  const itemHref = isTokenRoute ? tokenRoute(contractAddress, routeTokenId!) : itemRoute(contractAddress, pageItemId ?? "");
  const eligibility = cardEligibility(current, session);
  const listingPrice = eligibility
    ? {
        eligibility,
        credits: eligibility.allowed.includes("credits") && current.priceCredits > 0 ? current.priceCredits : null,
        mana: parseManaWei(current.manaWei ?? null),
        chainId: current.chainId,
      }
    : null;
  const showCompareAt = saleActive && !!eligibility?.allowed.includes("credits") && current.compareAtCredits != null;
  const supply = isTokenRoute ? null : (current.available ?? null);
  const maxSupply = MAX_SUPPLY[rarity.toLowerCase()];
  const outOfStock = !isTokenRoute && supply === 0;
  const unitsLeft = showCompareAt ? saleUnitsHint(current.saleUnitsLeft) : null;
  const owns = isTokenRoute && !!ownedQ.data;
  const ownedCount = ownedCountQ.data ?? 0;
  const description = descQ.data ?? "";
  const utility = meta?.utility ?? null;
  const network = current.chainId === 1 || current.chainId === 11_155_111 ? "ethereum" : "polygon";
  const resales = (resalesQ.data ?? []).filter((r) => r.tokenId !== routeTokenId || !isTokenRoute);
  const params2 = { contractAddress, tokenId: routeTokenId ?? "" };

  const orderQuote = offer ? offerQuote(offer, "mana") : null;
  const onBuy = (method: PaymentMethod, _offer?: unknown, mixed?: MixedPayment) => {
    if (!cartInput) return;
    if (mixed && method === "mana" && offer) {
      setBuyingMixed({ line: cartInput, offer, mixed });
      return;
    }
    if (legacyOrder && method === "mana" && offer && offerSettlementKey(offer, "mana")?.call.kind === "one") {
      setBuyingOrder({ order: legacyOrder, quoted: orderQuote?.method === "mana" ? orderQuote.amount : null });
      return;
    }
    launch({ line: cartInput, method, offer }, (to) => navigate(to));
  };
  const onAddToCart = (method: PaymentMethod | null) => {
    if (!cartInput) return;
    launch({ line: cartInput, method, offer }, (to) => navigate(to));
  };

  const favItemId = pageItemId && current.name && !["parcel", "estate"].includes(current.category) ? pageItemId : null;
  const hasResales = resales.length > 0 || (classicQ.data?.length ?? 0) > 0;
  const isLand = current.category === "parcel" || current.category === "estate";
  const canBuyResale = flags.secondarySales && !!pageItemId && !isLand;
  const showResales = canBuyResale && ((hasResales && !isTokenRoute) || resaleListOpen);
  const bidHref = isTokenRoute ? shopPath("tokenBid", params2) : shopPath("itemBid", { contractAddress, itemId: pageItemId ?? "" });
  const stock = supply !== null && maxSupply ? { supply, max: maxSupply } : null;
  const siblings = collectionItemsQ.data?.items ?? [];
  const sibling = pageItemId ? siblings.find((s) => s.itemId === pageItemId) : undefined;
  const traitRows = [current, meta, sibling].filter(Boolean) as Array<Pick<CatalogItem, "emoteLoop" | "emoteHasSound" | "emoteHasProps">>;
  const emoteTraits = {
    emoteLoop: traitRows.find((r) => r.emoteLoop != null)?.emoteLoop,
    emoteHasSound: traitRows.find((r) => r.emoteHasSound !== undefined)?.emoteHasSound,
    emoteHasProps: traitRows.find((r) => r.emoteHasProps !== undefined)?.emoteHasProps,
  };
  const showCreator = !!current.creator && network !== "ethereum" && (!isTokenRoute || isLand);
  const showOwner = isTokenRoute && isLand && !!current.seller;
  const collectionThumbs = siblings.map((s) => s.thumbnail).filter(Boolean).slice(0, 4);

  return (
    <div className="shop-page shop-detail shop-pdp" data-testid="item-detail" data-route={isTokenRoute ? "token" : "item"}>
      <nav className="shop-pdp__crumbs" aria-label={t("itemDetail.breadcrumbAria")}>
        <Link to={`${SHOP_BASE}/items`}>{t("nav.collectibles")}</Link>
        <span aria-hidden>/</span>
        <span aria-current="page">{current.name || t("itemDetail.itemFallback")}</span>
      </nav>

      <div className="shop-detail__main" style={{ "--glow-rgb": rarityGlowRgb(rarity), "--glow-core": rarityGlowCoreRgb(rarity) } as CSSProperties}>
        <div className="shop-detail__preview">
          {current.name ? <ItemPreview item={current} /> : <div className="shop-preview" aria-busy="true" aria-label={t("itemDetail.previewLoading")} />}
          {favItemId ? <FavoriteButton contractAddress={contractAddress} itemId={favItemId} name={current.name} placement="preview" /> : null}
          <div className="shop-detail__preview-actions">
            {videoQ.data ? (
              <a className="shop-pdp__pill" href={videoQ.data} target="_blank" rel="noopener noreferrer" data-testid="play-showcase">
                <Icon name="play" size={18} /> {t("itemDetail.playShowcase")}
              </a>
            ) : null}
          </div>
        </div>

        <div className="shop-detail__info" data-testid="item-info">
          {!current.name ? (
            <div className="shop-pdp__skel" aria-hidden>
              <span className="shop-pdp__skel-title" />
              <div className="shop-pdp__skel-chips">
                <span />
                <span />
                <span />
              </div>
              <span className="shop-pdp__skel-line" />
              <span className="shop-pdp__skel-line" />
              <span className="shop-pdp__skel-line" data-short />
              <hr className="shop-detail__divider" />
              <span className="shop-pdp__skel-price" />
              <span className="shop-pdp__skel-btn" />
            </div>
          ) : (
            <>
              {showCompareAt ? <SaleTag pct={saleDiscountPct(current.compareAtCredits!, current.priceCredits)} size="lg" testId="detail-sale-badge" /> : null}
              <div className="shop-pdp__head">
                <h1 className="shop-pdp__title">
                  {current.name}
                  {isTokenRoute && current.issuedId ? ` #${current.issuedId}` : ""}
                </h1>
                {favItemId ? <FavoriteButton contractAddress={contractAddress} itemId={favItemId} name={current.name} placement="title" /> : null}
              </div>
              <div className="shop-detail__chips">
                <Tooltip label={rarityTip(rarity)} side="top" className="shop-pdp__tip">
                  {rarityHref(rarity) ? (
                    <Link className="shop-detail__chip" data-variant="rarity" to={rarityHref(rarity)!} style={{ background: rarityColor(rarity) }} data-testid="detail-rarity-link">
                      {rarityLabel(rarity)}
                    </Link>
                  ) : (
                    <span className="shop-detail__chip" data-variant="rarity" tabIndex={0} style={{ background: rarityColor(rarity) }}>
                      {rarityLabel(rarity)}
                    </span>
                  )}
                </Tooltip>
                {categoryHref(current) ? (
                  <Link className="shop-detail__chip" to={categoryHref(current)!} title={t("filter.browseByCategory", { category: categoryLabel(current) })} data-testid="detail-category-link">
                    {catIco ? <Icon name={catIco} size={18} /> : null}
                    {categoryLabel(current)}
                  </Link>
                ) : (
                  <span className="shop-detail__chip">
                    {catIco ? <Icon name={catIco} size={18} /> : null}
                    {categoryLabel(current)}
                  </span>
                )}
                {gender ? (
                  <span className="shop-detail__chip">
                    {genderIco ? <Icon name={genderIco} size={18} /> : null}
                    {gender}
                  </span>
                ) : null}
                {current.isSmart ? (
                  <Link className="shop-detail__chip" to={smartHref()} title={t("filter.browseBySmart")} data-testid="detail-smart">
                    <Icon name="smart" size={18} />
                    {t("itemDetail.smart")}
                  </Link>
                ) : null}
                {emoteTraits.emoteLoop != null ? (
                  <span className="shop-detail__chip" data-testid="detail-play-mode">
                    <Icon name={emoteTraits.emoteLoop ? "play-loop" : "play-once"} size={18} />
                    {emoteTraits.emoteLoop ? t("itemDetail.playLoop") : t("itemDetail.playOnce")}
                  </span>
                ) : null}
                {emoteTraits.emoteHasSound ? (
                  <span className="shop-detail__chip" data-testid="detail-sound">
                    <Icon name="sound" size={18} />
                    {t("itemDetail.emoteSound")}
                  </span>
                ) : null}
                {emoteTraits.emoteHasProps ? (
                  <span className="shop-detail__chip" data-testid="detail-props">
                    <Icon name="props" size={18} />
                    {t("itemDetail.emoteProps")}
                  </span>
                ) : null}
                {vrmQ.data ? (
                  <span className="shop-detail__chip" data-variant="blocked" tabIndex={0} title={t("itemDetail.exportBlockedTooltip")} aria-label={t("itemDetail.exportBlockedTooltip")} data-testid="detail-export-blocked">
                    <Icon name="ban" size={16} />
                    {t("itemDetail.exportBlocked")}
                  </span>
                ) : null}
                {utility ? (
                  <span className="shop-detail__chip" data-testid="detail-utility-chip">
                    <Icon name="utility" size={18} />
                    {t("itemDetail.utility")}
                  </span>
                ) : null}
              </div>

              {description || utility ? (
                <div className="shop-detail__desc">
                  {description ? (
                    <div className="shop-pdp__col">
                      <h2 className="shop-detail__label">{t("itemDetail.description")}</h2>
                      <ClampedDescription key={description} text={description} />
                    </div>
                  ) : null}
                  {utility ? (
                    <div className="shop-pdp__col" data-testid="detail-utility">
                      <h2 className="shop-detail__label">{t("itemDetail.utility")}</h2>
                      <p data-expanded>{utility}</p>
                    </div>
                  ) : null}
                  {permissionsQ.data?.length ? (
                    <ul className="shop-detail__permissions" data-testid="detail-permissions">
                      {permissionsQ.data.map((p) => (
                        <li key={p}>{p}</li>
                      ))}
                    </ul>
                  ) : null}
                </div>
              ) : null}

              {showCreator || collectionQ.data?.name || showOwner ? (
                <div className="shop-detail__meta">
                  {showCreator ? (
                    <div className="shop-pdp__col">
                      <h2 className="shop-detail__label">{t("itemDetail.creator")}</h2>
                      <CreatorBadge address={current.creator} />
                    </div>
                  ) : null}
                  {collectionQ.data?.name ? (
                    <div className="shop-pdp__col">
                      <h2 className="shop-detail__label">{t("itemDetail.collection")}</h2>
                      <Link className="shop-badge shop-pdp__collection" to={`${SHOP_BASE}/collection/${contractAddress}`} data-testid="collection-badge">
                        <span className="shop-badge__avatar" data-count={collectionThumbs.length || undefined}>
                          {collectionThumbs.map((src) => (
                            <img key={src} src={src} alt="" loading="lazy" />
                          ))}
                        </span>
                        <span className="shop-badge__name">{collectionQ.data.name}</span>
                      </Link>
                    </div>
                  ) : null}
                  {showOwner ? (
                    <div className="shop-pdp__col">
                      <h2 className="shop-detail__label">{t("itemDetail.owner")}</h2>
                      <span className="shop-pdp__owner" data-testid="detail-owner">{shortAddress(current.seller!)}</span>
                    </div>
                  ) : null}
                </div>
              ) : null}

              <hr className="shop-detail__divider" />

              {showCompareAt ? (
                <div className="shop-detail__sale">
                  <span className="shop-detail__was" data-testid="detail-price-was">
                    {t("assetCard.credits", { count: current.compareAtCredits! })}
                  </span>
                  <SaleTimer until={current.saleEndsAt} testId="detail-countdown" />
                  {current.coupon?.checks?.uses ? <OfferStock claimed={current.coupon.used} total={Number(current.coupon.checks.uses)} testId="detail-offer-stock" /> : null}
                  {unitsLeft != null ? <p className="shop-detail__units">{t("assetCard.unitsLeftAtThisPrice", { count: unitsLeft })}</p> : null}
                </div>
              ) : null}

              {owns ? (
                <div className="shop-detail__manage" data-testid="manage-actions">
                  <p>{t("itemDetail.ownToken")}</p>
                  <div className="shop-detail__manage-links">
                    {ownedQ.data?.isOnSale ? (
                      <Link data-variant="scrim" to={shopPath("tokenCancel", params2)}>{t("itemDetail.manageCancel")}</Link>
                    ) : (
                      <Link data-variant="dark" to={shopPath("tokenSell", params2)}>{t("itemDetail.manageSell")}</Link>
                    )}
                    <Link data-variant="scrim" to={shopPath("tokenTransfer", params2)}>{t("itemDetail.manageTransfer")}</Link>
                    <div className="shop-detail__manage-more">
                      <Link to={shopPath("tokenBids", params2)}>{t("itemDetail.bids")}</Link>
                      <Link to={shopPath("tokenManage", params2)}>{t("itemDetail.manage")}</Link>
                    </div>
                  </div>
                </div>
              ) : forSale && !outOfStock ? (
                <PaymentPanel
                  offer={offer}
                  status={offers.status}
                  listing={listingPrice}
                  session={session}
                  wallet={{
                    account: wallet.account,
                    walletAccount: wallet.walletAccount,
                    chainId: wallet.chainId,
                    pendingChainId: wallet.pendingChainId,
                    accountMismatch: wallet.accountMismatch,
                    onSignIn: wallet.host?.onSignIn,
                    onConnectWallet: wallet.host?.onConnectWallet,
                    onSwitchAccount: wallet.host?.onSwitchAccount,
                    onSwitchNetwork: (id) => void wallet.switchTo(id),
                  }}
                  item={{ name: current.name, href: itemHref }}
                  creditsBalance={creditsBalance}
                  manaBalance={manaBalance}
                  inCart={!!cartLine && isTokenRoute}
                  atStockCap={atStockCap}
                  stock={stock}
                  setupUrl={services.walletSetupUrl}
                  revalidation={offersRevalidation(offers.status, offers.retry)}
                  onRetry={offers.retry}
                  onBuy={onBuy}
                  onAddToCart={onAddToCart}
                />
              ) : (
                <>
                  <div className="shop-pay__block shop-pdp__nfs">
                    <div className="shop-pay__row">
                      <div className="shop-pay__pricecol">
                        <div className="shop-pay__price" data-variant="none" data-testid="item-price">
                          <span>{outOfStock ? t("itemDetail.soldOut") : t("itemDetail.notForSale")}</span>
                          <Tooltip label={NOT_FOR_SALE_HINT} side="top" className="shop-pdp__tip">
                            <span className="shop-pay__info" tabIndex={0} role="img" aria-label={NOT_FOR_SALE_HINT}>
                              <Icon name="info" size={14} />
                            </span>
                          </Tooltip>
                        </div>
                      </div>
                      {stock ? (
                        <div className="shop-pay__stockcol" data-testid="detail-stock">
                          <span className="shop-pay__caption">{t("itemDetail.stock")}</span>{" "}
                          <span className="shop-pay__stock">
                            {stock.supply.toLocaleString("en-US")}/{stock.max.toLocaleString("en-US")}
                          </span>
                          {outOfStock ? (
                            <span className="shop-pay__stock" data-out data-testid="out-of-stock">
                              {t("itemDetail.outOfStock")}
                            </span>
                          ) : null}
                        </div>
                      ) : null}
                    </div>
                  </div>
                  <div className="shop-pdp__ctas">
                    {pageItemId ? <NotifyMe contractAddress={contractAddress} itemId={pageItemId} chainId={current.chainId} /> : null}
                    {canBuyResale ? (
                      <button
                        type="button"
                        className="shop-pdp__textlink"
                        data-testid="buy-resale"
                        aria-controls="shop-resales"
                        onClick={() => {
                          setResaleListOpen(true);
                          requestAnimationFrame(() => document.getElementById("shop-resales")?.scrollIntoView({ behavior: "smooth", block: "start" }));
                        }}
                      >
                        {pdpText("itemDetail.buyResale")}
                      </button>
                    ) : null}
                  </div>
                </>
              )}

              {!owns && isLand ? (
                <p className="shop-detail__offer">
                  <Link className="shop-pdp__textlink" to={bidHref} data-testid="make-offer">
                    {t("itemDetail.placeBid")}
                  </Link>
                </p>
              ) : null}

              {!isTokenRoute && ownedCount > 0 ? (
                <p className="shop-detail__own" data-testid="you-own-note">
                  {t("itemDetail.youOwnN", { count: ownedCount })} <Link to={`${SHOP_BASE}/my-items`}>{t("nav.myAssets")}</Link>
                </p>
              ) : null}
            </>
          )}
        </div>
      </div>

      {isTokenRoute && (current.category === "parcel" || current.category === "estate") ? (
        <LandBranch contractAddress={contractAddress} tokenId={routeTokenId!} viewer={account} />
      ) : null}

      {showResales ? (
        <ResaleOffers contractAddress={contractAddress} resales={resales} classic={classicQ.data ?? []} network={network} currentTokenId={routeTokenId ?? undefined} viewer={account} />
      ) : null}

      <SuggestedForYouRow exclude={personalExclude} title={pdpText("itemDetail.becauseYouOwn")} first={PERSONAL_RAIL_SIZE} />
      {personalHidden !== null && rail.items.length > 0 ? (
        <Rail
          title={rail.isCollectionOnly ? t("itemDetail.moreFromCollection") : t("itemDetail.youMightAlsoLike")}
          items={rail.items}
          source="related"
          viewAllTo={rail.isCollectionOnly ? `${SHOP_BASE}/collection/${contractAddress}` : undefined}
          testId="detail-rail"
        />
      ) : null}

      {!isLand ? <PhotoReel contractAddress={contractAddress} itemId={pageItemId} /> : null}

      {buyingMixed ? (
        <CombinedBuy
          line={buyingMixed.line}
          offer={buyingMixed.offer}
          mixed={buyingMixed.mixed}
          onClose={() => setBuyingMixed(null)}
          onDone={() => invalidateShopQueries()}
        />
      ) : null}
      {buyingOrder && routeTokenId ? (
        <LegacyOrderBuy
          name={current.name}
          contractAddress={contractAddress}
          tokenId={routeTokenId}
          order={buyingOrder.order}
          quoted={buyingOrder.quoted}
          onClose={() => setBuyingOrder(null)}
          onDone={() => invalidateShopQueries()}
        />
      ) : null}
    </div>
  );
}
