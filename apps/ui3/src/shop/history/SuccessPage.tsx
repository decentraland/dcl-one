// from: shop@2d3bf62:app/src/pages/Success.tsx (+ Success.styles.ts, components/Confetti); marketplace@3bf3662f:webapp/src/components/SuccessPage/SuccessPage.tsx
//       (item, token and NAME successes). Marketplace UI: Apache-2.0, Copyright 2020 Metaverse Holdings Ltd.
//       Supersedes v2 ui3/src/marketplace/pages/MkSuccessPage.tsx.
import { useEffect, useRef, useState } from "react";
import { Link, Navigate, useLocation, useNavigate } from "react-router";

import Spinner from "../../atoms/Spinner";
import { useMediaQuery } from "../../data/hooks/useMediaQuery";
import { useShopWallet } from "../wallet/context";
import { explorerTxUrl } from "../checkout/chain/config";
import { formatCredits } from "../checkout/lib/currency";
import { CreatorBadge } from "../discovery/components/Hero";
import { myItemsRouteFor } from "../discovery/lib/routes";
import { showsWalletConfirmations } from "../checkout/lib/wallet-kind";
import { EnsureQueryClient } from "../checkout/services";
import CreditMark from "../credits/components/CreditMark";
import { isIapMode } from "../credits/iap";
import { backpackDeepLink, jumpUrl } from "../detail/lib/jump";
import { isSuccessNavState, loadReceipt, type ReceiptTx, type SuccessNavState } from "./receipt";
import { useSettlement, type OwnershipCheck } from "./useSettlement";
import "./successpage.css";

const CheckMark = () => (
  <svg viewBox="0 0 18 18" width="12" height="12" aria-hidden="true">
    <path d="M4 9l3.5 3.5L14 5" fill="none" stroke="#fff" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

const JumpInIcon = () => (
  <svg width="28" height="28" viewBox="0 0 28 28" fill="none" aria-hidden="true">
    <rect x="0.5" y="0.5" width="27" height="27" rx="7.5" fill="rgba(255,255,255,0.16)" stroke="rgba(255,255,255,0.3)" />
    <path d="M11 9h8v8M19 9l-10 10" stroke="#fcfcfc" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

const CONFETTI_COLORS = ["#ff2d55", "#ff7439", "#c640cd", "#691fa9", "#34ce77", "#ffc9d5", "#fcfcfc", "#ffbc5b"];

function Confetti() {
  const ref = useRef<HTMLCanvasElement>(null);
  const play = !useMediaQuery("(prefers-reduced-motion: reduce)", true);
  useEffect(() => {
    const canvas = ref.current;
    const ctx = canvas?.getContext?.("2d");
    if (!play || !canvas || !ctx) return;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const w = window.innerWidth;
    const h = window.innerHeight;
    canvas.width = w * dpr;
    canvas.height = h * dpr;
    ctx.scale(dpr, dpr);
    type P = { x: number; y: number; vx: number; vy: number; rot: number; vr: number; size: number; color: string; shape: number; born: number };
    const parts: P[] = [];
    const fire = (at: number) => {
      for (const side of [-1, 1]) {
        for (let i = 0; i < 70; i++) {
          const tilt = 0.2 + Math.random() * 0.6;
          const speed = (0.9 + Math.random() * 0.8) * Math.max(h, 600) * 0.0165;
          parts.push({
            x: side < 0 ? 0 : w,
            y: h * 0.85,
            vx: -side * Math.sin(tilt) * speed,
            vy: -Math.cos(tilt) * speed,
            rot: Math.random() * Math.PI,
            vr: (Math.random() - 0.5) * 0.3,
            size: 6 + Math.random() * 7,
            color: CONFETTI_COLORS[Math.floor(Math.random() * CONFETTI_COLORS.length)]!,
            shape: Math.random(),
            born: at,
          });
        }
      }
    };
    let raf = 0;
    const start = performance.now();
    let second = false;
    fire(0);
    const frame = (now: number) => {
      const t = now - start;
      if (!second && t > 900) {
        second = true;
        fire(t);
      }
      ctx.clearRect(0, 0, w, h);
      let alive = 0;
      for (const p of parts) {
        const age = t - p.born;
        if (age < 0 || age > 3600) continue;
        p.vy += 0.32;
        p.vx *= 0.985;
        p.vy *= 0.985;
        p.x += p.vx;
        p.y += p.vy;
        p.rot += p.vr;
        if (p.y > h + 40) continue;
        alive++;
        ctx.save();
        ctx.globalAlpha = age > 2800 ? Math.max(0, 1 - (age - 2800) / 800) : 1;
        ctx.translate(p.x, p.y);
        ctx.rotate(p.rot);
        ctx.fillStyle = p.color;
        if (p.shape < 0.5) ctx.fillRect(-p.size / 2, -p.size / 4, p.size, p.size / 2);
        else {
          ctx.beginPath();
          ctx.arc(0, 0, p.size / 3, 0, Math.PI * 2);
          ctx.fill();
        }
        ctx.restore();
      }
      if (alive > 0 || t < 1000) raf = requestAnimationFrame(frame);
      else ctx.clearRect(0, 0, w, h);
    };
    raf = requestAnimationFrame(frame);
    return () => cancelAnimationFrame(raf);
  }, [play]);
  if (!play) return null;
  return <canvas ref={ref} className="shop-success__confetti" aria-hidden="true" data-testid="confetti" />;
}

function Receipts({ txs, confirmed }: { txs: ReceiptTx[]; confirmed?: boolean }) {
  const links = txs.map((t) => ({ t, url: explorerTxUrl(t.chainId, t.hash) })).filter((x) => !!x.url);
  if (links.length === 0) return null;
  return (
    <div className="shop-success__links" data-receipt={confirmed ? "" : undefined}>
      {links.map(({ t, url }, i) => (
        <a key={t.hash} className="shop-success__receipt" href={url!} target="_blank" rel="noopener noreferrer">
          {links.length > 1 ? `Receipt ${i + 1} of ${links.length} ↗` : "View receipt ↗"}
        </a>
      ))}
    </div>
  );
}

function itemCount(n: number): string {
  return `${n} ${n === 1 ? "item" : "items"}`;
}

function SuccessScreen({ state }: { state: SuccessNavState }) {
  const wallet = useShopWallet();
  const navigate = useNavigate();
  const items = state.items;
  const first = items[0];
  const ownership: OwnershipCheck | null =
    wallet.account && first?.contractAddress && first.itemId ? { owner: wallet.account, contractAddress: first.contractAddress, itemId: first.itemId } : null;
  const settlement = useSettlement(state.txs, { skip: state.settled, ownership });
  const selfCustody = showsWalletConfirmations(wallet.host?.walletKind ?? null);
  const subject = items.length === 1 ? <strong>{items[0]!.name}</strong> : itemCount(items.length);

  if (settlement !== "confirmed") {
    return (
      <main className="shop-success" aria-labelledby="shop-success-title">
        {settlement === "pending" || settlement === "indexing" ? (
          <section className="shop-success__status" role="status" aria-live="polite" data-testid={settlement === "indexing" ? "success-indexing" : "success-pending"}>
            <Spinner size={40} />
            <h1 id="shop-success-title" className="shop-success__title">
              {settlement === "indexing" ? "Finalizing your purchase…" : "Processing your purchase…"}
            </h1>
            <p className="shop-success__sub">
              {settlement === "indexing" ? (
                <>Payment confirmed — adding {subject} to your wardrobe. Almost there.</>
              ) : (
                <>Confirming {subject} — this usually takes a few seconds. Keep this tab open.</>
              )}
            </p>
            {selfCustody ? <Receipts txs={state.txs} /> : null}
          </section>
        ) : settlement === "timed-out" ? (
          <section className="shop-success__status" role="status" data-testid="success-timed-out">
            <h1 id="shop-success-title" className="shop-success__title">Still processing…</h1>
            <p className="shop-success__sub">
              This is taking longer than usual to confirm. Your purchase may still complete — check <Link to="/shop/activity">Activity</Link> in a few minutes to
              see the final status. No need to buy again.
            </p>
            {selfCustody ? <Receipts txs={state.txs} /> : null}
            <div className="shop-success__actions">
              <Link className="shop-success__btn" data-variant="purple" to="/shop/activity">View your activity</Link>
              <Link className="shop-success__btn" data-variant="ghost" to="/shop/items">Keep shopping</Link>
            </div>
          </section>
        ) : (
          <section className="shop-success__status" role="alert" data-testid="success-failed">
            <h1 id="shop-success-title" className="shop-success__title">Your purchase didn’t go through</h1>
            <p className="shop-success__sub">
              The purchase couldn’t be completed, so nothing was delivered. Your Credits weren’t spent (any hold is released shortly) — you can try again.
            </p>
            {selfCustody ? <Receipts txs={state.txs} /> : null}
            <div className="shop-success__actions">
              <Link className="shop-success__btn" data-variant="purple" to="/shop/items">Back to shop</Link>
            </div>
          </section>
        )}
      </main>
    );
  }

  const iap = isIapMode();
  return (
    <main className="shop-success" aria-labelledby="shop-success-title">
      <Confetti />
      <section className="shop-success__done" data-testid="success-done">
        <div className="shop-success__banner" role="status">
          <span className="shop-success__banner-check" aria-hidden="true">
            <svg viewBox="0 0 60 60" width="60" height="60">
              <circle cx="30" cy="30" r="30" fill="#34ce77" />
              <path d="M18 31l8 8 16-18" fill="none" stroke="#fff" strokeWidth="4.5" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </span>
          <p className="shop-success__banner-text">
            <b id="shop-success-title">Your purchase was successful!</b> {iap ? "You can find your items in your backpack." : "You can find your item in the My Items tab."}
          </p>
        </div>

        <ul className="shop-success__list">
          {state.creditsAdded ? (
            <li className="shop-success__credits" data-testid="success-credits">
              <span className="shop-success__check" aria-hidden="true">
                <CheckMark />
              </span>
              <CreditMark size={30} />
              <p className="shop-success__credits-text">
                <span className="shop-success__credits-amount">{state.creditsAdded.toLocaleString("en-US")} Credits</span> <span>added</span>
              </p>
            </li>
          ) : null}
          {items.map((item, i) => (
            <li key={`${item.name}-${i}`} className="shop-success__item">
              {i > 0 ? <span className="shop-success__divider" aria-hidden="true" /> : null}
              <div className="shop-success__row">
                <span className="shop-success__thumb" data-thumb aria-hidden="true">
                  {item.thumbnail ? <img src={item.thumbnail} alt="" /> : null}
                  <span className="shop-success__check">
                    <CheckMark />
                  </span>
                </span>
                <div className="shop-success__info" data-info>
                  <div className="shop-success__name" title={item.name}>
                    {item.href ? <Link to={item.href}>{item.name}</Link> : item.name}
                    {item.quantity > 1 ? <span className="shop-success__qty"> × {item.quantity}</span> : null}
                  </div>
                  {item.creator ? (
                    <span className="shop-success__creator">
                      <CreatorBadge address={item.creator} prefix />
                    </span>
                  ) : null}
                </div>
                <div className="shop-success__paid" data-price data-method={item.method}>
                  {item.method === "credits" ? <CreditMark size={22} /> : null}
                  {item.method === "credits" && typeof item.credits === "number" ? (
                    <span className="shop-success__amount" aria-label={`${item.credits} Credits`}>
                      {formatCredits(item.credits)}
                    </span>
                  ) : (
                    <span>{item.method === "credits" ? "Paid with Credits" : "Paid with MANA"}</span>
                  )}
                </div>
              </div>
            </li>
          ))}
        </ul>

        {selfCustody ? <Receipts txs={state.txs} confirmed /> : null}

        <div className="shop-success__ctas">
          {iap ? (
            <>
              <button type="button" className="shop-success__cta" data-variant="ghost" onClick={() => navigate("/shop/overview")}>
                Done
              </button>
              <a className="shop-success__cta" data-variant="ruby" href={backpackDeepLink(items)}>
                Go to backpack
              </a>
            </>
          ) : (
            <>
              <button type="button" className="shop-success__cta" data-variant="ghost" onClick={() => navigate(myItemsRouteFor(items.map((i) => i.category)))}>
                My Items
              </button>
              <a className="shop-success__cta" data-variant="ruby" href={jumpUrl()} target="_blank" rel="noreferrer">
                Try in World
                <JumpInIcon />
              </a>
            </>
          )}
        </div>
      </section>
    </main>
  );
}

function SuccessRoute() {
  const location = useLocation();
  const navState = isSuccessNavState(location.state) ? location.state : null;
  const [stored, setStored] = useState<SuccessNavState | null>();
  useEffect(() => {
    if (!navState) setStored(loadReceipt());
  }, [navState]);
  const state = navState ?? stored;
  if (state === undefined) return null;
  if (!state || state.items.length === 0) return <Navigate to="/shop/items" replace />;
  return <SuccessScreen state={state} />;
}

export default function SuccessPage() {
  return (
    <EnsureQueryClient>
      <SuccessRoute />
    </EnsureQueryClient>
  );
}
