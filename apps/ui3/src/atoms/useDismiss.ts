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
  const options = useRef({ anchor, onEscape, onOutside, modal });
  useEffect(() => { options.current = { anchor, onEscape, onOutside, modal }; });

  // The layer is pushed once per opening and reads its options when it is used: a change of an option keeps its
  // place under the layers opened after it.
  const own = useRef<OverlayLayer | null>(null);
  useEffect(() => {
    if (!open) return;
    const layer: OverlayLayer = {
      get modal() { return options.current.modal; },
      contains: (target) => {
        const { anchor } = options.current;
        return !!surface.current?.contains(target) || (anchor instanceof Element && anchor.contains(target));
      },
      get onEscape() {
        return options.current.onEscape && (() => options.current.onEscape?.());
      },
      onOutside: (event) => options.current.onOutside?.(event),
    };
    own.current = layer;
    const remove = pushLayer(layer);
    return () => {
      remove();
      own.current = null;
    };
  }, [open, surface]);

  return useCallback(() => own.current !== null && isTopLayer(own.current), []);
}
