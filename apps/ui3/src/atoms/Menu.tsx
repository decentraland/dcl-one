import type { ButtonHTMLAttributes, KeyboardEvent, ReactNode } from "react";
import { createContext, useContext, useEffect, useRef } from "react";
import Popover from "./Popover";
import type { OverlayAnchor, OverlayPlacement } from "./useOverlayPosition";
import styles from "./Menu.module.css";

export type MenuCloseReason = "escape" | "outside" | "tab" | "select";

export type MenuProps = {
  open: boolean;
  /** The control that opens the menu, or a box for a context menu. */
  anchor: OverlayAnchor;
  onClose?: (reason: MenuCloseReason) => void;
  placement?: OverlayPlacement;
  /** Distance from the anchor, in pixels. */
  offset?: number;
  /** Shift along the anchor edge, in pixels. */
  crossOffset?: number;
  /** The menu is at least as wide as the anchor. */
  matchAnchorWidth?: boolean;
  /** A click on an item also closes the menu, with the reason `select`. */
  closeOnSelect?: boolean;
  /** A press outside the menu reaches nothing else, and the page behind does not scroll. */
  modal?: boolean;
  /** Stop the page behind from scrolling while the menu is open. The default is the value of `modal`. */
  lockScroll?: boolean;
  id?: string;
  className?: string;
  "aria-label"?: string;
  "aria-labelledby"?: string;
  children?: ReactNode;
};

const TYPEAHEAD_RESET = 500;
const ITEMS = '[role="menuitem"]:not(:disabled)';
const CloseOnSelect = createContext<(() => void) | undefined>(undefined);

/** A list of commands next to its trigger. Focus moves into the menu when it opens and back when it closes. */
export function Menu({
  open, anchor, onClose, placement = "bottom-start", offset = 4, crossOffset, matchAnchorWidth, closeOnSelect = false, modal, lockScroll,
  id, className, children, ...aria
}: MenuProps) {
  const list = useRef<HTMLDivElement | null>(null);
  const typed = useRef({ text: "", at: 0 });

  const items = () => (list.current ? Array.from(list.current.querySelectorAll<HTMLElement>(ITEMS)) : []);
  const focusItem = (item: HTMLElement | undefined) => {
    if (!item) return;
    for (const other of items()) other.tabIndex = other === item ? 0 : -1;
    item.focus();
  };

  useEffect(() => {
    if (!open || !anchor) return;
    const all = items();
    focusItem(all.find((item) => item.dataset.selected === "true") ?? all[0]);
    if (all.length === 0) list.current?.focus();
  }, [open, anchor]);

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "Tab") {
      event.preventDefault();
      onClose?.("tab");
      return;
    }
    const all = items();
    if (all.length === 0) return;
    const current = document.activeElement instanceof HTMLElement ? all.indexOf(document.activeElement) : -1;
    let next: HTMLElement | undefined;
    if (event.key === "ArrowDown") next = all[current < 0 ? 0 : (current + 1) % all.length];
    else if (event.key === "ArrowUp") next = all[current < 0 ? all.length - 1 : (current - 1 + all.length) % all.length];
    else if (event.key === "Home") next = all[0];
    else if (event.key === "End") next = all[all.length - 1];
    else if (event.key.length === 1 && event.key !== " " && !event.ctrlKey && !event.metaKey && !event.altKey) {
      const now = Date.now();
      const text = (now - typed.current.at > TYPEAHEAD_RESET ? "" : typed.current.text) + event.key.toLowerCase();
      typed.current = { text, at: now };
      // One letter typed again goes to the next item with that letter; more letters search from the current item.
      const from = text.length === 1 ? current + 1 : Math.max(current, 0);
      const order = [...all.slice(from), ...all.slice(0, from)];
      next = order.find((item) => (item.textContent ?? "").trim().toLowerCase().startsWith(text));
      if (!next) return;
    } else return;
    event.preventDefault();
    focusItem(next);
  }

  return (
    <Popover
      open={open} anchor={anchor} placement={placement} offset={offset} crossOffset={crossOffset} matchAnchorWidth={matchAnchorWidth}
      onClose={onClose} className={className} modal={modal} lockScroll={lockScroll}
    >
      <CloseOnSelect.Provider value={closeOnSelect && onClose ? () => onClose("select") : undefined}>
        <div {...aria} ref={list} id={id} className={styles.list} role="menu" tabIndex={-1} onKeyDown={onKeyDown}>
          {children}
        </div>
      </CloseOnSelect.Provider>
    </Popover>
  );
}

export type MenuItemProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "type" | "role"> & {
  /** Decorative glyph before the label. */
  icon?: ReactNode;
  /** The current choice of a menu that picks one value. */
  selected?: boolean;
  tone?: "default" | "danger";
};

export function MenuItem({ icon, selected = false, tone = "default", disabled, className, onClick, children, ...rest }: MenuItemProps) {
  const close = useContext(CloseOnSelect);
  return (
    <button
      {...rest}
      type="button" role="menuitem" tabIndex={-1}
      className={styles.item + (className ? " " + className : "")}
      disabled={disabled} aria-disabled={disabled || undefined} data-selected={selected || undefined}
      data-tone={tone === "default" ? undefined : tone}
      onClick={(event) => { onClick?.(event); close?.(); }}
    >
      {icon ? <span className={styles.icon} aria-hidden="true">{icon}</span> : null}
      {children}
    </button>
  );
}

export function MenuSeparator({ className }: { className?: string }) {
  return <div className={styles.separator + (className ? " " + className : "")} role="separator" />;
}

export default Menu;
