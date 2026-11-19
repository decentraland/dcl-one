import { useEffect, useRef, useState, type RefObject } from "react";

/**
 * Loads the next page when the element at the end of a list scrolls into view, as the source's
 * infinite lists do. `root` is the list's own scroller when it has one (the members list), so the
 * next page loads as that list nears its end; the page scrolls otherwise. Without
 * IntersectionObserver nothing loads on its own and `manual` is true: the list shows a Load more
 * button instead, as the places grid does. `hasMore` is false after a failed page, so a failure is
 * not retried on its own: the list offers Try again instead.
 */
export function useLoadMore(
  hasMore: boolean,
  busy: boolean,
  loadMore: () => void,
  root?: RefObject<HTMLElement | null>,
) {
  const ref = useRef<HTMLDivElement>(null);
  const latest = useRef(loadMore);
  // Decided after hydration, so the server and the first browser render agree.
  const [manual, setManual] = useState(false);
  useEffect(() => {
    latest.current = loadMore;
  });
  useEffect(() => setManual(typeof IntersectionObserver === "undefined"), []);
  useEffect(() => {
    const node = ref.current;
    if (!node || !hasMore || busy || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) latest.current();
      },
      { root: root?.current ?? null, rootMargin: "200px" },
    );
    observer.observe(node);
    return () => observer.disconnect();
  }, [hasMore, busy, root]);
  return { ref, manual };
}
