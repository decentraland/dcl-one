import { useCallback, useEffect, useId, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { createPortal } from "react-dom";
import "./tooltip.css";

type TooltipSide = "right" | "left" | "top" | "bottom";
type TooltipAlign = "start" | "center" | "end";

type TooltipProps = {
  /** Plain text of the tip. Give `label` or `content`. */
  label?: string;
  /** Rich content of the tip; replaces `label`. */
  content?: ReactNode;
  shortcut?: string;
  side?: TooltipSide;
  className?: string;
  /** Class for the tip itself: carries the token scope into the portal. */
  tipClassName?: string;
  children?: ReactNode;
  portal?: boolean;
  /** Controlled state. Hover and focus then only call `onOpenChange`. */
  open?: boolean;
  /** Called when hover, focus or a dismissal asks to open or close the tip. */
  onOpenChange?: (open: boolean) => void;
  /** Element the tip points at when it is not the wrapped children. */
  anchor?: Element | null;
  /** The pointer can move into the tip and use the controls in it. */
  interactive?: boolean;
  /** Widest the tip gets, in pixels; longer text wraps. */
  maxWidth?: number;
  /** Flip to the opposite side or shift so the tip stays inside the viewport. */
  keepInViewport?: boolean;
  /** Close when anything scrolls instead of following the anchor. */
  closeOnScroll?: boolean;
  /** Never open. */
  disabled?: boolean;
  /** No padding, background or shadow: the content draws its own surface. */
  bare?: boolean;
  /** Draw a pointer from the tip to the anchor. */
  arrow?: boolean;
  /** Milliseconds the pointer stays on the anchor before the tip opens. Focus opens it at once. */
  enterDelay?: number;
  /** The tip is the name of the wrapped control, not its description: for a control with no text of its own. */
  labelChild?: boolean;
  /** Edge of the anchor the tip lines up with. Needs `keepInViewport`. */
  align?: TooltipAlign;
  /** Put the tip edge that faces the anchor, and the edge the tip starts at on the other axis, on a whole device pixel. Needs `keepInViewport`. */
  snap?: boolean;
  /** Distance between the tip and the anchor, in pixels. */
  gap?: number;
};

type TipPosition = { left: number; top: number; side?: TooltipSide; ready?: boolean };

const GAP = 12;
const EDGE = 8;
const CLOSE_DELAY = 120;
const OPPOSITE: Record<TooltipSide, TooltipSide> = { right: "left", left: "right", top: "bottom", bottom: "top" };
const UNPLACED: TipPosition = { left: 0, top: 0 };

function toDevicePixel(value: number): number {
  const ratio = window.devicePixelRatio || 1;
  return Math.round(value * ratio) / ratio;
}

function placeInViewport(side: TooltipSide, rect: DOMRect, width: number, height: number, align: TooltipAlign, gap: number): TipPosition {
  const maxLeft = window.innerWidth - EDGE - width;
  const maxTop = window.innerHeight - EDGE - height;
  const along = (start: number, length: number, own: number) =>
    align === "start" ? start : align === "end" ? start + length - own : start + length / 2 - own / 2;
  const at = (s: TooltipSide) => ({
    left: s === "right" ? rect.right + gap : s === "left" ? rect.left - gap - width : along(rect.left, rect.width, width),
    top: s === "bottom" ? rect.bottom + gap : s === "top" ? rect.top - gap - height : along(rect.top, rect.height, height),
  });
  const fits = (s: TooltipSide) => {
    const p = at(s);
    return s === "right" || s === "left" ? p.left >= EDGE && p.left <= maxLeft : p.top >= EDGE && p.top <= maxTop;
  };
  const placed = fits(side) || !fits(OPPOSITE[side]) ? side : OPPOSITE[side];
  const p = at(placed);
  return { left: Math.max(EDGE, Math.min(p.left, maxLeft)), top: Math.max(EDGE, Math.min(p.top, maxTop)), side: placed, ready: true };
}

export default function Tooltip({
  label, content, shortcut, side = "right", className, tipClassName, children, portal = false,
  open: openProp, onOpenChange, anchor, interactive = false, maxWidth, keepInViewport = false, closeOnScroll = false,
  disabled = false, bare = false, arrow = false, enterDelay, labelChild = false, align = "center", snap = false, gap = GAP,
}: TooltipProps) {
  const ref = useRef<HTMLSpanElement>(null);
  const tipRef = useRef<HTMLSpanElement>(null);
  const id = useId();
  const controlled = openProp !== undefined;
  const anchored = anchor !== undefined;
  const floating = portal || controlled || anchored || interactive || keepInViewport || closeOnScroll || onOpenChange !== undefined || enterDelay !== undefined;
  const [openState, setOpen] = useState(false);
  const [position, setPosition] = useState<TipPosition>(UNPLACED);
  const open = !disabled && (controlled ? openProp : openState);

  const wanted = useRef(false);
  const closeTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const openTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const notify = useRef(onOpenChange);
  useEffect(() => { notify.current = onOpenChange; });
  useEffect(() => () => { clearTimeout(closeTimer.current); clearTimeout(openTimer.current); }, []);
  // A parent that refused a request leaves `wanted` wrong; its `open` is the state the next request is compared with.
  useEffect(() => { if (controlled) wanted.current = !!openProp; }, [controlled, openProp]);

  const request = useCallback((next: boolean) => {
    clearTimeout(closeTimer.current);
    clearTimeout(openTimer.current);
    if (wanted.current === next || (next && disabled)) return;
    wanted.current = next;
    if (!controlled) setOpen(next);
    notify.current?.(next);
  }, [controlled, disabled]);
  const enter = useCallback(() => {
    if (document.pointerLockElement) return;
    if (!enterDelay) return request(true);
    clearTimeout(closeTimer.current);
    clearTimeout(openTimer.current);
    openTimer.current = setTimeout(() => request(true), enterDelay);
  }, [request, enterDelay]);
  const leave = useCallback(() => {
    if (!interactive) return request(false);
    clearTimeout(closeTimer.current);
    clearTimeout(openTimer.current);
    closeTimer.current = setTimeout(() => request(false), CLOSE_DELAY);
  }, [interactive, request]);
  const focus = useCallback((target: EventTarget | null) => {
    if (target instanceof Element && target.matches(":focus-visible") && !document.pointerLockElement) request(true);
  }, [request]);
  const blur = useCallback((next: EventTarget | null) => {
    if (next instanceof Node && tipRef.current?.contains(next)) return;
    request(false);
  }, [request]);

  useEffect(() => {
    if (!floating || !anchor) return;
    const onFocus = (event: Event) => focus(event.target);
    const onBlur = (event: Event) => blur((event as FocusEvent).relatedTarget);
    anchor.addEventListener("mouseenter", enter);
    anchor.addEventListener("mouseleave", leave);
    anchor.addEventListener("focusin", onFocus);
    anchor.addEventListener("focusout", onBlur);
    return () => {
      anchor.removeEventListener("mouseenter", enter);
      anchor.removeEventListener("mouseleave", leave);
      anchor.removeEventListener("focusin", onFocus);
      anchor.removeEventListener("focusout", onBlur);
    };
  }, [floating, anchor, enter, leave, focus, blur]);
  useEffect(() => {
    if (!open || !anchor || labelChild) return;
    const before = anchor.getAttribute("aria-describedby");
    anchor.setAttribute("aria-describedby", id);
    return () => { if (before === null) anchor.removeAttribute("aria-describedby"); else anchor.setAttribute("aria-describedby", before); };
  }, [open, anchor, id, labelChild]);
  // A plain label is the name at all times; rich content names the control while the tip is in the page.
  const inPage = floating ? open : !disabled;
  useEffect(() => {
    if (!labelChild || disabled) return;
    const target = anchored ? anchor : ref.current?.firstElementChild;
    if (!target || target === tipRef.current) return;
    const attribute = content === undefined && label !== undefined ? "aria-label" : inPage ? "aria-labelledby" : null;
    if (!attribute || target.hasAttribute(attribute)) return;
    target.setAttribute(attribute, attribute === "aria-label" ? label ?? "" : id);
    return () => target.removeAttribute(attribute);
  }, [labelChild, disabled, anchored, anchor, content, label, inPage, id]);

  useEffect(() => {
    if (!floating || !open) return;
    // An open tip always reports its dismissal, also when the parent opened it with no hover.
    const close = () => { wanted.current = true; request(false); };
    const outside = (event: Event) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if ((anchored ? anchor : ref.current)?.contains(target) || tipRef.current?.contains(target)) return;
      close();
    };
    const key = (event: KeyboardEvent) => { if (event.key === "Escape") close(); };
    document.addEventListener("pointerdown", outside, true);
    document.addEventListener("focusin", outside, true);
    document.addEventListener("pointerlockchange", close);
    document.addEventListener("keydown", key);
    window.addEventListener("blur", close);
    if (closeOnScroll) window.addEventListener("scroll", close, true);
    return () => {
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("focusin", outside, true);
      document.removeEventListener("pointerlockchange", close);
      document.removeEventListener("keydown", key);
      window.removeEventListener("blur", close);
      if (closeOnScroll) window.removeEventListener("scroll", close, true);
    };
  }, [floating, open, anchored, anchor, closeOnScroll, request]);
  useEffect(() => {
    if (!floating || !open) return;
    const measure = () => {
      const rect = (anchored ? anchor : ref.current)?.getBoundingClientRect();
      if (!rect) return;
      if (keepInViewport) {
        const tip = tipRef.current?.getBoundingClientRect();
        if (!tip) return;
        const place = placeInViewport(side, rect, tip.width, tip.height, align, gap);
        setPosition(snap ? {
          ...place,
          left: place.side === "left" ? toDevicePixel(place.left + tip.width) - tip.width : toDevicePixel(place.left),
          top: place.side === "top" ? toDevicePixel(place.top + tip.height) - tip.height : toDevicePixel(place.top),
        } : place);
        return;
      }
      setPosition(side === "right" ? { left: rect.right + gap, top: rect.top + rect.height / 2 }
        : side === "left" ? { left: rect.left - gap, top: rect.top + rect.height / 2 }
        : { left: rect.left + rect.width / 2, top: side === "top" ? rect.top - gap : rect.bottom + gap });
    };
    measure();
    window.addEventListener("scroll", measure, true);
    window.addEventListener("resize", measure);
    const resize = keepInViewport && tipRef.current && typeof ResizeObserver !== "undefined" ? new ResizeObserver(measure) : null;
    if (resize && tipRef.current) resize.observe(tipRef.current);
    return () => {
      window.removeEventListener("scroll", measure, true);
      window.removeEventListener("resize", measure);
      resize?.disconnect();
      if (keepInViewport) setPosition(UNPLACED);
    };
  }, [floating, open, side, align, anchored, anchor, keepInViewport, snap, gap]);

  const placed = (keepInViewport && position.side) || side;
  const transform = side === "right" ? "translateY(-50%)" : side === "left" ? "translate(-100%, -50%)" : side === "top" ? "translate(-50%, -100%)" : "translateX(-50%)";
  const fixed: CSSProperties | undefined = !floating ? undefined
    : keepInViewport ? { position: "fixed", left: position.left, top: position.top, right: "auto", bottom: "auto", transform: "none", opacity: 1, visibility: position.ready ? undefined : "hidden" }
    : { position: "fixed", left: position.left, top: position.top, right: "auto", bottom: "auto", transform, opacity: 1 };
  // The tip in the page (not floating) is placed by the stylesheet, which reads the gap from `--tt-gap`.
  const style = maxWidth === undefined && gap === GAP ? fixed : ({
    ...fixed,
    ...(maxWidth === undefined ? null : { "--tt-max-width": maxWidth + "px" }),
    ...(gap === GAP ? null : { "--tt-gap": gap + "px" }),
  } as CSSProperties);
  const tipClass = "tt__tip tt__tip--" + placed
    + (content !== undefined ? " tt__tip--rich" : "")
    + (maxWidth !== undefined ? " tt__tip--wrap" : "")
    + (interactive ? " tt__tip--interactive" : "")
    + (bare ? " tt__tip--bare" : "")
    + (arrow ? " tt__tip--arrow" : "")
    + (tipClassName ? " " + tipClassName : "");
  const tip = <span ref={tipRef} id={floating || labelChild ? id : undefined} className={tipClass} role="tooltip" style={style} onMouseEnter={interactive ? enter : undefined} onMouseLeave={interactive ? leave : undefined}>
    {content ?? label}{shortcut ? <span className="tt__shortcut">[{shortcut}]</span> : null}
  </span>;
  if (anchored) return <>{children}{open && anchor ? createPortal(tip, document.body) : null}</>;
  return (
    <span ref={ref} className={"tt__wrap" + (className ? " " + className : "")} aria-describedby={floating && open && !labelChild ? id : undefined} onMouseEnter={floating ? enter : undefined} onMouseLeave={floating ? leave : undefined} onFocus={floating ? (event) => focus(event.target) : undefined} onBlur={floating ? (event) => blur(event.relatedTarget) : undefined}>
      {children}
      {floating ? open && createPortal(tip, document.body) : disabled ? null : tip}
    </span>
  );
}
