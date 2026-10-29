import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import ContextMenu, { type ContextMenuItem } from "../../components/ContextMenu";
import "./compactnavigation.css";

/** A touch-sized section picker, using the same keyboard menu as desktop. */
export default function CompactNavigation({ label, name, icon, items, className = "" }: {
  label: ReactNode;
  name: string;
  icon?: ReactNode;
  items: ContextMenuItem[];
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const close = () => { setOpen(false); trigger.current?.focus(); };
  // Wherever the trigger sits, the open menu stays inside the viewport horizontally.
  useLayoutEffect(() => {
    const el = menu.current;
    if (!open || !el) return;
    el.style.translate = "";
    const box = el.getBoundingClientRect(), edge = 8;
    const shift = box.right > innerWidth - edge ? innerWidth - edge - box.right : box.left < edge ? edge - box.left : 0;
    if (shift) el.style.translate = `${Math.round(shift)}px 0`;
  }, [open]);
  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Node && !root.current?.contains(event.target)) setOpen(false);
    };
    const resize = () => setOpen(false);
    document.addEventListener("pointerdown", outside);
    window.addEventListener("resize", resize);
    return () => {
      document.removeEventListener("pointerdown", outside);
      window.removeEventListener("resize", resize);
    };
  }, [open]);
  return <div className={`compact-nav ${className}`} ref={root}
    onBlur={(event) => { if (!event.currentTarget.contains(event.relatedTarget)) setOpen(false); }}>
    <button type="button" ref={trigger} className="compact-nav__trigger" aria-label={name}
      aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen(value => !value)}
      onKeyDown={(event) => { if (event.key === "ArrowDown") { event.preventDefault(); setOpen(true); } }}>
      {icon && <span className="compact-nav__icon" aria-hidden="true">{icon}</span>}
      <span className="compact-nav__label">{label}</span>
      <svg className="compact-nav__chevron" viewBox="0 0 16 16" aria-hidden="true"><path d="m4 6 4 4 4-4" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" /></svg>
    </button>
    {open && <div className="compact-nav__menu" ref={menu} onKeyDown={(event) => { if (event.key === "Escape") event.stopPropagation(); }}>
      <ContextMenu autoFocus onClose={close} items={items.map(item => item.kind === "button" || item.kind === "submenu"
        ? { ...item, onClick: (event) => { item.onClick?.(event); close(); } } : item)} />
    </div>}
  </div>;
}
