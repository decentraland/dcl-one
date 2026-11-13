import { useCallback, useEffect, useRef, useState, type MouseEvent, type ReactNode, type TouchEvent, type TransitionEvent } from "react";

import styles from "./Carousel.module.css";
import { useMediaQuery } from "@ui/data/hooks/useMediaQuery";

const SWIPE_THRESHOLD = 50;

type CarouselProps<T> = {
  items: readonly T[];
  renderItem: (item: T) => ReactNode;
  keyOf: (item: T) => string;
  /** Autoplay interval in ms; 0 turns it off. */
  autoplayDelay?: number;
  /** Desktop slide width in px. */
  slideWidth?: number;
  alignItems?: "stretch" | "center";
};

/**
 * An endless slider: the items three times over, the middle copy real, snapping back into it after
 * a step lands on a copy (decentraland.org: src/components/Carousel).
 */
export function Carousel<T>({ items, renderItem, keyOf, autoplayDelay = 4000, slideWidth: desktopWidth = 750, alignItems = "stretch" }: CarouselProps<T>) {
  const isDesktop = useMediaQuery("(min-width: 991px)");
  const isMobile = useMediaQuery("(max-width: 599px)");
  const reduceMotion = useMediaQuery("(prefers-reduced-motion: reduce)");
  const total = items.length;
  const slides = [...items, ...items, ...items];

  const [pos, setPos] = useState(total);
  const [animated, setAnimated] = useState(true);
  // The first measured position is taken without a transition: hydration renders the unmeasured one first, and
  // a transition from it would have the browser rasterize the will-change track mid-animation, off its pixels.
  const [settled, setSettled] = useState(false);
  const [dragOffset, setDragOffset] = useState(0);
  const [viewportWidth, setViewportWidth] = useState(0);

  const wrapperRef = useRef<HTMLDivElement>(null);
  const autoRef = useRef<ReturnType<typeof setInterval>>(undefined);
  const movingRef = useRef(false);
  const startXRef = useRef(0);
  const dragRef = useRef(0);
  const draggingRef = useRef(false);
  const wasDragRef = useRef(false);

  useEffect(() => {
    const el = wrapperRef.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => setViewportWidth(entry.contentRect.width));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const mobilePadding = 32;
  const slideWidth = isDesktop ? desktopWidth : isMobile ? viewportWidth - mobilePadding : viewportWidth * 0.7;
  const gap = isDesktop ? 24 : isMobile ? mobilePadding : 16;
  const translateX = viewportWidth / 2 - slideWidth / 2 - pos * (slideWidth + gap) + dragOffset;
  const realIndex = ((pos % total) + total) % total;

  const step = useCallback((by: number) => {
    if (movingRef.current) return;
    movingRef.current = true;
    setAnimated(true);
    setPos((p) => p + by);
  }, []);

  const startAutoplay = useCallback(() => {
    clearInterval(autoRef.current);
    if (!autoplayDelay || reduceMotion) return;
    autoRef.current = setInterval(() => step(1), autoplayDelay);
  }, [autoplayDelay, reduceMotion, step]);

  useEffect(() => {
    startAutoplay();
    const onVisibility = () => (document.hidden ? clearInterval(autoRef.current) : startAutoplay());
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      clearInterval(autoRef.current);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [startAutoplay]);

  const onTransitionEnd = useCallback((e: TransitionEvent) => {
    if (e.target !== e.currentTarget) return;
    movingRef.current = false;
    setPos((p) => {
      if (p >= 2 * total) {
        setAnimated(false);
        return p - total;
      }
      if (p < total) {
        setAnimated(false);
        return p + total;
      }
      return p;
    });
  }, [total]);

  useEffect(() => {
    if (settled || viewportWidth === 0) return;
    let inner = 0;
    const outer = requestAnimationFrame(() => {
      inner = requestAnimationFrame(() => setSettled(true));
    });
    return () => {
      cancelAnimationFrame(outer);
      cancelAnimationFrame(inner);
    };
  }, [settled, viewportWidth]);

  useEffect(() => {
    if (animated) return;
    let inner = 0;
    const outer = requestAnimationFrame(() => {
      inner = requestAnimationFrame(() => setAnimated(true));
    });
    return () => {
      cancelAnimationFrame(outer);
      cancelAnimationFrame(inner);
    };
  }, [animated]);

  const beginDrag = useCallback((x: number) => {
    startXRef.current = x;
    dragRef.current = 0;
    draggingRef.current = true;
    wasDragRef.current = false;
    setAnimated(false);
    clearInterval(autoRef.current);
  }, []);

  const endDrag = useCallback(() => {
    if (!draggingRef.current) return;
    draggingRef.current = false;
    const dx = dragRef.current;
    dragRef.current = 0;
    setDragOffset(0);
    setAnimated(true);
    if (Math.abs(dx) > SWIPE_THRESHOLD) {
      movingRef.current = true;
      setPos((p) => p + (dx < 0 ? 1 : -1));
    }
    startAutoplay();
  }, [startAutoplay]);

  const moveDrag = useCallback((x: number) => {
    if (!draggingRef.current) return;
    dragRef.current = x - startXRef.current;
    if (Math.abs(dragRef.current) > 5) wasDragRef.current = true;
    setDragOffset(dragRef.current);
  }, []);

  useEffect(() => {
    const onMove = (e: globalThis.MouseEvent) => moveDrag(e.clientX);
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", endDrag);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", endDrag);
    };
  }, [moveDrag, endDrag]);

  return (
    <div ref={wrapperRef} className={styles.root} role="region" aria-roledescription="carousel">
      <div
        className={styles.track}
        style={{
          transform: `translateX(${translateX}px)`,
          transition: animated && settled ? "transform 0.4s ease" : "none",
          gap,
          alignItems,
          visibility: viewportWidth > 0 ? "visible" : "hidden",
        }}
        onMouseDown={(e: MouseEvent) => {
          if (e.button !== 0) return;
          e.preventDefault();
          beginDrag(e.clientX);
        }}
        onClickCapture={(e: MouseEvent) => {
          if (!wasDragRef.current) return;
          e.preventDefault();
          e.stopPropagation();
          wasDragRef.current = false;
        }}
        onTouchStart={(e: TouchEvent) => beginDrag(e.touches[0].clientX)}
        onTouchMove={(e: TouchEvent) => moveDrag(e.touches[0].clientX)}
        onTouchEnd={endDrag}
        onTransitionEnd={onTransitionEnd}
      >
        {slides.map((item, i) => (
          <div
            key={`${keyOf(item)}-${i}`}
            className={[styles.slide, i === pos ? styles.active : i === pos - 1 ? styles.prev : i === pos + 1 ? styles.next : ""].join(" ")}
            style={{ width: slideWidth || undefined, transition: animated && settled ? undefined : "none" }}
            aria-hidden={i < total || i >= 2 * total ? true : undefined}
          >
            {renderItem(item)}
          </div>
        ))}
      </div>
      <div className={styles.dots}>
        {items.map((item, i) => (
          <button
            key={keyOf(item)}
            type="button"
            className={styles.dot + (i === realIndex ? ` ${styles.dotActive}` : "")}
            aria-label={`Go to slide ${i + 1}`}
            aria-current={i === realIndex ? true : undefined}
            onClick={() => {
              if (i === realIndex || movingRef.current) return;
              movingRef.current = true;
              setAnimated(true);
              setPos(total + i);
              startAutoplay();
            }}
          />
        ))}
      </div>
      <button type="button" className={`${styles.nav} ${styles.navPrev}`} aria-label="Previous slide" onClick={() => { step(-1); startAutoplay(); }}>
        &#8249;
      </button>
      <button type="button" className={`${styles.nav} ${styles.navNext}`} aria-label="Next slide" onClick={() => { step(1); startAutoplay(); }}>
        &#8250;
      </button>
    </div>
  );
}
