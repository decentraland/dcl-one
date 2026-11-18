import type { CSSProperties, HTMLAttributes, ReactNode, Ref } from "react";
import { forwardRef, useEffect, useLayoutEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { useDismiss } from "./useDismiss";
import { useOverlayPosition, type OverlayAnchor, type OverlayPlacement } from "./useOverlayPosition";
import { useScrollLock } from "./useScrollLock";
import styles from "./Popover.module.css";

export type PopoverCloseReason = "escape" | "outside";

export type PopoverProps = Omit<HTMLAttributes<HTMLDivElement>, "children"> & {
  open: boolean;
  /** The element the surface is placed next to, or a box for a free position. */
  anchor: OverlayAnchor;
  onClose?: (reason: PopoverCloseReason) => void;
  placement?: OverlayPlacement;
  /** Distance from the anchor, in pixels. */
  offset?: number;
  /** Shift along the anchor edge, in pixels. */
  crossOffset?: number;
  flip?: boolean;
  /** The surface is at least as wide as the anchor. */
  matchAnchorWidth?: boolean;
  /** Move focus to the surface when it opens. */
  autoFocus?: boolean;
  /** When the surface closes with focus inside it, focus goes back to where it was before. */
  returnFocus?: boolean;
  closeOnEscape?: boolean;
  closeOnOutside?: boolean;
  /**
   * A clear layer with no paint covers the page under the surface. A press on it closes the surface, does not
   * move focus and reaches no control of the page. The page behind does not scroll, unless `lockScroll` is false.
   */
  modal?: boolean;
  /** Stop the page behind from scrolling while the surface is open. The default is the value of `modal`. */
  lockScroll?: boolean;
  /** No background, border, radius or shadow: the content draws its own surface. */
  bare?: boolean;
  children?: ReactNode;
};

function assignRef<T>(ref: Ref<T> | undefined, value: T | null) {
  if (typeof ref === "function") ref(value);
  else if (ref) (ref as { current: T | null }).current = value;
}

/** A floating surface in a portal on `document.body`, placed next to an anchor. It has no role of its own. */
const Popover = forwardRef<HTMLDivElement, PopoverProps>(function Popover({
  open, anchor, onClose, placement = "bottom-start", offset = 8, crossOffset = 0, flip = true, matchAnchorWidth = false,
  autoFocus = false, returnFocus = true, closeOnEscape = true, closeOnOutside = true, modal = false, lockScroll = modal,
  bare = false,
  className, style, children, tabIndex, ...rest
}, forwardedRef) {
  const surface = useRef<HTMLDivElement | null>(null);
  // The lock changes the page width: it comes before the first measure of the anchor.
  useScrollLock(open && lockScroll && !!anchor);
  const position = useOverlayPosition(open, anchor, surface, { placement, offset, crossOffset, flip });

  useDismiss(open, {
    surface, anchor, modal,
    onEscape: closeOnEscape && onClose ? () => onClose("escape") : undefined,
    // A modal surface gets its outside presses from its own layer under it.
    onOutside: !modal && closeOnOutside && onClose ? () => onClose("outside") : undefined,
  });

  const latest = useRef({ anchor, autoFocus, returnFocus });
  useEffect(() => { latest.current = { anchor, autoFocus, returnFocus }; });
  const before = useRef<Element | null>(null);
  useLayoutEffect(() => {
    if (!open) return;
    before.current = document.activeElement;
    if (latest.current.autoFocus) surface.current?.focus({ preventScroll: true });
  }, [open]);
  useEffect(() => {
    if (!open) return;
    return () => {
      const { anchor: opener, returnFocus: wanted } = latest.current;
      if (!wanted) return;
      // The surface is gone: focus that was inside it is now on the body. Focus that moved elsewhere stays there.
      const active = document.activeElement;
      if (active && active !== document.body) return;
      const previous = before.current;
      const target = previous instanceof HTMLElement && previous !== document.body && previous.isConnected ? previous
        : opener instanceof HTMLElement ? opener : null;
      target?.focus({ preventScroll: true });
    };
  }, [open]);

  if (!open || !anchor || typeof document === "undefined") return null;

  // The layer token of a floating surface can be under the one of a modal dialog: the surface says where it opened.
  const overModal = anchor instanceof Element && anchor.closest('[aria-modal="true"], [data-over-modal]') !== null;
  const geometry = {
    "--popover-left": position.left + "px",
    "--popover-top": position.top + "px",
    ...(matchAnchorWidth ? { "--popover-min-width": position.anchorWidth + "px" } : null),
  } as CSSProperties;

  return createPortal(
    <>
      {modal ? (
        <div
          className={styles.backdrop} aria-hidden="true" data-over-modal={overModal || undefined}
          // The press keeps focus where it is, and it is not a press on the tree that rendered the surface.
          onMouseDown={(event) => event.preventDefault()}
          onClick={(event) => {
            event.stopPropagation();
            if (closeOnOutside) onClose?.("outside");
          }}
        />
      ) : null}
      <div
        {...rest}
        ref={(node) => { surface.current = node; assignRef(forwardedRef, node); }}
        className={styles.root + (className ? " " + className : "")}
        style={{ ...geometry, ...style }}
        data-placement={position.placement}
        data-placed={position.ready || undefined}
        data-bare={bare || undefined}
        data-over-modal={overModal || undefined}
        tabIndex={tabIndex ?? (autoFocus ? -1 : undefined)}
      >
        {children}
      </div>
    </>,
    document.body,
  );
});

export default Popover;
