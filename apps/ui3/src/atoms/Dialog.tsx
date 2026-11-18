import type { CSSProperties, KeyboardEvent, ReactNode, RefObject } from "react";
import { useEffect, useId, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { wasOutsidePress } from "./overlayLayers";
import { useDismiss } from "./useDismiss";
import { useScrollLock } from "./useScrollLock";
import styles from "./Dialog.module.css";

export type DialogCloseReason = "escape" | "backdrop" | "close";

export type DialogProps = {
  open: boolean;
  /** With no `onClose` the dialog has no close button and stays open on Escape and on a backdrop press. */
  onClose?: (reason: DialogCloseReason) => void;
  /** Names the dialog. With no title, give `aria-label` or `aria-labelledby`. */
  title?: ReactNode;
  /** Element of the title. The look does not change with it. */
  titleAs?: "h1" | "h2" | "h3" | "h4" | "h5" | "h6";
  subtitle?: ReactNode;
  /** Content above the title row, edge to edge. */
  header?: ReactNode;
  /** Buttons under the content; each one takes an equal share of the row. */
  actions?: ReactNode;
  /** Shows the back button before the title. */
  onBack?: () => void;
  size?: "tiny" | "small" | "medium";
  /** Replaces the width of `size`. A number is in pixels. */
  width?: number | string;
  closeIcon?: ReactNode;
  backIcon?: ReactNode;
  closeLabel?: string;
  backLabel?: string;
  closeOnBackdrop?: boolean;
  closeOnEscape?: boolean;
  /** Element that takes focus when the dialog opens. The default is the dialog itself. */
  initialFocus?: RefObject<HTMLElement | null>;
  /** Stop the page behind from scrolling while the dialog is open. */
  lockScroll?: boolean;
  role?: "dialog" | "alertdialog";
  /** No title row and no padding: the children draw the whole content. */
  bare?: boolean;
  className?: string;
  backdropClassName?: string;
  "aria-label"?: string;
  "aria-labelledby"?: string;
  "aria-describedby"?: string;
  /** Goes on the dialog element, for tests. */
  "data-testid"?: string;
  children?: ReactNode;
};

const TABBABLE = 'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

const cross = (
  <svg viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true" focusable="false">
    <path d="M6 6l12 12M18 6L6 18" />
  </svg>
);
const chevron = (
  <svg viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false">
    <path d="M15 5l-7 7 7 7" />
  </svg>
);

/** A modal dialog in a portal on `document.body`: focus stays inside it and goes back to the opener on close. */
// How many open dialogs hold each element inert. An element is inert while any dialog holds it, whatever order
// the dialogs close in; one that was inert before the first dialog opened stays inert.
const inertHolds = new Map<Element, number>();
function holdInert(element: Element) {
  const holds = inertHolds.get(element);
  if (holds === undefined) {
    if (element.hasAttribute("inert")) return;
    element.setAttribute("inert", "");
    inertHolds.set(element, 1);
  } else {
    inertHolds.set(element, holds + 1);
  }
}
function releaseInert(element: Element) {
  const holds = inertHolds.get(element);
  if (holds === undefined) return;
  if (holds > 1) {
    inertHolds.set(element, holds - 1);
  } else {
    inertHolds.delete(element);
    element.removeAttribute("inert");
  }
}

export default function Dialog({
  open, onClose, title, titleAs: Title = "h2", subtitle, header, actions, onBack, size = "medium", width,
  closeIcon = cross, backIcon = chevron, closeLabel = "close", backLabel = "back",
  closeOnBackdrop = true, closeOnEscape = true, initialFocus, lockScroll = true, role = "dialog", bare = false,
  className, backdropClassName, children,
  "aria-label": ariaLabel, "aria-labelledby": ariaLabelledBy, "aria-describedby": ariaDescribedBy, "data-testid": testId,
}: DialogProps) {
  const paper = useRef<HTMLDivElement | null>(null);
  const pressedBackdrop = useRef(false);
  const titleId = useId();

  const isTopLayer = useDismiss(open, { surface: paper, modal: true, onEscape: closeOnEscape && onClose ? () => onClose("escape") : undefined });

  // The opener is read in the render that opens the dialog: a control in it with `autoFocus` takes focus before any effect.
  const [opened, setOpened] = useState<{ open: boolean; opener: Element | null }>({ open: false, opener: null });
  if (opened.open !== open) setOpened({ open, opener: open && typeof document !== "undefined" ? document.activeElement : null });
  const opener = opened.opener;

  // The page behind the dialog is inert while it is open: no focus, no pointer, out of the accessibility tree.
  // A dialog over another one makes the lower one inert and gives it back on close. Live regions stay readable,
  // and layers that open after the dialog (a select, a menu) come into the page after this and stay usable.
  useEffect(() => {
    const backdrop = paper.current?.parentElement;
    if (!open || !backdrop) return;
    const held: Element[] = [];
    for (const element of Array.from(document.body.children)) {
      if (element === backdrop || element.hasAttribute("aria-live")) continue;
      holdInert(element);
      held.push(element);
    }
    return () => { for (const element of held) releaseInert(element); };
  }, [open]);

  const firstFocus = useRef(initialFocus);
  useEffect(() => { firstFocus.current = initialFocus; });
  useEffect(() => {
    if (!open) return;
    const node = paper.current;
    // The update that opened the dialog can also remove the opener (a menu item, the dialog of a step before).
    // Focus is then where the cleanups of that update put it, outside this dialog.
    const active = document.activeElement;
    const back = opener?.isConnected ? opener
      : active && active !== document.body && !node?.contains(active) ? active : null;
    if (node && !node.contains(active)) (firstFocus.current?.current ?? node).focus({ preventScroll: true });
    return () => { if (back instanceof HTMLElement && back.isConnected) back.focus({ preventScroll: true }); };
  }, [open, opener]);

  useScrollLock(open && lockScroll);

  if (!open || typeof document === "undefined") return null;

  // Keys from a menu or a select opened from the dialog arrive here through the React tree; they are not the dialog's.
  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const node = paper.current;
    if (event.key !== "Tab" || !node || !(event.target instanceof Node) || !node.contains(event.target)) return;
    // A button out of the tab order (a tab that is not selected) and a control in a hidden part are not stops.
    const tabbable = Array.from(node.querySelectorAll<HTMLElement>(TABBABLE)).filter((item) => item.tabIndex >= 0 && !item.closest("[hidden]"));
    const first = tabbable[0];
    const last = tabbable[tabbable.length - 1];
    if (!first || !last) { event.preventDefault(); node.focus(); return; }
    const active = document.activeElement;
    if (event.shiftKey && (active === first || active === node)) { event.preventDefault(); last.focus(); }
    else if (!event.shiftKey && active === last) { event.preventDefault(); first.focus(); }
  }

  const hasTitleRow = !bare && (title !== undefined || onClose !== undefined || onBack !== undefined);
  const style = width === undefined ? undefined : ({ "--dialog-width": typeof width === "number" ? width + "px" : width } as CSSProperties);

  return createPortal(
    <div
      className={styles.backdrop + (backdropClassName ? " " + backdropClassName : "")}
      // A press on the backdrop while a menu or a select is open over the dialog closes that one, not the dialog.
      // In a browser that close can commit before this handler runs, so the dialog is the top layer again here.
      onPointerDown={(event) => {
        pressedBackdrop.current = event.target === event.currentTarget && isTopLayer() && !wasOutsidePress(event.nativeEvent);
      }}
      onClick={(event) => {
        // A press that starts in the dialog and ends on the backdrop (a text selection) does not close it.
        const onBackdrop = pressedBackdrop.current && event.target === event.currentTarget;
        pressedBackdrop.current = false;
        if (onBackdrop && closeOnBackdrop) onClose?.("backdrop");
      }}
    >
      <div
        ref={paper} role={role} aria-modal="true" tabIndex={-1}
        aria-label={ariaLabel} aria-labelledby={ariaLabelledBy ?? (title !== undefined && !bare && !ariaLabel ? titleId : undefined)}
        aria-describedby={ariaDescribedBy} data-testid={testId}
        className={styles.paper + (className ? " " + className : "")} style={style}
        data-size={size} data-bare={bare || undefined}
        onKeyDown={onKeyDown}
      >
        {header}
        {hasTitleRow ? (
          <div className={styles.titleRow}>
            {onBack ? <button type="button" className={styles.iconButton} aria-label={backLabel} onClick={onBack}>{backIcon}</button> : null}
            {title !== undefined ? <Title id={titleId} className={styles.title}>{title}</Title> : null}
            {onClose ? <button type="button" className={styles.iconButton} data-edge="end" aria-label={closeLabel} onClick={() => onClose("close")}>{closeIcon}</button> : null}
          </div>
        ) : null}
        {subtitle !== undefined && !bare ? <p className={styles.subtitle}>{subtitle}</p> : null}
        {bare ? children : <div className={styles.content}>{children}</div>}
        {actions !== undefined && !bare ? <div className={styles.actions}>{actions}</div> : null}
      </div>
    </div>,
    document.body,
  );
}
