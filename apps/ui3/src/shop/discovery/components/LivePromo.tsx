// from: shop@2d3bf62:app/src/components/LivePromo/LivePromo.tsx, app/src/components/LivePromo/LivePromo.styles.ts

import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { Link } from "react-router";

import { pickRenderer } from "../../detail/lib/pickRenderer";
import "./livepromo.css";

const SETTLE_MS = 700;

const WearablePreview = lazy(() => import("../../../wearable-preview/WearablePreview"));

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
  const [active, setActive] = useState(false);
  const [ready, setReady] = useState(false);
  const [failed, setFailed] = useState(false);
  const [onScreen, setOnScreen] = useState(false);
  const [settled, setSettled] = useState(false);

  useEffect(() => {
    const el = ref.current;
    if (!el || typeof IntersectionObserver === "undefined") return;
    const io = new IntersectionObserver((entries) => {
      const last = entries[entries.length - 1];
      if (last) setOnScreen(last.isIntersecting);
    });
    io.observe(el);
    return () => io.disconnect();
  }, []);

  useEffect(() => {
    if (!ready || !onScreen) {
      setSettled(false);
      return;
    }
    const timer = setTimeout(() => setSettled(true), SETTLE_MS);
    return () => clearTimeout(timer);
  }, [ready, onScreen]);

  useEffect(() => {
    const el = ref.current;
    if (!el || typeof IntersectionObserver === "undefined") return;
    if (typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    if (pickRenderer().renderer !== "full") return;
    const io = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) {
        setActive(true);
        io.disconnect();
      }
    });
    io.observe(el);
    return () => io.disconnect();
  }, []);

  const live = active && !failed;
  const show = live && ready && onScreen && settled;

  return (
    <Link ref={ref} className="shop-promo" to={to} aria-label={ariaLabel} data-testid={testId} data-crest={show || undefined}>
      <span className="shop-promo__card">
        <span className="shop-promo__inner" data-ready={show || undefined}>
          <span className="shop-promo__title">{title}</span>
          <span className="shop-promo__cta">{cta}</span>
        </span>
        <img className="shop-promo__fallback" src={fallback} alt={fallbackAlt} data-hidden={show || undefined} decoding="async" />
      </span>
      {live ? (
        <span className="shop-promo__avatar" data-ready={show || undefined} aria-hidden>
          <Suspense fallback={null}>
            <WearablePreview
              urns={urns}
              spin={false}
              controls={false}
              zoom={1.1}
              pauseOffscreen
              onStatus={(s) => {
                if (s === "ready") setReady(true);
                else if (s === "error" || s === "empty") setFailed(true);
              }}
            />
          </Suspense>
        </span>
      ) : null}
    </Link>
  );
}
