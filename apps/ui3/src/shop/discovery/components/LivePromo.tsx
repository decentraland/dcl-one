// from: shop@2d3bf62:app/src/components/LivePromo/LivePromo.tsx, app/src/components/LivePromo/LivePromo.styles.ts;
//       shop@32377959:app/src/components/LivePromo/LivePromo.tsx (the wearable-preview iframe, Babylon, muted and
//       looping; mounted only while on screen) and app/src/hooks/usePreviewActive.ts

import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { Link } from "react-router";

import PreviewFrame, { UNITY_LOAD_TIMEOUT_MS, type PreviewFrameOptions } from "../../detail/components/PreviewFrame";
import { pickRenderer } from "../../detail/lib/pickRenderer";
import { REMOTE_FLAG, useRemoteFlagState } from "../lib/remoteFlags";
import "./livepromo.css";

// The module path, not the "@dcl/one-ui" barrel: import() of the barrel keeps its whole namespace, which loses this
// lazy chunk and pulls every icon into the chunk the root route loads.
const AnimatedBackground = lazy(() =>
  import("@dcl/one-ui/components/AnimatedBackground/AnimatedBackground").then((m) => ({ default: m.AnimatedBackground })),
);

// Upstream's close-up framing, passed as the preview's own zoom so it survives the camera framing the avatar again as
// its items stream in. Its wheel range is widened to match; the tile takes no pointer input, so it only sets the range.
const ZOOM = 100;
const WHEEL_ZOOM = 2.5;

/**
 * The tile's avatar, as upstream draws it: Babylon whatever the Unity flag says (the Bevy avatar preview where its own
 * flag is on, in upstream's marketplace mode), a default body in the promo's items doing
 * PreviewEmote.FASHION (an emote item among the urns, listed first, plays instead). The urns are mainnet's, so the iframe
 * reads mainnet's peer and marketplace whatever the Shop's environment (previewBase).
 */
function previewOptions(urns: string[], bevy: boolean): PreviewFrameOptions {
  const options: PreviewFrameOptions = { urns, profile: "default", type: "avatar", emote: "fashion", zoom: ZOOM, wheelZoom: WHEEL_ZOOM, disableBackground: true };
  return bevy ? { ...options, bevy: true, unityMode: "marketplace" } : options;
}

type Props = {
  to: string;
  urns: string[];
  title: string;
  cta: string;
  ariaLabel: string;
  fallback: string;
  fallbackAlt: string;
  testId?: string;
};

export default function LivePromo({ to, urns, title, cta, ariaLabel, fallback, fallbackAlt, testId }: Props) {
  const ref = useRef<HTMLAnchorElement>(null);
  // Read on the client only: the still picture stays for reduced motion and for a device that should not draw two 3D
  // scenes on the home page.
  const [allowed, setAllowed] = useState(false);
  const [onScreen, setOnScreen] = useState(false);
  const [ready, setReady] = useState(false);
  const [failed, setFailed] = useState(false);
  // Bevy failing gives way to Babylon for the rest of the tile's life.
  const [bevyFailed, setBevyFailed] = useState(false);
  const bevyFlag = useRemoteFlagState(REMOTE_FLAG.bevyAvatarPreview, allowed);
  const bevy = bevyFlag === true && !bevyFailed;

  useEffect(() => {
    if (typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    if (pickRenderer().renderer !== "full") return;
    setAllowed(true);
  }, []);

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

  const live = allowed && !failed;
  // The iframe lives only while the tile is on screen: off screen its render loop runs at full speed for nobody. A hidden
  // tab keeps it, since the browser all but stops its frames there.
  // Mounted once the flag has said which renderer, so the tile never loads one and then the other.
  const mounted = live && onScreen && bevyFlag !== null;
  // A new iframe has drawn nothing: back on screen, the picture stays until it has.
  if (!mounted && ready) setReady(false);
  const show = mounted && ready;

  return (
    <Link ref={ref} className="shop-promo" to={to} aria-label={ariaLabel} data-testid={testId} data-crest={show || undefined}>
      <span className="shop-promo__card">
        {/* The fitting room's animated backdrop under the avatar, as upstream's tile draws it: only while the tile is
            on screen, with larger drifting tiles (tiling 1, against the fitting room's 1.66) for the smaller card. */}
        {mounted ? (
          <Suspense fallback={null}>
            <AnimatedBackground patternTiling={1} />
          </Suspense>
        ) : null}
        <span className="shop-promo__inner" data-ready={show || undefined}>
          <span className="shop-promo__title">{title}</span>
          <span className="shop-promo__cta">{cta}</span>
        </span>
        <img className="shop-promo__fallback" src={fallback} alt={fallbackAlt} data-hidden={show || undefined} decoding="async" />
      </span>
      {mounted ? (
        <span className="shop-promo__avatar" data-ready={show || undefined} aria-hidden>
          <PreviewFrame
            key={bevy ? "bevy" : "babylon"}
            options={previewOptions(urns, bevy)}
            ambient
            loadTimeoutMs={bevy ? UNITY_LOAD_TIMEOUT_MS : undefined}
            onLoad={() => setReady(true)}
            // A fresh iframe has drawn nothing yet (Bevy booting again): the picture covers it until it has.
            onReplaced={() => setReady(false)}
            onError={() => {
              setReady(false);
              if (bevy) setBevyFailed(true);
              else setFailed(true);
            }}
          />
        </span>
      ) : null}
    </Link>
  );
}
