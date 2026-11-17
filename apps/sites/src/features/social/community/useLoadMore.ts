import { useEffect, useRef } from "react";

/**
 * Loads the next page when the element at the end of a list scrolls into view, as the source's
 * infinite lists do. Without IntersectionObserver nothing loads on its own. `hasMore` is false after
 * a failed page, so a failure is not retried on its own: the list offers Try again instead.
 */
export function useLoadMore(hasMore: boolean, busy: boolean, loadMore: () => void) {
  const ref = useRef<HTMLDivElement>(null);
  const latest = useRef(loadMore);
  latest.current = loadMore;
  useEffect(() => {
    const node = ref.current;
    if (!node || !hasMore || busy || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) latest.current();
      },
      { rootMargin: "200px" },
    );
    observer.observe(node);
    return () => observer.disconnect();
  }, [hasMore, busy]);
  return ref;
}
