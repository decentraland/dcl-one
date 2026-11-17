import type { RefObject } from "react";
import { useLayoutEffect, useState } from "react";

export type OverlaySide = "top" | "bottom" | "left" | "right";
export type OverlayPlacement = OverlaySide | `${OverlaySide}-start` | `${OverlaySide}-end`;
/** An element, or anything with a box: a point from a pointer event places a context menu. */
export type OverlayAnchor = Element | { getBoundingClientRect: () => DOMRect } | null | undefined;

export type OverlayPositionOptions = {
  placement?: OverlayPlacement;
  /** Distance from the anchor, in pixels. */
  offset?: number;
  /** Shift along the anchor edge, in pixels. */
  crossOffset?: number;
  /** Use the opposite side when the surface does not fit on the asked one. */
  flip?: boolean;
  /** Smallest distance from the viewport edge, in pixels. */
  edge?: number;
};

export type OverlayPosition = { left: number; top: number; placement: OverlayPlacement; anchorWidth: number; ready: boolean };

type Box = { left: number; top: number; right: number; bottom: number; width: number; height: number };
type Size = { width: number; height: number };

const OPPOSITE: Record<OverlaySide, OverlaySide> = { top: "bottom", bottom: "top", left: "right", right: "left" };
const UNPLACED: OverlayPosition = { left: 0, top: 0, placement: "bottom-start", anchorWidth: 0, ready: false };

/** Where a surface of `size` goes for an anchor box inside a viewport. No DOM access. */
export function computeOverlayPosition(
  anchor: Box, size: Size, viewport: Size,
  { placement = "bottom-start", offset = 0, crossOffset = 0, flip = true, edge = 8 }: OverlayPositionOptions = {},
): { left: number; top: number; placement: OverlayPlacement } {
  const [side, align = "center"] = placement.split("-") as [OverlaySide, ("start" | "end")?];
  const vertical = side === "top" || side === "bottom";
  const along = (start: number, length: number, own: number) =>
    (align === "start" ? start : align === "end" ? start + length - own : start + length / 2 - own / 2) + crossOffset;
  const at = (s: OverlaySide) => ({
    left: s === "right" ? anchor.right + offset : s === "left" ? anchor.left - offset - size.width : along(anchor.left, anchor.width, size.width),
    top: s === "bottom" ? anchor.bottom + offset : s === "top" ? anchor.top - offset - size.height : along(anchor.top, anchor.height, size.height),
  });
  const maxLeft = viewport.width - edge - size.width;
  const maxTop = viewport.height - edge - size.height;
  const fits = (s: OverlaySide) => {
    const p = at(s);
    return vertical ? p.top >= edge && p.top <= maxTop : p.left >= edge && p.left <= maxLeft;
  };
  const placed = !flip || fits(side) || !fits(OPPOSITE[side]) ? side : OPPOSITE[side];
  const p = at(placed);
  return {
    left: Math.max(edge, Math.min(p.left, maxLeft)),
    top: Math.max(edge, Math.min(p.top, maxTop)),
    placement: (align === "center" ? placed : `${placed}-${align}`) as OverlayPlacement,
  };
}

/**
 * Fixed position of a floating surface next to its anchor. It is measured before paint and again when the
 * page scrolls, the window changes size, or the anchor or the surface change size.
 */
export function useOverlayPosition(
  open: boolean, anchor: OverlayAnchor, floating: RefObject<HTMLElement | null>,
  { placement = "bottom-start", offset = 0, crossOffset = 0, flip = true, edge = 8 }: OverlayPositionOptions = {},
): OverlayPosition {
  const [position, setPosition] = useState<OverlayPosition>(UNPLACED);

  useLayoutEffect(() => {
    if (!open || !anchor) return;
    const measure = () => {
      const surface = floating.current;
      if (!surface) return;
      const rect = anchor.getBoundingClientRect();
      const own = surface.getBoundingClientRect();
      const next = computeOverlayPosition(
        rect, { width: own.width, height: own.height }, { width: window.innerWidth, height: window.innerHeight },
        { placement, offset, crossOffset, flip, edge },
      );
      setPosition((prev) =>
        prev.ready && prev.left === next.left && prev.top === next.top && prev.placement === next.placement && prev.anchorWidth === rect.width
          ? prev
          : { ...next, anchorWidth: rect.width, ready: true });
    };
    measure();
    window.addEventListener("scroll", measure, true);
    window.addEventListener("resize", measure);
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(measure);
    if (observer && floating.current) observer.observe(floating.current);
    if (observer && anchor instanceof Element) observer.observe(anchor);
    return () => {
      window.removeEventListener("scroll", measure, true);
      window.removeEventListener("resize", measure);
      observer?.disconnect();
      setPosition(UNPLACED);
    };
  }, [open, anchor, floating, placement, offset, crossOffset, flip, edge]);

  return position;
}
