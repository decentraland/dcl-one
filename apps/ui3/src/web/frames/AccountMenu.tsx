import type { ReactNode } from "react";
import { useEffect, useRef, useState } from "react";
import type { ChromeNavigate } from "./chrome-nav";
import ChromeLink from "./ChromeLink";
import "./accountmenu.css";
import { truncateAddress } from "../../data/format";

type AccountMenuLink = { label: string; href: string };

type AccountMenuProps = {
  name?: string;
  account?: string;
  links?: AccountMenuLink[];
  onSignOut?: () => void;
  onSwitchAccount?: () => void;
  onNavigate?: ChromeNavigate;
  triggerClassName?: string;
  triggerLabel?: string;
  align?: "left" | "right";
  placement?: "below" | "above";
  className?: string;
  children: ReactNode;
};

export default function AccountMenu({
  name = "",
  account = "",
  links = [],
  onSignOut = undefined,
  onSwitchAccount = undefined,
  onNavigate = undefined,
  triggerClassName = "",
  triggerLabel = "My account",
  align = "right",
  placement = "below",
  className = "",
  children,
}: AccountMenuProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const triggerRef = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node))
        setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    return () => {
      document.removeEventListener("mousedown", onDown);
    };
  }, [open]);

  const close = () => setOpen(false);
  const run = (fn: () => void) => () => {
    close();
    fn();
  };

  return (
    <div
      className={"acm" + (className ? " " + className : "")}
      ref={rootRef}
      onKeyDown={(event) => {
        if (!open || event.defaultPrevented || event.key !== "Escape") return;
        event.preventDefault();
        event.stopPropagation();
        setOpen(false);
        triggerRef.current?.focus();
      }}
      onBlur={(event) => {
        if (
          event.relatedTarget &&
          !event.currentTarget.contains(event.relatedTarget)
        )
          setOpen(false);
      }}
    >
      <button
        ref={triggerRef}
        type="button"
        className={triggerClassName}
        aria-label={triggerLabel}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => {
          if (!open) triggerRef.current?.focus();
          setOpen((v) => !v);
        }}
      >
        {children}
      </button>
      {open ? (
        <div
          className={`acm__menu acm__menu--${align} acm__menu--${placement}`}
          role="menu"
          tabIndex={-1}
          aria-label={triggerLabel}
        >
          <div
            className="acm__head"
            onClick={() => triggerRef.current?.focus({ preventScroll: true })}
          >
            <span className="acm__name">{name || "My Account"}</span>
            {account ? (
              <span className="acm__addr" title={account}>
                {truncateAddress(account)}
              </span>
            ) : null}
          </div>
          {links.map((l) => (
            <ChromeLink
              key={l.href + l.label}
              className="acm__item"
              role="menuitem"
              href={l.href}
              onNavigate={onNavigate}
              onClick={() => {
                close();
              }}
            >
              {l.label}
            </ChromeLink>
          ))}
          {onSwitchAccount ? (
            <button
              type="button"
              className="acm__item"
              role="menuitem"
              onClick={run(onSwitchAccount)}
            >
              Switch account
            </button>
          ) : null}
          {onSignOut ? (
            <button
              type="button"
              className="acm__item acm__item--signout"
              role="menuitem"
              onClick={run(onSignOut)}
            >
              Log out
            </button>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
