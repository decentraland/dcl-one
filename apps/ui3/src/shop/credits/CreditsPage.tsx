// from: shop@2d3bf62:app/src/pages/GetCredits.tsx (+GetCredits.styles.ts), app/src/components/Faq/Faq.tsx
//       (+Faq.styles.ts, the buyer FAQ in its on-dark skin), app/src/intl/en.json (getCredits.*, faq.buyers.*);
//       v2 ui3/src/marketplace/pages/MkCreditsHub.tsx, sites marketplace.credits.tsx and marketplace.packs.tsx
//       (supersedes both)
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useId, useRef, useState } from "react";
import { Link, useNavigate, useSearchParams } from "react-router";

import Button from "../../atoms/Button";
import { useShopWallet } from "../wallet/context";
import { SHOP_QUERY_KEYS } from "../checkout/lib/after-purchase";
import { CURRENCY, formatAmount } from "../checkout/lib/currency";
import { EnsureQueryClient, useShopServices } from "../checkout/services";
import { cancelCreditOrder, creditOrderPill, fetchUserCreditOrders, resumeCreditOrder, type CreditOrder } from "./credits";
import { balanceLabel, useBalance, useCreditPacks } from "./hooks";
import { isIapMode } from "./iap";
import { createPackCheckout, packBonus, pollCreditGrant, type CreditPack } from "./payments";
import { clearTopUpHandoffs, takeTopUpReturn } from "./returns";
import CreditRate from "./components/CreditRate";
import HeldCredits from "./components/HeldCredits";
import packStack1 from "./assets/pack-stack1-512.webp?url";
import packStack2 from "./assets/pack-stack2-512.webp?url";
import packStack3 from "./assets/pack-stack3-512.webp?url";
import packChest from "./assets/pack-chest-512.webp?url";
import creditCoin from "./assets/credit-coin.webp?url";
import checkCircle from "./assets/check-circle.svg?url";
import loaderLogo from "./assets/loader-logo.svg?url";
import "./creditspage.css";

const PACK_ART_ORDER = [packStack1, packStack2, packStack3, packChest];
const PACK_ART: Record<string, string> = { pack_5: packStack1, pack_10: packStack2, pack_25: packStack3, pack_50: packChest };

function bundledArtFor(pack: CreditPack, index: number): string {
  return PACK_ART[pack.id] ?? PACK_ART_ORDER[index % PACK_ART_ORDER.length]!;
}

type Phase = "select" | "redirecting" | "processing" | "success" | "pending" | "error";

const FAQ: { q: string; a: string }[] = [
  {
    q: "What are Credits?",
    a: "Credits are Decentraland\u2019s in-app currency for purchasing new Wearables and Emotes in the Shop.\nBuy a Credit pack, then use your balance to collect creator-made items and build a look that\u2019s uniquely yours.",
  },
  {
    q: "What can I buy with Credits?",
    a: "Credits can be used to purchase new Wearables, Emotes and NAMEs in the Decentraland Shop.\nLAND, estates, rentals and secondary-market items are paid in MANA.",
  },
  {
    q: "Where can I buy Credits?",
    a: "You can buy Credit packs right here on this page, up above. Credits are also available to purchase in-world and on mobile.\nFor the best pricing, we recommend purchasing through desktop or web. Mobile pricing may differ due to mobile platform fees.",
  },
  {
    q: "Where can I use my Credits?",
    a: "Credits are tied to your Decentraland account, not the device where you purchased them.\nBuy Credits on desktop or web and use them everywhere, including mobile.",
  },
  { q: "Do Credits expire?", a: "No. Credits do not expire and will remain in your account until you use them." },
  {
    q: "Can Credits be transferred or exchanged for cash?",
    a: "No. Credits are tied to your account and cannot be transferred to another account, withdrawn, or exchanged for cash.",
  },
];

function friendly(e: unknown): string {
  const err = e as { name?: string; message?: string; status?: number } | null;
  if (err?.name === "AbortError") return "You cancelled the request.";
  if (err?.status === 401 || /sign in/i.test(err?.message ?? "")) return "Sign in again to get Credits.";
  return "Something went wrong. You were not charged unless you finished paying; if you did, your Credits arrive on their own.";
}

function StarIcon() {
  return (
    <svg viewBox="0 0 24 24" width="24" height="24" aria-hidden="true" focusable="false" className="shop-credits__star">
      <path
        fill="currentColor"
        d="M12 17.275L7.85 19.775C7.667 19.892 7.475 19.942 7.275 19.925C7.075 19.908 6.9 19.842 6.75 19.725C6.6 19.608 6.483 19.463 6.4 19.288C6.317 19.113 6.3 18.917 6.35 18.7L7.45 13.975L3.775 10.8C3.608 10.65 3.504 10.479 3.463 10.287C3.422 10.095 3.434 9.908 3.5 9.725C3.566 9.542 3.666 9.392 3.8 9.275C3.934 9.158 4.117 9.083 4.35 9.05L9.2 8.625L11.075 4.175C11.158 3.975 11.288 3.825 11.463 3.725C11.638 3.625 11.817 3.575 12 3.575C12.183 3.575 12.362 3.625 12.537 3.725C12.712 3.825 12.842 3.975 12.925 4.175L14.8 8.625L19.65 9.05C19.883 9.083 20.067 9.158 20.2 9.275C20.333 9.392 20.433 9.542 20.5 9.725C20.567 9.908 20.579 10.096 20.538 10.288C20.497 10.48 20.392 10.651 20.225 10.8L16.55 13.975L17.65 18.7C17.7 18.917 17.683 19.113 17.6 19.288C17.517 19.463 17.4 19.609 17.25 19.725C17.1 19.841 16.925 19.908 16.725 19.925C16.525 19.942 16.333 19.892 16.15 19.775L12 17.275Z"
      />
    </svg>
  );
}

function PackGrid({ packs, loading, onSelect }: { packs: CreditPack[]; loading: boolean; onSelect(pack: CreditPack): void }) {
  if (loading) {
    return (
      <div className="shop-credits__grid" aria-busy="true" aria-label={`Loading ${CURRENCY.name} packs\u2026`}>
        {[0, 1, 2, 3].map((i) => (
          <div key={i} className="shop-credits__pack" data-skeleton="true" data-testid="pack-skeleton" aria-hidden="true">
            <span className="shop-credits__pack-top">
              <span className="shop-credits__pack-heading">
                <span className="shop-credits__sk" data-part="amount" />
                <span className="shop-credits__sk" data-part="unit" />
              </span>
              <span className="shop-credits__sk" data-part="art" />
            </span>
            <span className="shop-credits__sk" data-part="price" />
          </div>
        ))}
      </div>
    );
  }
  return (
    <div className="shop-credits__grid">
      {packs.map((pack, i) => {
        const bonus = packBonus(pack, packs);
        return (
          <button
            key={pack.id}
            type="button"
            className="shop-credits__pack"
            data-testid="pack"
            data-best={pack.bestValue ? "true" : undefined}
            onClick={() => onSelect(pack)}
            aria-label={`${formatAmount(pack.credits)} for $${pack.usd.toFixed(2)}${bonus ? `, ${bonus.bonus} more than ${bonus.baseline} at the entry rate` : ""}`}
          >
            {pack.bestValue ? (
              <span className="shop-credits__badge">
                <StarIcon />
                Recommended
              </span>
            ) : null}
            <span className="shop-credits__pack-top">
              <span className="shop-credits__pack-heading">
                <span className="shop-credits__amount-row">
                  <CreditMarkOutline />
                  <span className="shop-credits__amount">{pack.credits.toLocaleString("en-US")}</span>
                </span>
                <span className="shop-credits__unit">{CURRENCY.name}</span>
              </span>
              <span className="shop-credits__art">
                <img
                  src={pack.artUrl ?? bundledArtFor(pack, i)}
                  alt=""
                  loading="lazy"
                  width={507}
                  height={507}
                  onError={(e) => {
                    const img = e.currentTarget;
                    if (img.dataset.artFallback) return;
                    img.dataset.artFallback = "done";
                    img.src = bundledArtFor(pack, i);
                  }}
                />
              </span>
            </span>
            <span className="shop-credits__price">${pack.usd.toFixed(2)}</span>
          </button>
        );
      })}
    </div>
  );
}

function CreditMarkOutline() {
  return (
    <svg className="shop-credits__mark" viewBox="0 0 20 21" fill="none" aria-hidden="true" focusable="false">
      <path d="M13.1318 0.891714L18.5 6.29015V14.0226L13.125 19.5831H6.26758L0.891602 14.0216V6.29113L6.26074 0.891714H13.1318Z" stroke="currentColor" strokeWidth="1.78379" />
      <path
        d="M7.81758 5.13837C8.7606 4.70684 9.80743 4.55428 10.8344 4.69871C11.8613 4.84315 12.8255 5.27853 13.6129 5.95344L12.0562 7.76966C11.6155 7.39196 11.076 7.1483 10.5012 7.06747C9.92652 6.98664 9.34068 7.07201 8.81294 7.31351C8.28519 7.555 7.8376 7.94253 7.52307 8.43029C7.20854 8.91805 7.0402 9.48566 7.03796 10.066C7.03572 10.6464 7.19967 11.2153 7.51042 11.7054C7.82117 12.1956 8.26576 12.5866 8.79162 12.8322C9.31748 13.0777 9.90265 13.1676 10.478 13.0912C11.0533 13.0148 11.5947 12.7753 12.0383 12.4011L13.581 14.2293C12.7884 14.8981 11.8209 15.326 10.7928 15.4625C9.7648 15.599 8.71916 15.4384 7.7795 14.9996C6.83984 14.5608 6.04542 13.8621 5.49014 12.9863C4.93485 12.1104 4.64191 11.0938 4.64592 10.0568C4.64992 9.01972 4.95071 8.00549 5.51274 7.13393C6.07478 6.26236 6.87456 5.5699 7.81758 5.13837Z"
        fill="currentColor"
      />
    </svg>
  );
}

function Faq({ entries }: { entries: { q: string; a: string }[] }) {
  const [open, setOpen] = useState<ReadonlySet<string>>(() => new Set());
  const baseId = useId();
  const toggle = (key: string) =>
    setOpen((cur) => {
      const next = new Set(cur);
      if (!next.delete(key)) next.add(key);
      return next;
    });
  return (
    <section className="shop-credits-faq" data-testid="faq" aria-labelledby={`${baseId}-title`}>
      <h2 id={`${baseId}-title`} className="shop-credits-faq__title" data-testid="faq-title">
        Learn More About {CURRENCY.name}
      </h2>
      <div className="shop-credits-faq__list">
        {entries.map((entry, i) => {
          const isOpen = open.has(entry.q);
          const panelId = `${baseId}-panel-${i}`;
          const headerId = `${baseId}-header-${i}`;
          return (
            <div key={entry.q} className="shop-credits-faq__item" data-open={isOpen} data-testid="faq-item">
              <button
                type="button"
                className="shop-credits-faq__header"
                id={headerId}
                aria-expanded={isOpen}
                aria-controls={isOpen ? panelId : undefined}
                data-open={isOpen}
                data-testid="faq-question"
                onClick={() => toggle(entry.q)}
              >
                <span className="shop-credits-faq__question">{entry.q}</span>
                <span className="shop-credits-faq__chevron" data-open={isOpen} aria-hidden="true">
                  <svg viewBox="0 0 36 36" width="36" height="36" fill="none">
                    <path d="M25.5 21L18 13.5L10.5 21" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" />
                  </svg>
                </span>
              </button>
              {isOpen ? (
                <div className="shop-credits-faq__answer" id={panelId} role="region" aria-labelledby={headerId} data-testid="faq-answer">
                  {entry.a
                    .split("\n")
                    .map((line) => line.trim())
                    .filter(Boolean)
                    .map((line) => (
                      <p key={line}>{line}</p>
                    ))}
                </div>
              ) : null}
            </div>
          );
        })}
      </div>
    </section>
  );
}

function RecentOrders({ address }: { address: string }) {
  const { client, openExternal } = useShopServices();
  const qc = useQueryClient();
  const [notice, setNotice] = useState<string | null>(null);
  const [resuming, setResuming] = useState<string | null>(null);
  const { data } = useQuery({
    queryKey: [SHOP_QUERY_KEYS.creditOrders, address, "recent"],
    queryFn: () => fetchUserCreditOrders(address, { first: 5 }, client),
    staleTime: 30_000,
  });
  const orders = (data?.items ?? []).filter((o) => o.status !== "abandoned");
  if (orders.length === 0) return null;

  async function resume(order: CreditOrder) {
    setResuming(order.id);
    setNotice(null);
    const result = await resumeCreditOrder(order.id, client);
    setResuming(null);
    if (result.kind === "url" && /^https:\/\/([a-z0-9-]+\.)*stripe\.com\//i.test(result.url)) {
      openExternal(result.url);
      return;
    }
    if (result.kind === "paid") setNotice("That order is already paid: its Credits are on the way.");
    else if (result.kind === "expired") setNotice("That checkout has expired. Choose a pack to start a new one.");
    else setNotice("We couldn't reopen that checkout right now. It may still be payable; try again in a moment.");
    void qc.invalidateQueries({ queryKey: [SHOP_QUERY_KEYS.creditOrders] });
  }

  return (
    <section className="shop-credits__orders" aria-labelledby="shop-credits-orders">
      <h2 id="shop-credits-orders">Recent Credit purchases</h2>
      {notice ? <p role="status">{notice}</p> : null}
      <ul>
        {orders.map((o) => {
          const pill = creditOrderPill(o.status);
          return (
            <li key={o.id} data-testid="credit-order">
              <span>{new Date(o.createdAt).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" })}</span>
              <span>
                {formatAmount(o.credits)} · ${(o.usdCents / 100).toFixed(2)}
              </span>
              <span className="shop-credits__pill" data-status={pill}>
                {pill === "SETTLED" ? "Completed" : pill === "FAILED" ? "Failed" : pill === "UNFINISHED" ? "Not paid yet" : "Processing"}
              </span>
              {o.status === "initiated" ? (
                <Button size="sm" variant="secondary" onClick={() => void resume(o)} disabled={resuming === o.id} data-testid="resume-order">
                  {resuming === o.id ? "Opening…" : "Continue"}
                </Button>
              ) : null}
            </li>
          );
        })}
      </ul>
      <Link to="/shop/activity" className="shop-credits__link">
        All activity
      </Link>
    </section>
  );
}

function CreditsScreen() {
  const [params, setParams] = useSearchParams();
  const navigate = useNavigate();
  const services = useShopServices();
  const wallet = useShopWallet();
  const account = wallet.account;
  const qc = useQueryClient();
  const { packs, isLoading } = useCreditPacks();
  const balance = useBalance(account);
  const [phase, setPhase] = useState<Phase>("select");
  const [granted, setGranted] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [canceledNote, setCanceledNote] = useState(false);
  const abortRef = useRef<AbortController | null>(null);
  const handled = useRef(false);
  const faqRef = useRef<HTMLDivElement>(null);
  const grantedArt = (granted != null ? packs.find((p) => p.credits === granted)?.artUrl : undefined) ?? creditCoin;

  useEffect(() => () => abortRef.current?.abort(), []);

  const clearReturnParams = useCallback(() => {
    setParams(
      (prev) => {
        const next = new URLSearchParams(prev);
        next.delete("order");
        next.delete("canceled");
        return next;
      },
      { replace: true },
    );
  }, [setParams]);

  const pollForGrant = useCallback(
    async (orderId: string) => {
      setPhase("processing");
      const ac = new AbortController();
      abortRef.current = ac;
      try {
        const result = await pollCreditGrant(orderId, { signal: ac.signal, packs }, services.client);
        void qc.invalidateQueries({ queryKey: [SHOP_QUERY_KEYS.balance] });
        void qc.invalidateQueries({ queryKey: [SHOP_QUERY_KEYS.creditOrders] });
        if (result.status === "credited") {
          const credits = result.creditsGranted ?? 0;
          setGranted(credits > 0 ? credits : null);
          const back = takeTopUpReturn();
          if (back) {
            navigate(back.path, { state: { resumeCheckout: true, ...(credits > 0 ? { creditsAdded: credits } : {}) } });
            return;
          }
          setPhase("success");
        } else if (result.status === "abandoned" || result.status === "initiated") {
          clearTopUpHandoffs();
          setCanceledNote(true);
          setPhase("select");
        } else if (result.status === "failed") {
          clearTopUpHandoffs();
          setError("We couldn't add your Credits. If you were charged, the payment is refunded.");
          setPhase("error");
        } else {
          clearTopUpHandoffs();
          setPhase("pending");
        }
      } catch (e) {
        clearTopUpHandoffs();
        setError(friendly(e));
        setPhase("error");
      }
    },
    [packs, services.client, qc, navigate],
  );

  useEffect(() => {
    if (handled.current || wallet.resolving) return;
    const orderId = params.get("order");
    const canceled = params.get("canceled") != null;
    if (canceled) {
      handled.current = true;
      if (orderId && account) void cancelCreditOrder(orderId, services.client);
      clearTopUpHandoffs();
      clearReturnParams();
      setCanceledNote(true);
      setPhase("select");
      return;
    }
    if (!orderId) return;
    handled.current = true;
    clearReturnParams();
    if (!account) {
      setError(`Your payment went through, but you're signed out. Sign in again with the same account to see your ${CURRENCY.name}.`);
      setPhase("error");
      return;
    }
    void pollForGrant(orderId);
  }, [params, account, wallet.resolving, services.client, clearReturnParams, pollForGrant]);

  async function startCheckout(pack: CreditPack) {
    if (!account) {
      wallet.host?.onSignIn?.();
      return;
    }
    setError(null);
    setCanceledNote(false);
    setPhase("redirecting");
    try {
      const session = await createPackCheckout(pack.id, services.client);
      if (session.url) {
        services.openExternal(session.url);
        return;
      }
      void pollForGrant(session.orderId);
    } catch (e) {
      setError(friendly(e));
      setPhase("error");
    }
  }

  function reset() {
    abortRef.current?.abort();
    setPhase("select");
    setGranted(null);
    setError(null);
    setCanceledNote(false);
  }

  function scrollToFaq() {
    const el = faqRef.current;
    if (!el) return;
    const reduced = typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    el.scrollIntoView?.({ behavior: reduced ? "auto" : "smooth", block: "center" });
    el.focus({ preventScroll: true });
  }

  if (isIapMode()) {
    return (
      <main className="shop-credits" aria-labelledby="shop-credits-title">
        <section className="shop-credits__hero">
          <header className="shop-credits__head">
            <h1 id="shop-credits-title" className="shop-credits__title">
              {CURRENCY.name}
            </h1>
            <p className="shop-credits__sub">Get Credits in the Decentraland app. Your balance works everywhere in the Shop.</p>
          </header>
        </section>
      </main>
    );
  }

  return (
    <main className="shop-credits" aria-labelledby="shop-credits-title">
      {phase === "select" || phase === "redirecting" ? (
        <>
          <section className="shop-credits__hero" data-testid="credits-hero">
            <div className="shop-credits__panel">
              <div className="shop-credits__inner" data-hidden={phase === "redirecting" || undefined}>
                <header className="shop-credits__head">
                  <h1 id="shop-credits-title" className="shop-credits__title">
                    Stock up on {CURRENCY.name}
                  </h1>
                  <div className="shop-credits__subrow">
                    <p className="shop-credits__sub">
                      Choose a Credit pack, then shop creator-made Wearables and Emotes to build a look that’s uniquely yours.
                    </p>
                    <button type="button" className="shop-credits__learn" onClick={scrollToFaq} data-testid="credits-learn-more">
                      Learn More
                    </button>
                  </div>
                </header>

                {account ? (
                  <section className="shop-credits__balance" aria-label={`Your ${CURRENCY.name}`}>
                    <span>Your balance</span>
                    <b data-testid="credits-balance">
                      {balanceLabel(balance.data, balance.isError)} {CURRENCY.name}
                    </b>
                    <HeldCredits held={balance.data?.held} />
                    <CreditRate />
                  </section>
                ) : null}

                {canceledNote ? (
                  <p className="shop-credits__note" role="status">
                    Payment canceled — no charge was made. Pick a pack whenever you’re ready.
                  </p>
                ) : null}

                <PackGrid packs={packs} loading={isLoading} onSelect={(p) => void startCheckout(p)} />
              </div>

              {phase === "redirecting" ? (
                <div className="shop-credits__redirect" role="status" aria-live="polite">
                  <img className="shop-credits__logo" data-size="lg" src={loaderLogo} alt="" width={72} height={72} />
                  <p>Redirecting to secure checkout…</p>
                </div>
              ) : null}
            </div>
          </section>

          {account ? <RecentOrders address={account} /> : null}

          <div className="shop-credits__faq" ref={faqRef} tabIndex={-1} data-testid="credits-faq">
            <Faq entries={FAQ} />
          </div>
        </>
      ) : null}

      {phase === "processing" ? (
        <section className="shop-credits__processing" role="status" aria-live="polite" data-testid="credits-processing">
          <img className="shop-credits__logo" src={loaderLogo} alt="" width={61} height={61} />
          <div className="shop-credits__processing-body">
            <p className="shop-credits__processing-title">
              <strong>Completing purchase</strong>…
            </p>
            <span className="shop-credits__progress" aria-hidden="true">
              <span />
            </span>
          </div>
        </section>
      ) : null}

      {phase === "success" ? (
        <section className="shop-credits__outcome" data-outcome="success">
          <div className="shop-credits__success" role="status" aria-live="polite" data-testid="credits-success">
            <div className="shop-credits__banner">
              <img src={checkCircle} alt="" width={60} height={60} />
              <p>
                <strong>Your purchase was successful!</strong> You can find your {CURRENCY.name} in your balance.
              </p>
            </div>
            {granted != null ? (
              <div className="shop-credits__granted">
                <div className="shop-credits__granted-row">
                  <img className="shop-credits__coin" src={grantedArt} alt="" width={93} height={93} />
                  <p className="shop-credits__granted-text">
                    <CreditMarkOutline />
                    <span>
                      <strong>
                        {granted.toLocaleString("en-US")} {CURRENCY.name}
                      </strong>{" "}
                      <span>Added to your account</span>
                    </span>
                  </p>
                </div>
              </div>
            ) : null}
            <div className="shop-credits__actions">
              <button type="button" className="shop-credits__action" data-variant="outline" onClick={reset}>
                Buy more {CURRENCY.name}
              </button>
              <button type="button" className="shop-credits__action" onClick={() => navigate("/shop/items")}>
                Start shopping
              </button>
            </div>
          </div>
        </section>
      ) : null}

      {phase === "pending" ? (
        <section className="shop-credits__outcome">
          <div className="shop-credits__pending" role="status" aria-live="polite" data-testid="credits-pending">
            <div className="shop-credits__pending-head">
              <img src={checkCircle} alt="" width={32} height={32} />
              <p>Your {CURRENCY.name} are on the way!</p>
            </div>
            <p className="shop-credits__pending-body">
              Your payment went through. It’s taking a little longer than usual to confirm — your balance will update automatically as soon as it lands, no
              need to pay again.
            </p>
            <div className="shop-credits__actions" data-pad="true">
              <button type="button" className="shop-credits__action" data-variant="outline" onClick={() => navigate("/shop/items")}>
                Go shopping
              </button>
              <button type="button" className="shop-credits__action" data-variant="ruby" onClick={reset}>
                Got it
              </button>
            </div>
          </div>
        </section>
      ) : null}

      {phase === "error" ? (
        <section className="shop-credits__status" role="alert" data-testid="credits-error">
          <p className="shop-credits__status-title">Something went wrong</p>
          <p className="shop-credits__error">{error}</p>
          <div className="shop-credits__status-actions">
            <button type="button" className="shop-credits__action" onClick={reset}>
              Try again
            </button>
          </div>
        </section>
      ) : null}
    </main>
  );
}

export default function CreditsPage() {
  return (
    <EnsureQueryClient>
      <CreditsScreen />
    </EnsureQueryClient>
  );
}
