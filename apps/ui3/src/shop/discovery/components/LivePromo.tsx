// from: shop@2d3bf62:app/src/components/LivePromo/LivePromo.tsx, app/src/components/LivePromo/LivePromo.styles.ts;
//       shop@32377959:app/src/components/LivePromo/LivePromo.tsx (the wearable-preview iframe, Babylon, muted and
//       looping; mounted only while on screen) and app/src/hooks/usePreviewActive.ts

import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { Link } from "react-router";
import { LivePromo as Promo, type LivePromoProps } from "@dcl/one-ui/apps/shop/preview";
import { decidePreviewDevice } from "../../detail/lib/previewDevice";
import { REMOTE_FLAG, useRemoteFlagState } from "../lib/remoteFlags";

// The module path, not the "@dcl/one-ui" barrel: import() of the barrel keeps its whole namespace, which loses this
// lazy chunk and pulls every icon into the chunk the root route loads.
const AnimatedBackground = lazy(() =>
  import("@dcl/one-ui/components/AnimatedBackground/AnimatedBackground").then((m) => ({ default: m.AnimatedBackground })),
);

export default function LivePromo(props: Omit<LivePromoProps, "Link" | "backdrop" | "allowed" | "bevyFlag" | "linkRef" | "onScreen">) {
  const ref = useRef<HTMLAnchorElement>(null);
  const [onScreen, setOnScreen] = useState(false);
  const [allowed, setAllowed] = useState(false);
  const bevyFlag = useRemoteFlagState(REMOTE_FLAG.bevyAvatarPreview, allowed);
  // Decided the first time the tile nears the screen: a visitor who never scrolls to it does not pay for the link probe.
  const [near, setNear] = useState(false);
  if (onScreen && !near) setNear(true);
  useEffect(() => {
    if (!near) return;
    if (typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    return decidePreviewDevice((decision) => {
      if (decision.renderer === "full") setAllowed(true);
    });
  }, [near]);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    // Nothing to tell whether the tile is on screen with: it mounts, rather than keep the picture for good.
    if (typeof IntersectionObserver === "undefined") {
      setOnScreen(true);
      return;
    }
    // A little before it scrolls into view, so the scene is ready when it lands. The tile is the one element observed, so
    // the last entry of a batch is where it is now.
    const io = new IntersectionObserver(
      (entries) => {
        const last = entries[entries.length - 1];
        if (last) setOnScreen(last.isIntersecting);
      },
      { rootMargin: "200px" },
    );
    io.observe(el);
    return () => io.disconnect();
  }, []);

  return (
    <Promo
      {...props}
      Link={Link}
      allowed={allowed}
      linkRef={ref}
      onScreen={onScreen}
      bevyFlag={bevyFlag}
      backdrop={<Suspense fallback={null}><AnimatedBackground patternTiling={1} /></Suspense>}
    />
  );
}
