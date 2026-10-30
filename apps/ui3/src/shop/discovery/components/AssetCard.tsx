// from: shop@2d3bf62:app/src/components/AssetCard/AssetCard.tsx, app/src/components/AssetCard/AssetCard.styles.ts,
//       app/src/hooks/useFavoriteCount.ts, app/src/components/CreatorName/CreatorName.tsx

import { useEffect, useState, type MouseEvent } from "react";
import { Link, useNavigate } from "react-router";

import { cartLineId, shopCart, useCart } from "../../cart/store";
import { fetchCatalogByIds } from "../../market/lib/catalog";
import { favoriteKey, fetchFavoriteStats } from "../../market/lib/favorites";
import { shopFavorites, useFavorites } from "../../market/store/favorites";
import { itemAsset, tokenAsset, type AssetId } from "../../model/asset";
import { parseEvmAddress } from "../../model/address";
import { SHOP_BASE } from "../../routes";
import { useShopWallet } from "../../wallet/context";
import { cardPrice } from "../cardPrice";
import { useCreatorName, useSaleActive } from "../hooks";
import { t } from "../intl";
import type { CatalogItem } from "../lib/catalogItem";
import { categoryIcon, genderIcon } from "../lib/itemIcons";
import { isListingForSale } from "../lib/pricing";
import { rarityColor, rarityDescription, rarityLabel, rarityMedia } from "../lib/rarity";
import { detailRouteFor } from "../lib/routes";
import { saleDiscountPct, saleUnitsHint } from "../lib/sale";
import Icon from "./Icon";
import PriceTag from "./PriceTag";
import SaleTag from "./SaleTag";
import { useShopQuery } from "../query";
import { publicThumbnail, publicThumbnailSet } from "../../../data/thumbnail";
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
  const catIco = categoryIcon(item);
  const genderIco = genderIcon(item.gender);
  const unitsLeft = showSale ? saleUnitsHint(item.saleUnitsLeft) : null;
  const network = item.chainId === 1 || item.chainId === 11155111 ? "ethereum" : "polygon";
  const profileName = useCreatorName(creatorName || item.issuedId ? null : item.creator);
  const author = creatorName || profileName;

  const add = (e: MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (!input || cartFull) return;
    shopCart.add(input);
    navigate(`${SHOP_BASE}/cart`);
  };

  return (
    <article className="shop-card" data-testid="card" data-sale={showSale || undefined} data-mode={mode} data-position={position}>
      {detailPath ? (
        <Link className="shop-card__link" to={detailPath} state={{ item }} aria-label={item.name} data-testid="card-link" />
      ) : null}
      <CardFav item={item} />
      <div className="shop-card__media" data-testid="card-media" style={{ backgroundImage: rarityMedia(item.rarity) }}>
        {showSale && price.kind === "credits" && price.compareAt ? <SaleTag pct={saleDiscountPct(price.compareAt, price.credits)} testId="card-sale-badge" /> : null}
        {item.tokenId || item.listingType === "secondary" ? <span className="shop-card__resale" data-testid="card-resale">{t("assetCard.resale")}</span> : null}
        {item.listingCount && item.listingCount > 1 ? (
          <span className="shop-card__count" data-testid="card-listing-count">
            {t("assetCard.listings", { count: item.listingCount })}
          </span>
        ) : null}
        {thumbSource && thumbFailures < 2 ? (
          <img className="shop-card__img" src={thumbSource} alt="" loading={early ? "eager" : "lazy"} decoding="async"
            fetchPriority={early && source === "grid" && position! < 5 ? "high" : undefined}
            srcSet={thumbFailures ? undefined : publicThumbnailSet(item.thumbnail, [320, 640])} sizes="(max-width: 640px) 45vw, 240px"
            onError={() => setThumbFailures(n => (thumbSource === item.thumbnail ? 2 : n + 1))} />
        ) : null}
      </div>
      <div className="shop-card__body" data-sale={showSale || undefined}>
        <div className="shop-card__top">
          <div className="shop-card__desc">
            <span className="shop-card__name" data-testid="card-name" title={item.name}>
              {item.name}
            </span>
            {item.issuedId ? (
              <span className="shop-card__sub" data-issued data-testid="card-issued">
                #{item.issuedId}
              </span>
            ) : item.creator ? (
              <span className="shop-card__sub" data-testid="card-author">
                {author ? t("search.byCreator", { name: author }) : "\u00a0"}
              </span>
            ) : (
              <span className="shop-card__sub">&nbsp;</span>
            )}
          </div>
          <PriceTag price={price} network={network} />
        </div>
        <div className="shop-card__action">
          <div className="shop-card__chips" data-chips>
            <span className="shop-card__chip" data-variant="rarity" style={{ background: rarityColor(item.rarity) }} title={rarityDescription(item.rarity)}>
              {rarityLabel(item.rarity)}
            </span>
            {item.isSmart ? (
              <span className="shop-card__chip" data-variant="smart" data-testid="chip-smart">
                <Icon name="smart" size={13} />
                {t("assetCard.smart")}
              </span>
            ) : null}
            {catIco ? (
              <span className="shop-card__chip" data-variant="icon">
                <Icon name={catIco} size={14.6} />
              </span>
            ) : null}
            {genderIco ? (
              <span className="shop-card__chip" data-variant="icon">
                <Icon name={genderIco} size={14.6} />
              </span>
            ) : null}
            {item.emoteLoop !== undefined ? (
              <span className="shop-card__chip" data-variant="icon" data-testid="chip-play-mode" title={item.emoteLoop ? t("itemDetail.playLoop") : t("itemDetail.playOnce")}>
                <Icon name={item.emoteLoop ? "play-loop" : "play-once"} size={14.6} />
              </span>
            ) : null}
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
                <Icon name="cart" size={20} />
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
