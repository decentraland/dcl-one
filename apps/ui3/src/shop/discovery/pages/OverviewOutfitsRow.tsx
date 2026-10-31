// from: shop@3a322cf9:app/src/components/OutfitsRow/OutfitsRow.tsx, app/src/components/OutfitsRow/OutfitsRow.styles.ts,
//       app/src/components/OutfitCard/OutfitCard.tsx, app/src/components/OutfitCard/OutfitCard.styles.ts,
//       app/src/lib/outfits.ts (outfitGradient, outfitFade, orderOutfitsByRecency, isBuyableFromCreator)

import { useCallback, useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import { Link } from "react-router";

import { isShopApiError } from "../../api/client";
import { shopClient } from "../../api/shop";
import { shopCart } from "../../cart/store";
import { SHOP_BASE } from "../../routes";
import { fetchCatalogByIds } from "../api";
import { cartInputFor } from "../components/AssetCard";
import { formatCredits, formatCreditsFull } from "../components/PriceTag";
import Icon from "../components/Icon";
import { t } from "../intl";
import type { CatalogItem } from "../lib/catalogItem";
import { railGeometry, railPageFromGeometry, scrollRailToPage } from "../lib/pagedRail";
import { useShopQuery } from "../query";
import carouselArrow from "../icons/carousel-arrow.svg?url";
import "../components/rail.css";
import "./overviewoutfits.css";

const MAX_DOTS = 6;
const DEFAULT_GRADIENT = { from: "#c640cd", to: "#691fa9" } as const;
const HEX = /^#[0-9a-f]{6}$/i;

type OutfitSummary = {
  id: string;
  name: string;
  thumbnailHash: string;
  items: { contractAddress: string; itemId: string }[];
  gradientFrom: string;
  gradientTo: string;
  createdAt: number;
};

function tr(key: string, fallback: string, vars?: Record<string, string | number>): string {
  const s = t(key, vars);
  return s === key ? fallback : s;
}

function parseOutfits(body: unknown): OutfitSummary[] {
  const list = body && typeof body === "object" && "outfits" in body ? (body as { outfits: unknown }).outfits : body;
  if (!Array.isArray(list)) return [];
  const out: OutfitSummary[] = [];
  for (const raw of list as Partial<OutfitSummary & { published: boolean }>[]) {
    if (!raw || typeof raw.id !== "string" || typeof raw.name !== "string" || !Array.isArray(raw.items) || raw.published === false) continue;
    out.push({
      id: raw.id,
      name: raw.name,
      thumbnailHash: raw.thumbnailHash ?? "",
      items: raw.items.filter((i) => i && typeof i.contractAddress === "string" && typeof i.itemId === "string"),
      gradientFrom: raw.gradientFrom ?? "",
      gradientTo: raw.gradientTo ?? "",
      createdAt: typeof raw.createdAt === "number" ? raw.createdAt : 0,
    });
  }
  return out;
}

async function fetchOutfits(signal: AbortSignal): Promise<OutfitSummary[]> {
  try {
    return parseOutfits(await shopClient().outfits.list(undefined, { signal }));
  } catch (e) {
    if (isShopApiError(e) && (e.status === 404 || e.code === "noLegacyAlias")) return [];
    throw e;
  }
}

const itemKey = (ref: { contractAddress: string; itemId: string }) => `${ref.contractAddress.toLowerCase()}-${ref.itemId}`;

function stops(o: OutfitSummary) {
  return { from: HEX.test(o.gradientFrom) ? o.gradientFrom : DEFAULT_GRADIENT.from, to: HEX.test(o.gradientTo) ? o.gradientTo : DEFAULT_GRADIENT.to };
}

function rgba(hex: string, a: number) {
  const v = Number.parseInt(hex.slice(1), 16);
  return `rgba(${(v >> 16) & 0xff}, ${(v >> 8) & 0xff}, ${v & 0xff}, ${a})`;
}

// A live mint with supply left, sold by its creator for credits.
function isBuyableFromCreator(item: CatalogItem): boolean {
  if (item.priceCredits <= 0) return false;
  if (!item.tokenId && typeof item.available === "number" && item.available <= 0) return false;
  return item.hasPrimaryListing === true && (item.available ?? 0) > 0;
}

const DAY_MS = 24 * 60 * 60 * 1000;
const RECENCY_BUCKETS_MS = [7, 21, 60, 365].map((days) => days * DAY_MS);

function shuffle<T>(items: readonly T[]): T[] {
  const out = [...items];
  for (let i = out.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [out[i], out[j]] = [out[j]!, out[i]!];
  }
  return out;
}

// Shuffled, newer age buckets first: a fresh drop leads while every visit still varies.
export function orderOutfitsByRecency<T extends { createdAt: number }>(outfits: readonly T[], now = Date.now()): T[] {
  const bucket = (o: T) => {
    const i = RECENCY_BUCKETS_MS.findIndex((limit) => now - o.createdAt < limit);
    return i === -1 ? RECENCY_BUCKETS_MS.length : i;
  };
  return shuffle(outfits).sort((a, b) => bucket(a) - bucket(b));
}

function OutfitCard({ outfit, byKey, resolving }: { outfit: OutfitSummary; byKey: Map<string, CatalogItem>; resolving: boolean }) {
  const [added, setAdded] = useState(false);
  const { from, to } = stops(outfit);
  const thumb = outfit.thumbnailHash ? shopClient().outfits.thumbnailUrl(outfit.thumbnailHash) : null;
  const items = outfit.items.map((ref) => byKey.get(itemKey(ref))).filter((i): i is CatalogItem => !!i);
  const credits = items.reduce((sum, i) => sum + (i.priceCredits > 0 ? i.priceCredits : 0), 0);
  const priced = items.length === outfit.items.length && items.every((i) => i.priceCredits > 0);

  const onAdd = (event: MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
    let n = 0;
    for (const item of items) {
      const input = cartInputFor(item, "outfit");
      if (!input) continue;
      shopCart.add({ ...input, provenance: { source: "outfit", outfitId: outfit.id } });
      n++;
    }
    if (n > 0) setAdded(true);
  };

  return (
    <Link className="shop-outfit-card" to={`${SHOP_BASE}/items/outfits/${outfit.id}`} data-testid="outfit-card">
      <span className="shop-outfit-card__frame" data-testid="outfit-card-thumb" style={{ background: `radial-gradient(65.83% 65.83% at 50% 50%, ${to} 0%, ${from} 100%)` }} />
      <span className="shop-outfit-card__mask">{thumb ? <img className="shop-outfit-card__thumb" src={thumb} alt={outfit.name} loading="lazy" /> : null}</span>
      <span className="shop-outfit-card__fade" style={{ background: `linear-gradient(180deg, ${rgba(to, 0)} 0%, ${rgba(to, 0.8)} 100%)` }} />
      <span className="shop-outfit-card__scrim" />
      <span className="shop-outfit-card__body" data-testid="outfit-card-info">
        <span className="shop-outfit-card__top">
          <span className="shop-outfit-card__name">{outfit.name}</span>
          <span className="shop-outfit-card__meta">{tr("outfits.card.items", t("outfit.items", { count: outfit.items.length }), { count: outfit.items.length })}</span>
          {resolving ? (
            <span className="shop-outfit-card__price">
              <span className="shop-outfit-card__price-sk" />
            </span>
          ) : priced ? (
            <span className="shop-outfit-card__price">
              <Icon name="credits" size={16} /> <span title={formatCreditsFull(credits)} aria-hidden>{formatCredits(credits)}</span>
              <span className="shop-sr">{t("assetCard.credits", { count: credits })}</span>
            </span>
          ) : null}
        </span>
        <button type="button" className="shop-outfit-card__cta" onClick={onAdd} disabled={resolving || items.length === 0} data-testid="outfit-card-cta">
          <Icon name="cart-solid" size={16} />
          {added ? t("outfit.added") : tr("outfits.card.add", t("itemDetail.addToCart"))}
        </button>
      </span>
    </Link>
  );
}

export default function OverviewOutfitsRow() {
  const outfitsQ = useShopQuery(["overview-outfits"], (signal) => fetchOutfits(signal), { staleMs: 60_000 });
  const outfits = useMemo(() => orderOutfitsByRecency(outfitsQ.data ?? []), [outfitsQ.data]);
  const ids = useMemo(() => Array.from(new Set(outfits.flatMap((o) => o.items.map(itemKey)))), [outfits]);
  const itemsQ = useShopQuery(ids.length ? ["overview-outfit-items", ...ids] : null, (signal) => fetchCatalogByIds(ids, { signal }), { staleMs: 60_000 });
  const byKey = useMemo(() => {
    const m = new Map<string, CatalogItem>();
    for (const item of itemsQ.data ?? []) if (item.itemId != null) m.set(itemKey({ contractAddress: item.contractAddress, itemId: item.itemId }), item);
    return m;
  }, [itemsQ.data]);
  const resolving = ids.length > 0 && itemsQ.isLoading;

  const visible = useMemo(() => {
    if (resolving || itemsQ.error) return outfits;
    return outfits.filter((o) => o.items.length > 0 && o.items.every((ref) => {
      const item = byKey.get(itemKey(ref));
      return !!item && isBuyableFromCreator(item);
    }));
  }, [outfits, byKey, resolving, itemsQ.error]);

  const trackRef = useRef<HTMLDivElement>(null);
  const [pageCount, setPageCount] = useState(1);
  const [page, setPage] = useState(0);

  const measure = useCallback(() => {
    const el = trackRef.current;
    if (!el) return;
    const g = railGeometry(el);
    if (!g) return;
    setPageCount(g.pageCount);
    setPage(railPageFromGeometry(el, g));
  }, []);

  useEffect(() => {
    measure();
    const el = trackRef.current;
    if (!el) return;
    let frame = 0;
    const onScroll = () => {
      if (frame) return;
      frame = requestAnimationFrame(() => {
        frame = 0;
        const g = railGeometry(el);
        if (g) setPage(railPageFromGeometry(el, g));
      });
    };
    el.addEventListener("scroll", onScroll, { passive: true });
    window.addEventListener("resize", measure);
    return () => {
      if (frame) cancelAnimationFrame(frame);
      el.removeEventListener("scroll", onScroll);
      window.removeEventListener("resize", measure);
    };
  }, [measure, visible.length]);

  const scrollToPage = useCallback((target: number) => {
    const el = trackRef.current;
    if (!el) return;
    const g = railGeometry(el);
    if (g) scrollRailToPage(el, g, target);
  }, []);

  if (outfitsQ.isLoading || outfitsQ.error || visible.length === 0) return null;

  const title = tr("outfits.rowTitle", "Buy the Look");
  const dotCount = Math.min(pageCount, MAX_DOTS);
  const activeDot = pageCount <= 1 ? 0 : Math.round((page / (pageCount - 1)) * (dotCount - 1));
  const dotPage = (dot: number) => (dotCount <= 1 ? 0 : Math.round((dot / (dotCount - 1)) * (pageCount - 1)));
  const showControls = pageCount > 1;

  return (
    <section className="shop-rail shop-outfits-row" data-testid="outfits-row" aria-label={title}>
      <header className="shop-rail__head">
        <h2 className="shop-rail__title">{title}</h2>
      </header>
      <div className="shop-rail__viewport shop-outfits-row__viewport">
        {showControls ? (
          <button type="button" className="shop-rail__arrow" data-side="left" data-testid="outfits-row-prev" onClick={() => scrollToPage(page - 1)} disabled={page <= 0} aria-label={t("overview.previous")}>
            <img src={carouselArrow} alt="" />
          </button>
        ) : null}
        <div className="shop-outfits-row__track" ref={trackRef} data-testid="outfits-row-track">
          {visible.map((o) => (
            <OutfitCard key={o.id} outfit={o} byKey={byKey} resolving={resolving} />
          ))}
        </div>
        {showControls ? (
          <button type="button" className="shop-rail__arrow" data-side="right" data-testid="outfits-row-next" onClick={() => scrollToPage(page + 1)} disabled={page >= pageCount - 1} aria-label={t("overview.next")}>
            <img src={carouselArrow} alt="" />
          </button>
        ) : null}
      </div>
      {showControls ? (
        <div className="shop-rail__dots" role="group" data-testid="outfits-row-dots" aria-label={t("overview.carouselPages", { title })}>
          {Array.from({ length: dotCount }, (_, i) => (
            <button
              key={i}
              type="button"
              className="shop-rail__dot"
              data-active={i === activeDot || undefined}
              onClick={() => scrollToPage(dotPage(i))}
              aria-label={t("overview.goToPage", { page: dotPage(i) + 1 })}
              aria-current={i === activeDot ? "true" : undefined}
            />
          ))}
        </div>
      ) : (
        <div className="shop-rail__dots" aria-hidden data-testid="rail-dots-reserved" />
      )}
    </section>
  );
}
