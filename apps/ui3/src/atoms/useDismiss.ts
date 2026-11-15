import type { RefObject } from "react";
import { useCallback, useEffect, useRef } from "react";
import { isTopLayer, pushLayer, type OverlayLayer } from "./overlayLayers";

export type DismissAnchor = Element | { getBoundingClientRect: () => DOMRect } | null | undefined;

type DismissOptions = {
  surface: RefObject<HTMLElement | null>;
  /** The control that opened the surface: a press on it is not an outside press. */
  anchor?: DismissAnchor;
  onEscape?: () => void;
  onOutside?: (event: Event) => void;
  modal?: boolean;
};

/**
 * While `open`, the surface is a layer: the top layer gets Escape, and a press outside it calls `onOutside`.
 * Returns a function that tells whether the surface is the top layer now.
 */
export function useDismiss(open: boolean, { surface, anchor, onEscape, onOutside, modal = false }: DismissOptions) {
  const handlers = useRef({ onEscape, onOutside });
  useEffect(() => { handlers.current = { onEscape, onOutside }; });

  const closesOnEscape = onEscape !== undefined;
  const own = useRef<OverlayLayer | null>(null);
  useEffect(() => {
    if (!open) return;
    const layer: OverlayLayer = {
      modal,
      contains: (target) => !!surface.current?.contains(target) || (anchor instanceof Element && anchor.contains(target)),
      onEscape: closesOnEscape ? () => handlers.current.onEscape?.() : undefined,
      onOutside: (event) => handlers.current.onOutside?.(event),
    };
    own.current = layer;
    const remove = pushLayer(layer);
    return () => {
      remove();
      own.current = null;
    };
  }, [open, surface, anchor, modal, closesOnEscape]);

  return useCallback(() => own.current !== null && isTopLayer(own.current), []);
}
