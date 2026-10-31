// from: shop@3a322cf9:app/src/components/AssetCard/AssetCard.tsx, app/src/components/AssetCard/AssetCard.styles.ts,
//       app/src/components/CreatorBadge/CreatorBadge.tsx;
//       shop@2d3bf62:app/src/hooks/useFavoriteCount.ts, app/src/components/CreatorName/CreatorName.tsx

import { useEffect, useState, type CSSProperties, type MouseEvent } from "react";
import { Link, useNavigate } from "react-router";

import { cartLineId, shopCart, useCart } from "../../cart/store";
import { fetchCatalogByIds } from "../../market/lib/catalog";
import { favoriteKey, fetchFavoriteStats } from "../../market/lib/favorites";
import { shopFavorites, useFavorites } from "../../market/store/favorites";
import { itemAsset, tokenAsset, type AssetId } from "../../model/asset";
import { parseEvmAddress } from "../../model/address";
import { SHOP_BASE } from "../../routes";
import { useShopWallet } from "../../wallet/context";
import { formatMana } from "../../model/money";
import { cardPrice } from "../cardPrice";
import { useCreatorProfile, useSaleActive } from "../hooks";
import { t } from "../intl";
import type { CatalogItem } from "../lib/catalogItem";
import { genderIcon } from "../lib/itemIcons";
import { isListingForSale } from "../lib/pricing";
import { rarityCardPalette, rarityDescription, rarityLabel } from "../lib/rarity";
import { detailRouteFor } from "../lib/routes";
import { saleDiscountPct, saleUnitsHint } from "../lib/sale";
import Icon from "./Icon";
import CategoryFlap from "./CategoryFlap";
import PriceTag, { formatCredits, formatCreditsFull } from "./PriceTag";
import SaleTag from "./SaleTag";
import { useShopQuery } from "../query";
import { publicThumbnail, publicThumbnailSet } from "../../../data/thumbnail";
import coin from "./assets/credits-coin.svg?url";
import "./assetcard.css";

export type CardSource = "grid" | "trending" | "deals" | "new_creations" | "suggested" | "related" | "recently_viewed" | "collection" | "creator" | "search";

export type AssetCardProps = {
  item: CatalogItem;
  mode?: "shop" | "view";
  source?: CardSource;
  position?: number;
  creatorName?: string | null;
};

export function assetOf(item: CatalogItem): AssetId | null {
  const contract = parseEvmAddress(item.contractAddress);
  if (!contract) return null;
  try {
    if (item.tokenId) return tokenAsset(item.chainId, contract, item.tokenId);
    if (item.itemId != null) return itemAsset(item.chainId, contract, item.itemId);
  } catch {
    return null;
  }
  return null;
}

export function cartInputFor(item: CatalogItem, source?: string) {
  const asset = assetOf(item);
  if (!asset) return null;
  const listing = { ...(item.tradeId ? { tradeId: item.tradeId } : {}), ...(item.acquisition ? { acquisition: item.acquisition } : {}) };
  return {
    asset,
    listing,
    display: {
      name: item.name,
      thumbnail: item.thumbnail || undefined,
      category: item.category,
      rarity: item.rarity,
      creator: item.creator || undefined,
      urn: item.urn,
      href: detailRouteFor(item) ?? undefined,
    },
    provenance: source ? { source } : {},
    available: item.tokenId ? 1 : (item.available ?? null),
  };
}

function CardFav({ item }: { item: CatalogItem }) {
  const { account } = useShopWallet();
  const favorites = useFavorites();
  const key = favoriteKey(item);
  const faved = !!key && !!favorites.items[key];
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    shopFavorites.reloadFor(account ?? null);
  }, [account]);
  const stats = useShopQuery(key ? ["favorite-count", key, account?.toLowerCase() ?? null] : null, () => fetchFavoriteStats(key!, account), { staleMs: 5 * 60_000 });
  if (!key) return null;
  const hydrating = favorites.status === "loading";
  const count = stats.data ? (hydrating || faved === stats.data.pickedByUser ? stats.data.count : Math.max(0, stats.data.count + (faved ? 1 : -1))) : undefined;
  const label = `${faved ? "Remove from favorites" : "Add to favorites"}${count !== undefined ? `, ${count} saved` : ""}`;
  const toggle = async (e: MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (busy || hydrating) return;
    setBusy(true);
    try {
      const owner = shopFavorites.getState().account;
      const saved = shopFavorites.getState().items[key] ?? (await fetchCatalogByIds([key]))[0];
      if (saved && shopFavorites.getState().account === owner) await shopFavorites.toggle(saved);
    } catch {
    } finally {
      setBusy(false);
    }
  };
  return (
    <button type="button" className="shop-card__fav" data-on={faved || undefined} data-count={count !== undefined ? count : undefined} data-testid="card-fav" aria-pressed={faved} aria-label={label} onClick={(e) => void toggle(e)}>
      <Icon name={faved ? "heart-solid" : "heart"} size={16} />
      {count !== undefined ? <span className="shop-card__favcount" data-testid="card-fav-count">{count.toLocaleString("en-US")}</span> : null}
    </button>
  );
}

function shortAddress(address: string): string {
  return /^0x[a-fA-F0-9]{40}$/.test(address) ? `${address.slice(0, 6)}…${address.slice(-4)}` : address;
}

function colorForAddress(address: string): string {
  let hash = 0;
  for (let i = 0; i < address.length; i++) hash = (hash * 31 + address.charCodeAt(i)) >>> 0;
  return `hsl(${hash % 360}, 52%, 45%)`;
}

// Held blank while the profile loads, so a fresh grid does not flash every creator's address first.
function CardCreator({ address, name }: { address: string; name?: string | null }) {
  const { profile, loading } = useCreatorProfile(address);
  const [broken, setBroken] = useState(false);
  const face = profile?.picture ?? null;
  useEffect(() => setBroken(false), [face]);
  if (loading && !name) return <span className="shop-card__sub">&nbsp;</span>;
  const own = profile?.name?.trim();
  const shown = name || (own ? own.charAt(0).toUpperCase() + own.slice(1) : shortAddress(address));
  const initial = (name || own || address.replace(/^0x/i, ""))[0]?.toUpperCase() ?? "?";
  return (
    <span className="shop-creator" data-testid="creator">
      {face && !broken ? (
        <img className="shop-creator__ava" data-avatar data-testid="creator-ava" src={face} alt="" loading="lazy" onError={() => setBroken(true)} />
      ) : (
        <span className="shop-creator__ava" data-avatar data-letter data-testid="creator-ava-letter" style={{ backgroundColor: colorForAddress(address) }} aria-hidden>
          {initial}
        </span>
      )}
      <span className="shop-creator__name" data-testid="creator-name">
        <span data-testid="creator-display">{shown}</span>
      </span>
    </span>
  );
}

export default function AssetCard({ item, mode = "shop", source = "grid", position, creatorName }: AssetCardProps) {
  const { session } = useShopWallet();
  const navigate = useNavigate();
  const lines = useCart((s) => s.lines);
  const [thumbFailures, setThumbFailures] = useState(0);
  const thumbSource = item.thumbnail ? (thumbFailures ? item.thumbnail : publicThumbnail(item.thumbnail, 320)) : undefined;
  const early = position !== undefined && position < 10;
  const price = cardPrice(item, session);
  const onSale = useSaleActive({ priceCredits: item.priceCredits, compareAtCredits: item.compareAtCredits, saleEndsAt: item.saleEndsAt });
  const showSale = onSale && price.kind === "credits" && price.compareAt != null;
  const detailPath = detailRouteFor(item);
  const input = cartInputFor(item, source);
  const lineId = input ? cartLineId(input.asset, input.listing) : null;
  const line = lineId ? lines.find((l) => l.id === lineId) : undefined;
  const stockCap = item.tokenId ? 1 : typeof item.available === "number" ? item.available : Infinity;
  const cartFull = !!line && line.quantity >= stockCap;
  const buyable = mode === "shop" && !!input && isListingForSale(item) && price.kind !== "soldOut" && price.kind !== "notForSale";
  const genderIco = genderIcon(item.gender);
  const palette = rarityCardPalette(item.rarity);
  const unitsLeft = showSale ? saleUnitsHint(item.saleUnitsLeft) : null;
  const network = item.chainId === 1 || item.chainId === 11155111 ? "ethereum" : "polygon";

  const add = (e: MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (!input || cartFull) return;
    shopCart.add(input);
    navigate(`${SHOP_BASE}/cart`);
  };

  const priceNode =
    price.kind === "credits" ? (
      showSale && price.compareAt ? (
        <div className="shop-card__price" data-variant="sale">
          <span className="shop-card__pricenow" data-testid="card-price-now" title={formatCreditsFull(price.credits)}>
            <img className="shop-card__coin" src={coin} alt="" aria-hidden />
            {formatCredits(price.credits)}
          </span>
          <span className="shop-card__pricewas" data-testid="card-price-was" title={formatCreditsFull(price.compareAt)}>
            {formatCredits(price.compareAt)}
          </span>
        </div>
      ) : (
        <div className="shop-card__price" data-testid="card-price" title={formatCreditsFull(price.credits)}>
          <img className="shop-card__coin" src={coin} alt="" aria-hidden />
          {formatCredits(price.credits)}
          {price.mana ? (
            <span className="shop-price__or" data-testid="card-price-mana">
              {t("assetCard.orMana", { amount: formatMana(price.mana, { group: true }) })}
            </span>
          ) : null}
        </div>
      )
    ) : (
      <PriceTag price={price} network={network} compactView />
    );

  return (
    <article className="shop-card" data-testid="card" data-sale={showSale || undefined} data-mode={mode} data-position={position}>
      {detailPath ? (
        <Link className="shop-card__link" to={detailPath} state={{ item }} aria-label={item.name} data-testid="card-link" />
      ) : null}
      <CardFav item={item} />
      <div className="shop-card__media" data-testid="card-media" style={{ background: palette.background, "--tile-stroke": palette.stroke } as CSSProperties}>
        <CategoryFlap item={item} testId="card-flap" />
        {showSale && price.kind === "credits" && price.compareAt ? <SaleTag pct={saleDiscountPct(price.compareAt, price.credits)} testId="card-sale-badge" /> : null}
        {item.tokenId || item.listingType === "secondary" ? <span className="shop-card__resale" data-testid="card-resale">{t("assetCard.resale")}</span> : null}
        {item.listingCount && item.listingCount > 1 ? (
          <span className="shop-card__count" data-testid="card-listing-count">
            {t("assetCard.listings", { count: item.listingCount })}
          </span>
        ) : null}
        {thumbSource && thumbFailures < 2 ? (
          <img className="shop-card__img" src={thumbSource} alt={item.name} loading={early ? "eager" : "lazy"} decoding="async"
            fetchPriority={early && source === "grid" && position! < 5 ? "high" : undefined}
            srcSet={thumbFailures ? undefined : publicThumbnailSet(item.thumbnail, [320, 640])} sizes="(max-width: 640px) 45vw, 240px"
            onError={() => setThumbFailures(n => (thumbSource === item.thumbnail ? 2 : n + 1))} />
        ) : null}
      </div>
      <div className="shop-card__body" data-row data-sale={showSale || undefined}>
        <div className="shop-card__top">
          <div className="shop-card__desc">
            <div className="shop-card__name" data-testid="card-name" title={item.name}>
              <span>{item.name}</span>
            </div>
            {item.issuedId ? (
              <div className="shop-card__sub" data-issued data-testid="card-issued">
                #{item.issuedId}
              </div>
            ) : item.creator ? (
              <div className="shop-card__creator" data-testid="card-author">
                <CardCreator address={item.creator} name={creatorName} />
              </div>
            ) : (
              <div className="shop-card__sub">&nbsp;</div>
            )}
          </div>
        </div>
        <div className="shop-card__action">
          <div className="shop-card__rest" data-chips>
            <div className="shop-card__chips">
              <span className="shop-card__chip" data-variant="rarity" data-testid="chip-rarity" style={{ background: palette.labelBg, color: palette.label }} title={rarityDescription(item.rarity)}>
                {rarityLabel(item.rarity)}
              </span>
              {item.isSmart ? (
                <span className="shop-card__chip" data-variant="smart" data-testid="chip-smart">
                  <Icon name="smart" size={13} />
                  {t("assetCard.smart")}
                </span>
              ) : null}
              {genderIco ? (
                <span className="shop-card__chip" data-variant="icon">
                  <Icon name={genderIco} />
                </span>
              ) : null}
              {item.emoteLoop !== undefined ? (
                <span className="shop-card__chip" data-variant="icon" data-testid="chip-play-mode" title={item.emoteLoop ? t("itemDetail.playLoop") : t("itemDetail.playOnce")}>
                  <Icon name={item.emoteLoop ? "play-loop" : "play-once"} />
                </span>
              ) : null}
            </div>
            {priceNode}
          </div>
          {buyable ? (
            <>
              <button
                type="button"
                className="shop-card__addround"
                data-testid="card-add-round"
                data-in={cartFull || undefined}
                onClick={add}
                disabled={cartFull}
                aria-label={cartFull ? `${t("itemDetail.inCart")}: ${item.name}` : `${t("itemDetail.addToCart")}: ${item.name}`}
              >
                <Icon name="plus" size={18} />
              </button>
              <button
                type="button"
                className="shop-card__cart"
                data-testid="card-cart"
                data-in={cartFull || undefined}
                onClick={add}
                disabled={cartFull}
                aria-label={cartFull ? `${t("itemDetail.inCart")}: ${item.name}` : `${t("itemDetail.addToCart")}: ${item.name}`}
              >
                <Icon name="cart" />
                <span>{cartFull ? t("itemDetail.inCart") : t("itemDetail.addToCart")}</span>
              </button>
            </>
          ) : null}
        </div>
        {unitsLeft != null ? <p className="shop-card__units">{t("assetCard.unitsLeftAtThisPrice", { count: unitsLeft })}</p> : null}
      </div>
    </article>
  );
}
