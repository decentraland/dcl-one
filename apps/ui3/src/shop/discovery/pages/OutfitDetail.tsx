// from: shop@2d3bf62:app/src/pages/OutfitDetail.tsx, app/src/pages/OutfitDetail.styles.ts,
//       app/src/hooks/useOutfits.ts (useOutfitItems, useOutfitCart), app/src/lib/outfits.ts (classifyOutfitItem)

import { lazy, Suspense, useEffect, useMemo, useState } from "react";
import { Link, useParams } from "react-router";

import { isShopApiError } from "../../api/client";
import { shopCart, useCart } from "../../cart/store";
import { BASE_FEMALE, BASE_MALE, requiredShape } from "../../detail/lib/bodyShape";
import { fetchEmoteClipUrl } from "../../detail/lib/emoteBase64";
import { itemUrn } from "../../detail/lib/urn";
import { assetKey } from "../../model/asset";
import { SHOP_BASE } from "../../routes";
import { useShopWallet } from "../../wallet/context";
import { fetchCatalogByIds, fetchShopItems } from "../api";
import { cardEligibility, cardPrice } from "../cardPrice";
import { assetOf, cartInputFor } from "../components/AssetCard";
import Icon from "../components/Icon";
import IndexNotice from "../components/IndexNotice";
import PriceTag, { formatCreditsFull } from "../components/PriceTag";
import { indexUnavailable } from "../freshness";
import { useCreatorName, useSeo } from "../hooks";
import { t } from "../intl";
import type { CatalogItem } from "../lib/catalogItem";
import { categoryIcon, genderIcon } from "../lib/itemIcons";
import { isWearable, playingEmote } from "../lib/outfit";
import { rarityColor, rarityDescription, rarityLabel } from "../lib/rarity";
import { detailRouteFor } from "../lib/routes";
import { useShopQuery } from "../query";
import { classifyOutfitItem, fetchOutfit, isListingUnavailable, outfitItemKey, outfitRadialGradient, thumbnailUrl, type Outfit, type OutfitItemState } from "./outfits";
import "./outfitdetail.css";

export { fetchOutfit, type Outfit } from "./outfits";

const WearablePreview = lazy(() => import("../../../wearable-preview/WearablePreview"));

const COPY = {
  notFoundTitle: "This outfit isn't available",
  notFoundBody: "It may no longer exist. Browse the shop to find your next look.",
  notFoundCta: "Browse Collectibles",
  generic: "Something went wrong. Please try again.",
  retry: "Try again",
  resolveError: "We couldn't load this outfit's items.",
  unavailable: "No longer available",
  yourListing: "Your listing",
  yourListingHint: "You're selling some of these items, so they don't count toward the total.",
  inCart: "In your cart",
  totalPrice: "Total price",
  looks: "Looks",
  adding: "Adding…",
  nothingToAdd: "Added to cart",
  addFailed: "We couldn't check these items right now. Nothing was added — please try again.",
  noneLeft: "These items just sold out — nothing was added to your cart",
  itemsCount: (n: number) => `${n} ${n === 1 ? "item" : "items"} in this outfit`,
  addCount: (n: number) => `Add ${n} ${n === 1 ? "item" : "items"} to cart`,
};

function webglAvailable(): boolean {
  return typeof window !== "undefined" && typeof (window as { WebGLRenderingContext?: unknown }).WebGLRenderingContext !== "undefined";
}

export default function OutfitDetail() {
  const { id } = useParams<{ id: string }>();
  const outfitQ = useShopQuery(id ? ["outfit", id] : null, (signal) => fetchOutfit(id!, signal), { staleMs: 60_000 });
  const outfit = outfitQ.data ?? null;
  const notFound = !id || (outfitQ.data === null && !outfitQ.isLoading && !outfitQ.error);
  useSeo({ title: outfit?.name ?? (notFound ? "Outfit not available" : null) });

  const serviceMissing = isShopApiError(outfitQ.error) && outfitQ.error.code === "noLegacyAlias";
  const unavailable = indexUnavailable(outfitQ.error);

  if (serviceMissing || unavailable) {
    return (
      <div className="shop-page shop-outfit" data-testid="outfit-unavailable">
        <IndexNotice freshness={unavailable ?? { kind: "unavailable", source: "outfits", message: t("outfit.unavailableService"), configure: null }} what={t("outfit.title")} onRetry={outfitQ.refetch} />
      </div>
    );
  }
  if (notFound) {
    return (
      <div className="shop-page shop-outfit shop-outfit__empty" data-testid="outfit-notfound">
        <Icon name="cart" size={44} className="shop-outfit__empty-ico" />
        <h1 className="shop-outfit__empty-title">{COPY.notFoundTitle}</h1>
        <p className="shop-outfit__empty-body">{COPY.notFoundBody}</p>
        <Link className="shop-outfit__btn" to={`${SHOP_BASE}/items`}>
          {COPY.notFoundCta}
        </Link>
      </div>
    );
  }
  if (outfitQ.error) {
    return (
      <div className="shop-page shop-outfit shop-outfit__empty" data-testid="outfit-error">
        <p className="shop-outfit__notice" role="alert">
          <Icon name="warning-fill" size={16} />
          {COPY.generic}
        </p>
        <button type="button" className="shop-outfit__btn" onClick={outfitQ.refetch}>
          {COPY.retry}
        </button>
      </div>
    );
  }
  if (!outfit) {
    return (
      <div className="shop-page shop-outfit" aria-busy="true" data-testid="outfit-loading">
        <div className="shop-outfit__main">
          <div className="shop-outfit__preview">
            <span className="shop-outfit__skel" style={{ position: "absolute", inset: 0 }} aria-hidden />
          </div>
          <div className="shop-outfit__info">
            <span className="shop-outfit__skel" style={{ width: "60%", height: 32 }} aria-hidden />
            <span className="shop-outfit__skel" style={{ width: "40%", height: 16 }} aria-hidden />
            <span className="shop-outfit__skel" style={{ width: "100%", height: 48 }} aria-hidden />
          </div>
        </div>
      </div>
    );
  }
  return <OutfitContent outfit={outfit} />;
}

type ItemRow = { key: string; item: CatalogItem | null; state: OutfitItemState | "missing" };

function ItemChips({ item }: { item: CatalogItem }) {
  const catIco = categoryIcon(item);
  const genderIco = genderIcon(item.gender);
  return (
    <span className="shop-outfit__chips">
      {item.rarity ? (
        <span className="shop-outfit__chip" data-variant="rarity" style={{ background: rarityColor(item.rarity) }} title={rarityDescription(item.rarity)}>
          {rarityLabel(item.rarity)}
        </span>
      ) : null}
      {item.isSmart ? (
        <span className="shop-outfit__chip" data-variant="smart">
          <Icon name="smart" size={13} />
          {t("assetCard.smart")}
        </span>
      ) : null}
      {catIco ? (
        <span className="shop-outfit__chip" data-variant="icon">
          <Icon name={catIco} size={14} />
        </span>
      ) : null}
      {genderIco ? (
        <span className="shop-outfit__chip" data-variant="icon">
          <Icon name={genderIco} size={14} />
        </span>
      ) : null}
    </span>
  );
}

function ItemAuthor({ address }: { address: string }) {
  const name = useCreatorName(address);
  return (
    <Link className="shop-outfit__author" to={`${SHOP_BASE}/items/creator/${address}`}>
      {name ? `By ${name}` : " "}
    </Link>
  );
}

function OutfitRow({ row, loading }: { row: ItemRow; loading: boolean }) {
  const { session } = useShopWallet();
  const item = row.item;
  if (!item) {
    return (
      <li className="shop-outfit__item" data-testid="outfit-detail-item" data-state={row.state}>
        <span className={`shop-outfit__thumb${loading ? " shop-outfit__skel" : ""}`} aria-hidden />
        <span className="shop-outfit__body">
          {loading ? (
            <span className="shop-outfit__name">
              <span className="shop-outfit__skel" style={{ display: "block", width: "70%", height: 14 }} />
            </span>
          ) : (
            <>
              <span className="shop-outfit__name">{COPY.unavailable}</span>
              <span className="shop-outfit__pricerow">
                <span className="shop-outfit__badge">{COPY.unavailable}</span>
              </span>
            </>
          )}
        </span>
      </li>
    );
  }
  const href = detailRouteFor(item) ?? `${SHOP_BASE}/items/${item.contractAddress}/${item.itemId ?? ""}`;
  const network = item.chainId === 1 || item.chainId === 11155111 ? "ethereum" : "polygon";
  return (
    <li className="shop-outfit__item" data-testid="outfit-detail-item" data-state={row.state}>
      <Link className="shop-outfit__overlay" to={href} state={{ item }} aria-label={item.name} />
      {item.thumbnail ? <img className="shop-outfit__thumb" src={item.thumbnail} alt="" loading="lazy" /> : <span className="shop-outfit__thumb" aria-hidden />}
      <span className="shop-outfit__body">
        <span className="shop-outfit__name">{item.name}</span>
        {item.creator ? <ItemAuthor address={item.creator} /> : null}
        <span className="shop-outfit__pricerow">
          {row.state !== "unavailable" ? <PriceTag price={cardPrice(item, session)} network={network} compactView /> : null}
          {row.state === "unavailable" ? (
            <span className="shop-outfit__badge">{COPY.unavailable}</span>
          ) : row.state === "own_listing" ? (
            <span className="shop-outfit__badge">{COPY.yourListing}</span>
          ) : row.state === "in_cart" ? (
            <span className="shop-outfit__badge">{COPY.inCart}</span>
          ) : null}
          <ItemChips item={item} />
        </span>
      </span>
    </li>
  );
}

function OutfitContent({ outfit }: { outfit: Outfit }) {
  const { session, account } = useShopWallet();
  const lines = useCart((s) => s.lines);
  const keys = useMemo(() => [...new Set(outfit.items.map(outfitItemKey))].sort(), [outfit]);
  const itemsQ = useShopQuery(keys.length ? ["outfit-items", ...keys] : null, (signal) => fetchCatalogByIds(keys, { signal }), { staleMs: 60_000 });
  const byKey = useMemo(() => new Map((itemsQ.data ?? []).map((item) => [item.id.toLowerCase(), item])), [itemsQ.data]);
  const loading = keys.length > 0 && itemsQ.isLoading && !itemsQ.data;
  const cartAssets = useMemo(() => new Set(lines.map((l) => assetKey(l.asset))), [lines]);
  const [adding, setAdding] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  const rows: ItemRow[] = useMemo(
    () =>
      outfit.items.map((ref) => {
        const key = outfitItemKey(ref);
        const item = byKey.get(key) ?? null;
        const inCart = (i: CatalogItem) => {
          const asset = assetOf(i);
          return !!asset && cartAssets.has(assetKey(asset));
        };
        return { key, item, state: item ? classifyOutfitItem(item, { address: account, inCart }) : "missing" };
      }),
    [outfit, byKey, account, cartAssets],
  );

  const resolved = useMemo(() => rows.map((r) => r.item).filter((i): i is CatalogItem => !!i), [rows]);
  const purchasable = rows.filter((r) => r.state === "purchasable").map((r) => r.item!);
  const inCartCount = rows.filter((r) => r.state === "in_cart").length;
  const ownCount = rows.filter((r) => r.state === "own_listing").length;
  const allCredits = resolved.length > 0 && resolved.every((i) => cardEligibility(i, session)?.allowed.includes("credits") && i.priceCredits > 0);
  const outfitCredits = resolved.reduce((n, i) => n + i.priceCredits, 0);
  const ctaCredits = purchasable.reduce((n, i) => n + i.priceCredits, 0);

  const wearables = useMemo(() => resolved.filter(isWearable), [resolved]);
  const urns = useMemo(() => wearables.map((i) => i.urn ?? itemUrn(i)).filter((u): u is NonNullable<typeof u> => !!u), [wearables]);
  const forced = requiredShape(resolved);
  const body = forced ?? (outfit.bodyShape === "female" ? BASE_FEMALE : BASE_MALE);
  const emoteItem = useMemo(() => playingEmote(resolved), [resolved]);
  const emoteUrn = emoteItem ? (emoteItem.urn ?? itemUrn(emoteItem)) : null;
  const clipQ = useShopQuery(emoteUrn ? ["emote-clip", emoteUrn, body] : null, (signal) => fetchEmoteClipUrl(emoteUrn!, body, signal), { staleMs: 30 * 60_000 });
  const thumb = thumbnailUrl(outfit.thumbnailHash);
  const [webgl, setWebgl] = useState(false);
  useEffect(() => setWebgl(webglAvailable()), []);
  const show3d = webgl && urns.length > 0;

  const addOutfit = async () => {
    if (adding || purchasable.length === 0) return;
    setAdding(true);
    setStatus(null);
    try {
      const live = await Promise.all(
        purchasable.map(async (item) => {
          const page = await fetchShopItems({ contractAddress: item.contractAddress, itemId: item.itemId ?? "", first: 1 });
          const fresh = page.items[0];
          return fresh && !isListingUnavailable(fresh) ? fresh : null;
        }),
      );
      let added = 0;
      for (const item of live) {
        const input = item ? cartInputFor(item, "outfit") : null;
        if (!input) continue;
        shopCart.add({ ...input, provenance: { source: "outfit", outfitId: outfit.id } });
        added++;
      }
      if (added === 0) setStatus(COPY.noneLeft);
      else if (added < outfit.items.length) setStatus(`Added ${added} of ${outfit.items.length} items`);
    } catch {
      setStatus(COPY.addFailed);
    } finally {
      setAdding(false);
    }
  };

  return (
    <div className="shop-page shop-outfit" data-testid="outfit-detail">
      <nav className="shop-outfit__crumbs" aria-label="Breadcrumb">
        <Link to={`${SHOP_BASE}/overview`}>Overview</Link>
        <span aria-hidden>{">"}</span>
        <Link to={`${SHOP_BASE}/overview`}>{COPY.looks}</Link>
        <span aria-hidden>{">"}</span>
        <span className="shop-outfit__crumb-current" aria-current="page">
          {outfit.name}
        </span>
      </nav>

      <div className="shop-outfit__main">
        <div className="shop-outfit__preview" data-testid="outfit-preview" style={{ background: outfitRadialGradient(outfit) }}>
          {!show3d ? (
            thumb && !loading ? <img className="shop-outfit__fallback" src={thumb} alt={outfit.name} /> : null
          ) : (
            <Suspense fallback={thumb ? <img className="shop-outfit__fallback" src={thumb} alt={outfit.name} /> : null}>
              <WearablePreview urns={urns} body={body} emote={clipQ.data ?? undefined} spin={false} zoom={1} />
            </Suspense>
          )}
        </div>

        <div className="shop-outfit__info">
          <h1 className="shop-outfit__title">{outfit.name}</h1>
          <p className="shop-outfit__meta" data-testid="outfit-meta">
            {COPY.itemsCount(outfit.items.length)}
          </p>

          {itemsQ.error ? (
            <div className="shop-outfit__resolve-error">
              <p className="shop-outfit__notice" role="alert" data-testid="outfit-detail-resolve-error">
                <Icon name="warning-fill" size={16} />
                {COPY.resolveError}
              </p>
              <button type="button" className="shop-outfit__btn shop-outfit__btn--outline" data-testid="outfit-detail-retry" onClick={itemsQ.refetch}>
                {COPY.retry}
              </button>
            </div>
          ) : (
            <>
              <div className="shop-outfit__scroll">
                <ul className="shop-outfit__items" data-testid="outfit-items">
                  {rows.map((row) => (
                    <OutfitRow key={row.key} row={row} loading={loading} />
                  ))}
                </ul>
              </div>

              <div className="shop-outfit__ctabar" data-testid="outfit-detail-ctabar">
                {!loading && ownCount > 0 ? <p className="shop-outfit__hint">{COPY.yourListingHint}</p> : null}
                {resolved.length > 0 || loading ? (
                  <div className="shop-outfit__total">
                    <span className="shop-outfit__total-label">{COPY.totalPrice}</span>
                    {loading ? (
                      <span className="shop-outfit__skel" style={{ width: 64, height: 26 }} aria-hidden />
                    ) : allCredits ? (
                      <span className="shop-outfit__total-value" data-testid="outfit-total">
                        <Icon name="credits" size={22} />
                        {formatCreditsFull(outfitCredits)}
                      </span>
                    ) : (
                      <span className="shop-outfit__total-note">Priced per item at checkout</span>
                    )}
                  </div>
                ) : null}
                {loading ? (
                  <span className="shop-outfit__skel" style={{ width: "100%", height: 52, borderRadius: 26 }} aria-hidden />
                ) : (
                  <button type="button" className="shop-outfit__cta" data-testid="add-look" onClick={() => void addOutfit()} disabled={adding || purchasable.length === 0} aria-busy={adding || undefined}>
                    {adding ? COPY.adding : purchasable.length > 0 ? COPY.addCount(purchasable.length) : inCartCount > 0 ? COPY.nothingToAdd : COPY.unavailable}
                    {!adding && purchasable.length > 0 && allCredits ? (
                      <span className="shop-outfit__cta-price">
                        <Icon name="credits" size={16} />
                        {formatCreditsFull(ctaCredits)}
                      </span>
                    ) : null}
                  </button>
                )}
                {status ? (
                  <p className="shop-outfit__hint" role="status" data-testid="outfit-add-status">
                    {status}
                  </p>
                ) : null}
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
