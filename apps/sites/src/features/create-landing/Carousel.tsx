import { useCallback, useEffect, useRef, useState, type FocusEvent, type KeyboardEvent, type MouseEvent, type ReactNode, type TouchEvent, type TransitionEvent } from "react";

import styles from "./Carousel.module.css";
import { useMediaQuery } from "@ui/data/hooks/useMediaQuery";

// The sizes, thresholds and the two breakpoints are the reference's own (decentraland.org:
// src/components/Carousel/Carousel.tsx), so the slider measures as it does there.
/** A drag longer than this many px moves one slide. */
const SWIPE_THRESHOLD = 50;
/** A pointer that moved more than this many px was a drag: its click does not follow a link. */
const DRAG_CLICK_THRESHOLD = 5;
/** On a phone the slide leaves 16 px on each side; the gap between slides is that same sum. */
const MOBILE_PADDING = 32;
const DESKTOP_GAP = 24;
const TABLET_GAP = 16;
/** Between the breakpoints a slide takes this share of the viewport, so its neighbours show. */
const TABLET_SLIDE_SHARE = 0.7;
/** How long a step slides. */
const SLIDE_MS = 400;
/** A step that got no transitionend ends this long after its slide: the browser sends none for a transition it cancelled or never ran. */
const SLIDE_END_SLACK_MS = 100;

type CarouselProps<T> = {
  /** The name of the carousel region. */
  label: string;
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
export function Carousel<T>({ label, items, renderItem, keyOf, autoplayDelay = 4000, slideWidth: desktopWidth = 750, alignItems = "stretch" }: CarouselProps<T>) {
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
  // A step with a control or a swipe ends autoplay for the life of the page; the pointer over the carousel or
  // focus inside it holds autoplay while it lasts.
  const [stopped, setStopped] = useState(false);
  const heldRef = useRef({ hover: false, focus: false });

  const wrapperRef = useRef<HTMLDivElement>(null);
  const autoRef = useRef<ReturnType<typeof setInterval>>(undefined);
  const movingRef = useRef(false);
  const landRef = useRef<ReturnType<typeof setTimeout>>(undefined);
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

  const slideWidth = isDesktop ? desktopWidth : isMobile ? viewportWidth - MOBILE_PADDING : viewportWidth * TABLET_SLIDE_SHARE;
  const gap = isDesktop ? DESKTOP_GAP : isMobile ? MOBILE_PADDING : TABLET_GAP;
  const translateX = viewportWidth / 2 - slideWidth / 2 - pos * (slideWidth + gap) + dragOffset;
  const realIndex = ((pos % total) + total) % total;

  // A step slides only once the track is measured and the visitor accepts motion.
  const slidesMove = settled && !reduceMotion;

  /** A step is over: the controls answer again, and a position on a copy goes to the middle copy with no slide. */
  const land = useCallback(() => {
    clearTimeout(landRef.current);
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

  const moveTo = useCallback((to: (p: number) => number) => {
    if (movingRef.current) return;
    if (!slidesMove) {
      // No transition runs, so no transitionend comes: the position lands in the middle copy at once.
      setPos((p) => {
        const next = to(p);
        return next >= 2 * total ? next - total : next < total ? next + total : next;
      });
      return;
    }
    movingRef.current = true;
    setAnimated(true);
    setPos(to);
    // The step ends at its transitionend. A press on the track cancels the transition, and a tab the browser
    // does not draw runs none: then this clock ends the step.
    clearTimeout(landRef.current);
    landRef.current = setTimeout(land, SLIDE_MS + SLIDE_END_SLACK_MS);
  }, [land, slidesMove, total]);

  const step = useCallback((by: number) => moveTo((p) => p + by), [moveTo]);

  const autoplays = Boolean(autoplayDelay) && !reduceMotion && !stopped;
  const startAutoplay = useCallback(() => {
    clearInterval(autoRef.current);
    if (!autoplays || heldRef.current.hover || heldRef.current.focus) return;
    autoRef.current = setInterval(() => step(1), autoplayDelay);
  }, [autoplayDelay, autoplays, step]);

  const hold = useCallback((kind: "hover" | "focus", held: boolean) => {
    heldRef.current[kind] = held;
    startAutoplay();
  }, [startAutoplay]);

  /** A step the visitor asked for. */
  const control = useCallback((move: () => void) => {
    setStopped(true);
    clearInterval(autoRef.current);
    move();
  }, []);

  useEffect(() => {
    startAutoplay();
    const onVisibility = () => (document.hidden ? clearInterval(autoRef.current) : startAutoplay());
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      clearInterval(autoRef.current);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [startAutoplay]);

  useEffect(() => () => clearTimeout(landRef.current), []);

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
    // The press takes the transition off the track, so a step it interrupts ends here.
    if (movingRef.current) land();
    setAnimated(false);
    clearInterval(autoRef.current);
  }, [land]);

  const endDrag = useCallback(() => {
    if (!draggingRef.current) return;
    draggingRef.current = false;
    const dx = dragRef.current;
    dragRef.current = 0;
    setDragOffset(0);
    setAnimated(true);
    if (Math.abs(dx) > SWIPE_THRESHOLD) control(() => step(dx < 0 ? 1 : -1));
    else startAutoplay();
  }, [control, startAutoplay, step]);

  const moveDrag = useCallback((x: number) => {
    if (!draggingRef.current) return;
    dragRef.current = x - startXRef.current;
    if (Math.abs(dragRef.current) > DRAG_CLICK_THRESHOLD) wasDragRef.current = true;
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

  if (total === 0) return null;

  return (
    <div
      ref={wrapperRef}
      className={styles.root}
      role="region"
      aria-roledescription="carousel"
      aria-label={label}
      onMouseEnter={() => hold("hover", true)}
      onMouseLeave={() => hold("hover", false)}
      onFocus={() => hold("focus", true)}
      onBlur={(e: FocusEvent) => {
        if (!e.currentTarget.contains(e.relatedTarget)) hold("focus", false);
      }}
      onKeyDown={(e: KeyboardEvent) => {
        // The arrow keys belong to a slide's own controls (its tabs) while focus is in a slide.
        if ((e.key !== "ArrowLeft" && e.key !== "ArrowRight") || (e.target as Element).closest(`.${styles.track}`)) return;
        e.preventDefault();
        control(() => step(e.key === "ArrowLeft" ? -1 : 1));
      }}
    >
      <div
        className={styles.track}
        aria-live={autoplays ? "off" : "polite"}
        style={{
          transform: `translateX(${translateX}px)`,
          transition: animated && slidesMove ? `transform ${SLIDE_MS}ms ease` : "none",
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
        onTouchCancel={endDrag}
        onTransitionEnd={(e: TransitionEvent) => {
          if (e.target === e.currentTarget) land();
        }}
      >
        {slides.map((item, i) => (
          <div
            key={`${keyOf(item)}-${i}`}
            className={[styles.slide, i === pos ? styles.active : i === pos - 1 ? styles.prev : i === pos + 1 ? styles.next : ""].join(" ")}
            style={{ width: slideWidth || undefined, transition: animated && slidesMove ? undefined : "none" }}
            // Only the slide in the middle takes focus and is read: the copies and the neighbours hold the same links.
            aria-hidden={i === pos ? undefined : true}
            inert={i === pos ? undefined : true}
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
              if (i !== realIndex) control(() => moveTo(() => total + i));
            }}
          />
        ))}
      </div>
      <button type="button" className={`${styles.nav} ${styles.navPrev}`} aria-label="Previous slide" onClick={() => control(() => step(-1))}>
        &#8249;
      </button>
      <button type="button" className={`${styles.nav} ${styles.navNext}`} aria-label="Next slide" onClick={() => control(() => step(1))}>
        &#8250;
      </button>
    </div>
  );
}
