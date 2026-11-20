import { useEffect, useId, useLayoutEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";

import { Button } from "@dcl/one-ui";
import { EditorKebabSmall, MenusShare } from "@dcl/one-ui/foundations/icons";

import styles from "./ProfileMoreActions.module.css";

export type MoreAction = { label: string; icon: ReactNode; disabled?: boolean; onSelect: () => void };

const GUTTER = 16;

/**
 * The three-dot button of another member's header and its menu. The shared library has no menu
 * component: this one opens below the button, moves with the arrow keys, Home and End, and closes
 * on a choice, Escape, Tab or a press outside.
 */
export function ProfileMoreActions({ actions, label }: { actions: readonly MoreAction[]; label?: string }) {
  const [open, setOpen] = useState<"first" | "last" | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const menuId = useId();

  const items = () => Array.from(menu.current?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)') ?? []);

  useLayoutEffect(() => {
    if (!open || !menu.current) return;
    // The menu ends at the button's right edge; on a narrow screen it moves right to stay on screen.
    menu.current.style.setProperty("--menu-shift", "0px");
    const { left } = menu.current.getBoundingClientRect();
    if (left < GUTTER) menu.current.style.setProperty("--menu-shift", `${GUTTER - left}px`);
    items().at(open === "first" ? 0 : -1)?.focus();
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const press = (event: PointerEvent) => {
      if (event.target instanceof Node && !root.current?.contains(event.target)) setOpen(null);
    };
    document.addEventListener("pointerdown", press);
    return () => document.removeEventListener("pointerdown", press);
  }, [open]);

  const close = () => {
    setOpen(null);
    trigger.current?.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!open) {
      if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
      event.preventDefault();
      setOpen(event.key === "ArrowDown" ? "first" : "last");
      return;
    }
    if (event.key === "Tab") {
      setOpen(null);
      return;
    }
    if (event.key === "Escape") {
      // Escape closes this only: an enclosing dialog or page handler does not see it.
      event.preventDefault();
      event.stopPropagation();
      close();
      return;
    }
    const list = items();
    const at = list.indexOf(document.activeElement as HTMLButtonElement);
    const next =
      event.key === "ArrowDown"
        ? list[(at + 1) % list.length]
        : event.key === "ArrowUp"
          ? list[(at - 1 + list.length) % list.length]
          : event.key === "Home"
            ? list[0]
            : event.key === "End"
              ? list.at(-1)
              : undefined;
    if (!next) return;
    event.preventDefault();
    next.focus();
  };

  const triggerContent = label
    ? { variant: "primary" as const, label, trailingIcon: <MenusShare size={18} aria-hidden="true" /> }
    : { variant: "iconOnly" as const, children: <EditorKebabSmall size={30} aria-hidden="true" /> };

  return (
    <div ref={root} className={styles.root} onKeyDown={onKeyDown}>
      <Button
        ref={trigger}
        size="md"
        {...triggerContent}
        aria-label={label ?? "More actions"}
        aria-haspopup="menu"
        aria-expanded={open !== null}
        aria-controls={open ? menuId : undefined}
        onClick={() => setOpen(open ? null : "first")}
      />
      {open ? (
        <div ref={menu} id={menuId} role="menu" aria-label={label ?? "More actions"} className={styles.menu}>
          {actions.map((action) => (
            <button
              key={action.label}
              type="button"
              role="menuitem"
              tabIndex={-1}
              className={styles.item}
              disabled={action.disabled}
              onClick={() => {
                close();
                action.onSelect();
              }}
            >
              {action.icon}
              {action.label}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}
