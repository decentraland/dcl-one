// from: shop@2d3bf62:app/src/components/TopCreators/TopCreators.tsx, app/src/components/TopCreators/TopCreators.styles.ts,
//       app/src/styles/row.styles.ts (Root, Head, Title, Viewport, Arrow, Dots, Dot)

import { useCallback, useEffect, useRef, useState } from "react";
import { Link } from "react-router";

import { fetchCommunityProfiles } from "../../../data/hooks/useCommunityProfiles";
import { SHOP_BASE } from "../../routes";
import { fetchShopTopCreators } from "../api";
import { t } from "../intl";
import { railGeometry, railPageFromGeometry, scrollRailToPage } from "../lib/pagedRail";
import { avatarBackgroundColor, selectTopCreators, type CreatorProfile, type TopCreator } from "../lib/topCreators";
import { useShopQuery } from "../query";
import carouselArrow from "../icons/carousel-arrow.svg?url";
import "./rail.css";
import "./topcreators.css";

const CARDS = 8;
const DAYS = 30;
const CANDIDATES = 30;

const count = new Intl.NumberFormat("en-US");

function tr(key: string, fallback: string): string {
  const s = t(key);
  return s === key ? fallback : s;
}

export async function loadTopCreators(signal?: AbortSignal): Promise<TopCreator[]> {
  const ranked = await fetchShopTopCreators(CANDIDATES, DAYS, { signal });
  const raw = ranked.length ? await fetchCommunityProfiles(ranked.map((c) => c.id), { signal }) : {};
  const profiles = new Map<string, CreatorProfile>();
  for (const [address, p] of Object.entries(raw)) profiles.set(address.toLowerCase(), { name: p.name, hasClaimedName: p.claimed === true, face: p.picture ?? undefined });
  return selectTopCreators(ranked, profiles, CARDS);
}

function CreatorCard({ creator }: { creator: TopCreator }) {
  const catalogue =
    creator.collections != null && creator.items != null
      ? t("topCreators.catalogue", { collections: count.format(creator.collections), items: count.format(creator.items) })
      : null;
  const lifetime = creator.totalSales != null ? t("topCreators.totalSales", { sales: count.format(creator.totalSales) }) : null;
  return (
    <Link className="shop-creator-card" to={`${SHOP_BASE}/items/creator/${creator.address}`} aria-label={t("topCreators.viewCreationsBy", { name: creator.name })} data-testid="top-creator-card">
      <span className="shop-creator-card__avatar" style={{ backgroundColor: avatarBackgroundColor(creator.name) }}>
        {creator.face ? <img src={creator.face} alt="" loading="lazy" /> : null}
      </span>
      <span className="shop-creator-card__panel">
        <span className="shop-creator-card__text">
          <span className="shop-creator-card__name" title={creator.name}>
            {creator.name}
          </span>
          <span className="shop-creator-card__desc">
            {catalogue ? (
              <>
                {catalogue}
                <br />
              </>
            ) : null}
            {lifetime}
          </span>
        </span>
        <span className="shop-creator-card__cta" aria-hidden>
          {tr("topCreators.viewCreations", "View creations")}
        </span>
      </span>
    </Link>
  );
}

function SkeletonCard() {
  return (
    <span className="shop-creator-card shop-creator-card--skeleton" aria-hidden data-testid="top-creator-skeleton">
      <span className="shop-creator-card__avatar skeleton" />
      <span className="shop-creator-card__panel" data-skeleton>
        <span className="shop-creator-card__sk-name skeleton" />
        <span className="shop-creator-card__sk-desc">
          <span className="skeleton" />
          <span className="skeleton" data-short />
        </span>
        <span className="shop-creator-card__sk-cta skeleton" />
      </span>
    </span>
  );
}

export default function TopCreators() {
  const { data, isLoading, error } = useShopQuery(["shop-top-creators", DAYS, CANDIDATES, CARDS], (signal) => loadTopCreators(signal), { staleMs: 5 * 60_000 });
  const creators = data ?? [];

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
  }, [measure, creators.length]);

  const scrollToPage = useCallback((target: number) => {
    const el = trackRef.current;
    if (!el) return;
    const g = railGeometry(el);
    if (g) scrollRailToPage(el, g, target);
  }, []);

  if (error || (!isLoading && creators.length === 0)) return null;

  const title = t("topCreators.title");
  const showControls = !isLoading && pageCount > 1;

  return (
    <section className="shop-rail shop-creators" data-testid="top-creators" aria-label={title}>
      <header className="shop-rail__head">
        <h2 className="shop-rail__title">{title}</h2>
      </header>
      <div className="shop-rail__viewport shop-creators__viewport">
        {showControls ? (
          <button type="button" className="shop-rail__arrow" data-side="left" onClick={() => scrollToPage(page - 1)} disabled={page <= 0} aria-label={t("overview.previous")}>
            <img src={carouselArrow} alt="" />
          </button>
        ) : null}
        <div className="shop-creators__track" ref={trackRef} aria-busy={isLoading || undefined}>
          {isLoading ? Array.from({ length: CARDS }, (_, i) => <SkeletonCard key={i} />) : creators.map((c) => <CreatorCard key={c.address} creator={c} />)}
        </div>
        {showControls ? (
          <button type="button" className="shop-rail__arrow" data-side="right" onClick={() => scrollToPage(page + 1)} disabled={page >= pageCount - 1} aria-label={t("overview.next")}>
            <img src={carouselArrow} alt="" />
          </button>
        ) : null}
      </div>
      {showControls ? (
        <div className="shop-rail__dots" role="group" data-testid="top-creators-dots" aria-label={t("overview.carouselPages", { title })}>
          {Array.from({ length: pageCount }, (_, i) => (
            <button
              key={i}
              type="button"
              className="shop-rail__dot"
              data-active={i === page || undefined}
              onClick={() => scrollToPage(i)}
              aria-label={t("overview.goToPage", { page: i + 1 })}
              aria-current={i === page ? "true" : undefined}
            />
          ))}
        </div>
      ) : (
        <div className="shop-rail__dots" aria-hidden data-testid="rail-dots-reserved" />
      )}
    </section>
  );
}
