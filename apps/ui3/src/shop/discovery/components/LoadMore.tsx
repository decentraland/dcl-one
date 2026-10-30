// from: shop@94584a5c:app/src/components/LoadMore/LoadMore.tsx; shop@2d3bf62:app/src/components/LoadMore/LoadMore.styles.ts,
//       app/src/components/Button/Button.styles.ts (the ghost variant)

import { useEffect, useRef } from "react";

import { t } from "../intl";
import "./loadmore.css";

export default function LoadMore({
  hasNextPage,
  isFetching,
  isError = false,
  onLoadMore,
  auto = true,
}: {
  hasNextPage: boolean;
  isFetching: boolean;
  isError?: boolean;
  onLoadMore: () => void;
  auto?: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const loadMore = useRef(onLoadMore);
  loadMore.current = onLoadMore;

  useEffect(() => {
    const el = ref.current;
    if (!el || !hasNextPage || isFetching || isError || !auto || typeof IntersectionObserver === "undefined") return;
    const io = new IntersectionObserver((entries) => {
      if (entries[0]?.isIntersecting) loadMore.current();
    }, { rootMargin: "600px 0px" });
    io.observe(el);
    return () => io.disconnect();
  }, [hasNextPage, isFetching, isError, auto]);

  if (!hasNextPage) return null;
  return (
    <div ref={ref} className="shop-loadmore">
      <button type="button" className="shop-loadmore__btn" onClick={onLoadMore} disabled={isFetching} data-testid="load-more">
        {isFetching ? t("loadMore.loading") : isError ? t("loadMore.retry") : t("loadMore.loadMore")}
      </button>
    </div>
  );
}
