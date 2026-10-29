// from: shop@94584a5c:app/src/components/HeldCredits/HeldCredits.tsx
// Credits a purchase committed but has not yet been proven to spend. They come back on their own, and
// the panel says when only when there is an honest date: a reservation still inside its TTL counts
// down; one waiting on chain processing says there is no estimate. Opens on hover AND focus.

import { useEffect, useId, useRef, useState, type FocusEvent } from "react";

import { ClockIcon } from "../../../atoms/icons";
import type { HeldCredits as Held } from "../credits";
import { CreditsAmount } from "./Amount";
import "./heldcredits.css";

function formatRemaining(seconds: number): string {
  const safe = Math.max(0, seconds);
  return `${Math.floor(safe / 60)}:${String(safe % 60).padStart(2, "0")}`;
}

export default function HeldCredits({ held, now: fixedNow }: { held: Held | undefined; now?: number }) {
  const panelId = useId();
  const [open, setOpen] = useState(false);
  const [now, setNow] = useState(() => Math.floor((fixedNow ?? Date.now()) / 1000));
  const rootRef = useRef<HTMLDivElement>(null);
  const releasesAt = held?.releasesAtSeconds ?? null;
  const remaining = releasesAt === null ? 0 : releasesAt - now;
  const unknown = releasesAt === null || remaining <= 0;

  useEffect(() => {
    if (!held || fixedNow !== undefined) return;
    const id = setInterval(() => setNow(Math.floor(Date.now() / 1000)), 1000);
    return () => clearInterval(id);
  }, [held, fixedNow]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open]);

  if (!held || held.credits <= 0) return null;
  const hideUnlessInside = (e: FocusEvent<HTMLElement>) => {
    if (!rootRef.current?.contains(e.relatedTarget as Node | null)) setOpen(false);
  };
  const count = held.purchases.length;

  return (
    <div className="shop-held" ref={rootRef} onMouseEnter={() => setOpen(true)} onMouseLeave={() => setOpen(false)} data-testid="held-credits">
      <button type="button" className="shop-held__trigger" aria-describedby={open ? panelId : undefined} onFocus={() => setOpen(true)} onBlur={hideUnlessInside}>
        <ClockIcon size={14} />
        <CreditsAmount credits={held.credits} size={12} />
        <span>on hold</span>
      </button>
      {open ? (
        <div className="shop-held__panel" id={panelId} role="tooltip">
          <p className="shop-held__title">
            {held.credits} {held.credits === 1 ? "Credit is" : "Credits are"} on hold
          </p>
          <p>
            {count <= 1 ? "A purchase is still being confirmed." : `${count} purchases are still being confirmed.`}{" "}
            {unknown ? (
              <span data-testid="held-credits-unknown">They come back once the network confirms it; we can't say exactly when.</span>
            ) : (
              <span>
                Back in <b data-testid="held-credits-countdown">{formatRemaining(remaining)}</b>
              </span>
            )}
          </p>
          <p className="shop-held__caveat">Nothing is lost and there is nothing you need to do.</p>
        </div>
      ) : null}
    </div>
  );
}
