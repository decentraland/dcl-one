// from: shop@2d3bf62:app/src/pages/Overview.tsx (Carousel), app/src/components/CardCarousel/CardCarousel.tsx,
//       app/src/components/CollectionCarousel/CollectionCarousel.tsx, app/src/styles/row.styles.ts

import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { Link } from "react-router";

import { t } from "../intl";
import type { CatalogItem } from "../lib/catalogItem";
import { listingKey } from "../lib/listingKey";
import { railPageCount, railPageFromScroll } from "../lib/pagedRail";
import AssetCard, { type CardSource } from "./AssetCard";
import Icon from "./Icon";
import SkeletonCards from "./SkeletonCards";
import carouselArrow from "../icons/carousel-arrow.svg?url";
import "./rail.css";

const SKELETON_COUNT = 6;

export type RailProps = {
  title: ReactNode;
  items: CatalogItem[];
  loading?: boolean;
  source?: CardSource;
  viewAllTo?: string;
  testId?: string;
  renderItem?: (item: CatalogItem, index: number) => ReactNode;
  headerExtra?: ReactNode;
};

export default function Rail({ title, items, loading = false, source = "grid", viewAllTo, testId, renderItem, headerExtra }: RailProps) {
  const trackRef = useRef<HTMLDivElement>(null);
  const [pageCount, setPageCount] = useState(1);
  const [page, setPage] = useState(0);
  const count = loading ? SKELETON_COUNT : items.length;

  const measure = useCallback(() => {
    const el = trackRef.current;
    if (!el || el.clientWidth <= 0) return;
    setPageCount(railPageCount(el.scrollWidth, el.clientWidth));
    setPage(railPageFromScroll(el.scrollLeft, el.scrollWidth, el.clientWidth));
  }, []);

  useEffect(() => {
    measure();
    const el = trackRef.current;
    if (!el) return;
    const onScroll = () => setPage(railPageFromScroll(el.scrollLeft, el.scrollWidth, el.clientWidth));
    el.addEventListener("scroll", onScroll, { passive: true });
    window.addEventListener("resize", measure);
    return () => {
      el.removeEventListener("scroll", onScroll);
      window.removeEventListener("resize", measure);
    };
  }, [measure, count]);

  const scrollToPage = useCallback(
    (p: number) => {
      const el = trackRef.current;
      if (!el) return;
      const target = Math.max(0, Math.min(pageCount - 1, p));
      el.scrollTo?.({ left: target * el.clientWidth, behavior: "smooth" });
    },
    [pageCount],
  );

  const showControls = !loading && pageCount > 1;
  const label = typeof title === "string" ? title : undefined;

  return (
    <section className="shop-rail" data-testid={testId} aria-label={label}>
      <header className="shop-rail__head">
        <h2 className="shop-rail__title">{title}</h2>
        {headerExtra}
        {viewAllTo ? (
          <Link className="shop-rail__all" to={viewAllTo}>
            {t("overview.viewAll")} <Icon name="view-all-arrow" size={18} />
          </Link>
        ) : null}
      </header>
      <div className="shop-rail__viewport">
        {showControls ? (
          <button type="button" className="shop-rail__arrow" data-side="left" onClick={() => scrollToPage(page - 1)} disabled={page <= 0} aria-label={t("overview.previous")}>
            <img src={carouselArrow} alt="" />
          </button>
        ) : null}
        <div className="shop-rail__track" ref={trackRef} aria-busy={loading || undefined}>
          {loading ? (
            <SkeletonCards count={SKELETON_COUNT} />
          ) : (
            items.map((item, i) => (
              <div className="shop-rail__cell" key={listingKey(item)}>
                {renderItem ? renderItem(item, i) : <AssetCard item={item} source={source} position={i} />}
              </div>
            ))
          )}
        </div>
        {showControls ? (
          <button type="button" className="shop-rail__arrow" data-side="right" onClick={() => scrollToPage(page + 1)} disabled={page >= pageCount - 1} aria-label={t("overview.next")}>
            <img src={carouselArrow} alt="" />
          </button>
        ) : null}
      </div>
      {showControls ? (
        <div className="shop-rail__dots" role="group" aria-label={t("overview.carouselPages", { title: label ?? "" })}>
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
