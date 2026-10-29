// from: shop@94584a5c:app/src/components/CreatorSales/CreatorSales.tsx, app/src/components/CreatorSaleModal/CreatorSaleModal.tsx,
//       app/src/lib/coupons.ts, app/src/lib/saleableCollections.ts
// A creator's discounts ("creator sales"): the running and scheduled ones with an End button, and the
// form that starts one on a collection. A discount lowers the Shop's credit price of the creator's own
// PRIMARY listings while it runs (the server applies coupons to collection items only); a classic MANA
// listing is left alone and the review says so. A node without its coupons index explains the outage.

import { useQueryClient } from "@tanstack/react-query";
import { useMemo, useState } from "react";

import Button from "../../../atoms/Button";
import EmptyState from "../../../components/EmptyState";
import { CHAIN, type ChainId } from "../../model/chain";
import { CENTS_PER_CREDIT, COUPON_PPM, creditsCeil, discountedCents, usdCents } from "../../model/money";
import { dataUnavailable } from "../../market/lib/api";
import { formatDate, LoadFailure, Loading, useShopProvider, useWalletBlock, useWrite, WriteStatus } from "../../market/components/kit";
import { useCreatorSales } from "../hooks";
import {
  createCollectionSale,
  endSale,
  isSaleCapped,
  liveSaleStatus,
  MAX_SALE_DURATION_MS,
  MAX_SALE_PCT,
  MIN_SALE_PCT,
  postCoupon,
  type CreatorSale,
  type SaleInputProblem,
} from "../lib/coupons";
import type { SaleableCollection } from "../lib/saleableCollections";

const DAY = 86_400_000;
const PRESETS = [10, 20, 30, 50];
const DURATIONS = [1, 3, 7, 14, 30];

const PROBLEM_TEXT: Record<SaleInputProblem, string> = {
  collections: "Pick a collection.",
  pct: `A discount is between ${MIN_SALE_PCT}% and ${MAX_SALE_PCT}%.`,
  window: "The discount has to end after it starts, and in the future.",
  duration: "A discount runs for at most 30 days.",
  uses: "The unit limit is a whole number, at least 1.",
};

/** The sale price of `credits` under `pct` off, rounded up to whole credits as a charge is. */
export function salePrice(credits: number, pct: number): number {
  const cents = discountedCents(usdCents(credits * CENTS_PER_CREDIT), (pct * COUPON_PPM) / 100);
  return cents === null ? credits : creditsCeil(cents);
}

export function CreatorSaleForm({ collection, chainId = CHAIN.POLYGON, onClose, now = Date.now }: { collection: SaleableCollection; chainId?: ChainId; onClose: () => void; now?: () => number }) {
  const qc = useQueryClient();
  const provider = useShopProvider();
  const block = useWalletBlock("Starting a discount");
  const [pct, setPct] = useState(20);
  const [days, setDays] = useState(7);
  const [cap, setCap] = useState("");
  const [reviewing, setReviewing] = useState(false);
  const write = useWrite();
  const example = collection.examplePriceCredits;
  const discounted = collection.items.filter((i) => i.state === "discounted").length;
  const classic = collection.items.filter((i) => i.state === "classic").length;
  const unlisted = collection.items.filter((i) => i.state === "unlisted").length;
  const problem = useMemo(() => {
    if (!Number.isInteger(pct) || pct < MIN_SALE_PCT || pct > MAX_SALE_PCT) return "pct" as const;
    if (days * DAY > MAX_SALE_DURATION_MS) return "duration" as const;
    if (cap && (!/^\d+$/.test(cap) || Number(cap) < 1)) return "uses" as const;
    return null;
  }, [pct, days, cap]);

  if (block || !provider) return block;
  if (write.state.kind === "done")
    return (
      <div className="mk-section" data-testid="shop-sale-started">
        <p role="status">
          {pct}% off “{collection.name}” until {formatDate(now() + days * DAY)}.
        </p>
        <Button size="sm" variant="ghost" onClick={onClose}>
          Close
        </Button>
      </div>
    );

  const submit = () =>
    void write.run("Sign the discount in your wallet…", "Discount started.", async (progress) => {
      const payload = await createCollectionSale({ provider, chainId, collections: [collection.contractAddress], discountPct: pct, endsAtMs: now() + days * DAY, uses: cap ? Number(cap) : undefined });
      progress("Publishing the discount…");
      try {
        const sale = await postCoupon(payload);
        await qc.invalidateQueries({ queryKey: ["creator-sales"] });
        return sale.id;
      } catch (e) {
        const outage = dataUnavailable(e);
        if (outage) throw new Error("Discounts are not available on this node yet: its coupons index is still being built. Nothing was published.");
        throw e;
      }
    });

  return (
    <div className="mk-panel mk-form" role="group" aria-label={`Discount ${collection.name}`}>
      <h2 className="mk-panel__title">{reviewing ? "Review your discount" : "Discount this collection"}</h2>
      {!reviewing ? (
        <>
          <p className="mk-hint">Your listed items show the lower price until it ends, then go back to their regular price by themselves.</p>
          <p>
            <strong>{collection.name}</strong> · {collection.listedCount} {collection.listedCount === 1 ? "item" : "items"} listed
          </p>
          <fieldset className="mk-field">
            <legend className="mk-field__label">Discount</legend>
            <div className="mk-toolbar__filters">
              {PRESETS.map((p) => (
                <button key={p} type="button" className="mk-chip" aria-pressed={pct === p} onClick={() => setPct(p)}>
                  {p}%
                </button>
              ))}
              <label className="mk-inline-check">
                Custom
                <input type="number" min={MIN_SALE_PCT} max={MAX_SALE_PCT} value={pct} onChange={(e) => setPct(Number(e.target.value))} aria-label="Discount percentage" />
              </label>
            </div>
          </fieldset>
          <label className="mk-field">
            <span className="mk-field__label">Runs for</span>
            <select value={days} onChange={(e) => setDays(Number(e.target.value))}>
              {DURATIONS.map((d) => (
                <option key={d} value={d}>
                  {d === 1 ? "1 day" : `${d} days`}
                </option>
              ))}
            </select>
          </label>
          <label className="mk-field">
            <span className="mk-field__label">Units at the discounted price (optional)</span>
            <input inputMode="numeric" value={cap} onChange={(e) => setCap(e.target.value)} placeholder="No limit" />
          </label>
          {example !== null && !problem ? (
            <p className="mk-hint" data-testid="shop-sale-preview">
              {salePrice(example, pct) < example
                ? `Example: an item priced ${example} credits sells for ${salePrice(example, pct)} credits while the discount runs.`
                : `An item priced ${example} credits would still sell for the same price: prices are whole Credits, so pick a bigger discount.`}
            </p>
          ) : null}
          {problem ? <p className="mk-field__error">{PROBLEM_TEXT[problem]}</p> : null}
          <div className="mk-actions">
            <Button disabled={!!problem} onClick={() => setReviewing(true)}>
              Review discount
            </Button>
            <Button variant="ghost" onClick={onClose}>
              Cancel
            </Button>
          </div>
        </>
      ) : (
        <>
          <ul className="mk-steps">
            <li data-state="done">
              {pct}% off, starts now, ends {formatDate(now() + days * DAY)}
              {cap ? `, for ${cap} units` : ""}
            </li>
            <li data-state="done">
              {discounted} {discounted === 1 ? "item gets" : "items get"} the discount
            </li>
            {classic > 0 ? (
              <li data-state="todo">
                {classic} {classic === 1 ? "item is" : "items are"} listed in MANA only and {classic === 1 ? "is" : "are"} not discounted; move {classic === 1 ? "it" : "them"} into the Shop first.
              </li>
            ) : null}
            {unlisted > 0 ? (
              <li data-state="todo">
                {unlisted} {unlisted === 1 ? "item is" : "items are"} not listed and {unlisted === 1 ? "stays" : "stay"} as {unlisted === 1 ? "it is" : "they are"}
              </li>
            ) : null}
          </ul>
          <div className="mk-actions">
            <Button disabled={write.busy || discounted === 0} onClick={submit}>
              {write.busy ? "Starting…" : "Start discount"}
            </Button>
            <Button variant="ghost" disabled={write.busy} onClick={() => setReviewing(false)}>
              Back
            </Button>
          </div>
        </>
      )}
      <WriteStatus state={write.state} />
    </div>
  );
}

function SaleRow({ sale, names }: { sale: CreatorSale; names: Record<string, string> }) {
  const qc = useQueryClient();
  const provider = useShopProvider();
  const write = useWrite();
  const status = liveSaleStatus(sale);
  const live = status === "active" || status === "scheduled";
  return (
    <li className="mk-row" data-testid="shop-creator-sale">
      <div className="mk-row__main">
        <span>
          {sale.discount / 10_000}% off · {sale.collections.map((c) => names[c.toLowerCase()] ?? c).join(", ")}
        </span>
        <span className="mk-row__sub">
          {status === "scheduled" ? `Starts ${formatDate(sale.checks.effective)}` : status === "active" ? `Ends ${formatDate(sale.checks.expiration)}` : status}
          {isSaleCapped(sale) ? ` · ${sale.state?.uses ?? 0} of ${sale.checks.uses} used` : sale.state ? ` · ${sale.state.uses} sold` : ""}
        </span>
      </div>
      <span className="mk-badge">{status}</span>
      {live ? (
        <Button
          size="sm"
          variant="secondary"
          disabled={!provider || write.busy || write.state.kind === "done"}
          onClick={() =>
            void write.run("Confirm in your wallet…", "Ended.", async () => {
              const hash = await endSale({ provider: provider!, sale });
              await qc.invalidateQueries({ queryKey: ["creator-sales"] });
              return hash;
            })
          }
        >
          End now
        </Button>
      ) : (
        <span />
      )}
      <WriteStatus state={write.state} />
    </li>
  );
}

export default function CreatorSales({ address, saleable, names }: { address: string; saleable: SaleableCollection[]; names: Record<string, string> }) {
  const sales = useCreatorSales(address);
  const [openFor, setOpenFor] = useState<string | null>(null);
  const collection = saleable.find((c) => c.contractAddress === openFor);
  return (
    <section className="mk-panel" aria-label="Discounts">
      <h2 className="mk-panel__title">Discounts</h2>
      {collection ? <CreatorSaleForm collection={collection} onClose={() => setOpenFor(null)} /> : null}
      {sales.isPending ? (
        <Loading label="Loading your discounts…" />
      ) : sales.isError ? (
        <LoadFailure error={sales.error} what="Discounts" onRetry={() => void sales.refetch()} />
      ) : sales.data.length === 0 ? (
        <EmptyState variant="inline" title="No discounts yet" subtitle="Put a collection on sale for a few days to bring buyers back." data-testid="shop-empty" />
      ) : (
        <ul className="mk-list">
          {sales.data.map((s) => (
            <SaleRow key={s.id} sale={s} names={names} />
          ))}
        </ul>
      )}
      {saleable.length > 0 && !collection ? (
        <div className="mk-actions">
          {saleable.map((c) => (
            <Button key={c.contractAddress} size="sm" variant="secondary" onClick={() => setOpenFor(c.contractAddress)}>
              Discount {c.name}
            </Button>
          ))}
        </div>
      ) : null}
    </section>
  );
}

