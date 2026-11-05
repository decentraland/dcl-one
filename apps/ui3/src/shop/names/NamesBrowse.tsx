// from: marketplace@f3956826:webapp/src/components/NamesPage/NamesPage.tsx (AssetBrowse, Section.ENS),
// webapp/src/components/ClaimYourName/* ("claim your own NAME" beside the resale list),
// webapp/src/components/AssetFilters/PriceFilter/PriceFilter.tsx (the ENS section's MANA price range)
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
// NAMEs being resold. A resale is a secondary sale: MANA only (SHOP.md), so the cards show MANA and
// the buy happens on the NAME's token page, where a session without a web3 wallet gets the wallet and
// network guidance. The Shop's own registration is one tab away, for a NAME nobody holds yet.

import { useState, type FormEvent } from "react";
import { useSearchParams } from "react-router";

import Button from "../../atoms/Button";
import EmptyState from "../../components/EmptyState";
import type { ShopClient } from "../api/shop";
import { reasonText } from "../payment/copy";
import { indexOutage } from "../land/api";
import { useResource } from "../land/useResource";
import { fetchNameResales, NAME_SORT_LABEL, nameOf, parsePriceBound, nameResaleQueryFromSearch, nameResaleQueryToSearch, NAMES_PAGE_SIZE, type NameResaleQuery } from "./api";
import NameCard from "./NameCard";
import "./names.css";

/**
 * The listing price, in MANA, applied on Enter or when the focus leaves the range. A bound that is not a
 * positive amount, or a minimum above the maximum, is held back with the reason instead of applied.
 */
function PriceRange({ min, max, onApply }: { min: string; max: string; onApply: (range: { minPrice: string; maxPrice: string }) => void }) {
  const [draft, setDraft] = useState({ min, max });
  const [applied, setApplied] = useState({ min, max });
  if (applied.min !== min || applied.max !== max) {
    // The URL moved (back button, "clear"): the boxes follow it.
    setApplied({ min, max });
    setDraft({ min, max });
  }
  const minBound = parsePriceBound(draft.min);
  const maxBound = parsePriceBound(draft.max);
  const minBad = minBound === null;
  const maxBad = maxBound === null;
  const minWei = minBound?.wei ?? null;
  const maxWei = maxBound?.wei ?? null;
  const crossed = minWei !== null && maxWei !== null && minWei > maxWei;
  const problem = minBad || maxBad ? "Enter a MANA amount, such as 10 or 2.5." : crossed ? "The minimum is above the maximum." : null;
  const apply = () => {
    if (problem) return;
    // Zero is no bound: the box empties rather than carrying a bound that filters nothing.
    const next = { minPrice: minWei ? draft.min.trim() : "", maxPrice: maxWei ? draft.max.trim() : "" };
    setDraft({ min: next.minPrice, max: next.maxPrice });
    if (next.minPrice !== min || next.maxPrice !== max) onApply(next);
  };
  return (
    <form
      className="shop-names-browse__price"
      aria-label="Price in MANA"
      onSubmit={(e: FormEvent) => {
        e.preventDefault();
        apply();
      }}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) apply();
      }}
    >
      <input
        type="text"
        inputMode="decimal"
        aria-label="Minimum price in MANA"
        placeholder="Min MANA"
        value={draft.min}
        aria-invalid={minBad || crossed || undefined}
        onChange={(e) => setDraft({ ...draft, min: e.target.value })}
      />
      <span aria-hidden="true">to</span>
      <input
        type="text"
        inputMode="decimal"
        aria-label="Maximum price in MANA"
        placeholder="Max MANA"
        value={draft.max}
        aria-invalid={maxBad || crossed || undefined}
        onChange={(e) => setDraft({ ...draft, max: e.target.value })}
      />
      {/* Enter submits the form; the button stays out of sight and out of the tab order. */}
      <button type="submit" hidden />
      {problem ? (
        <p className="shop-names-browse__price-error" role="alert">
          {problem}
        </p>
      ) : null}
    </form>
  );
}

export default function NamesBrowse({ client }: { client?: ShopClient }) {
  const [params, setParams] = useSearchParams();
  const query = nameResaleQueryFromSearch(`?${params}`);
  const [draft, setDraft] = useState(query.search);
  const results = useResource((signal) => fetchNameResales(query, { client, signal }), [nameResaleQueryToSearch(query), client]);

  const update = (patch: Partial<NameResaleQuery>) => {
    const next = { ...query, ...patch, page: patch.page ?? 1 };
    const q = new URLSearchParams(nameResaleQueryToSearch(next));
    q.set("section", "resale");
    setParams(q);
  };

  const outage = results.status === "error" ? indexOutage(results.error) : null;
  const total = results.data?.total ?? 0;
  const pages = Math.max(1, Math.ceil(total / NAMES_PAGE_SIZE));

  return (
    <section className="shop-names-browse" aria-labelledby="shop-names-browse">
      <h2 id="shop-names-browse">NAMEs being resold</h2>
      <p className="shop-land-page__sub">{reasonText("secondarySale")} Buying one needs a web3 wallet holding MANA on Ethereum.</p>
      <div className="shop-names-browse__toolbar">
        <form
          role="search"
          onSubmit={(e) => {
            e.preventDefault();
            update({ search: draft.trim() });
          }}
        >
          <input type="search" aria-label="Search NAMEs" placeholder="Search NAMEs" value={draft} onChange={(e) => setDraft(e.target.value)} />
        </form>
        <select aria-label="Sort by" value={query.sort} onChange={(e) => update({ sort: e.target.value as NameResaleQuery["sort"] })}>
          {(Object.keys(NAME_SORT_LABEL) as NameResaleQuery["sort"][]).map((s) => (
            <option key={s} value={s}>
              {NAME_SORT_LABEL[s]}
            </option>
          ))}
        </select>
        <label className="shop-land-check">
          <input type="checkbox" checked={query.onlyOnSale} onChange={(e) => update({ onlyOnSale: e.target.checked })} /> Only on sale
        </label>
        <PriceRange min={query.minPrice} max={query.maxPrice} onApply={update} />
      </div>

      {results.status === "loading" && !results.data ? (
        <ul className="shop-names-grid" aria-hidden="true">
          {Array.from({ length: 8 }, (_, i) => (
            <li key={i} className="shop-land-skeleton" />
          ))}
        </ul>
      ) : outage ? (
        <div className="shop-land-note shop-land-note--warn" role="status">
          <p>NAMEs on resale cannot be shown right now: {outage.message} This is a delay in our index, not an empty market.</p>
          <Button size="sm" variant="secondary" onClick={results.reload}>
            Try again
          </Button>
        </div>
      ) : results.status === "error" ? (
        <div className="shop-land-note shop-land-note--error" role="alert">
          <p>The NAMEs on resale could not be loaded.</p>
          <Button size="sm" variant="secondary" onClick={results.reload}>
            Try again
          </Button>
        </div>
      ) : results.data && results.data.results.length === 0 ? (
        <EmptyState
          title={
            query.search
              ? `No NAME matching "${query.search}" is on resale${query.minPrice || query.maxPrice ? " in this price range" : ""}`
              : query.minPrice || query.maxPrice
                ? "No NAMEs on resale in this price range"
                : "No NAMEs are on resale right now"
          }
          subtitle="If nobody holds it yet, you can register it yourself from the Claim tab."
        />
      ) : results.data ? (
        <>
          <p className="shop-lands__count" role="status">
            {total.toLocaleString("en-US")} {total === 1 ? "NAME" : "NAMEs"}
          </p>
          <ul className="shop-names-grid">
            {results.data.results.map((r) => (
              <li key={r.nft.id}>
                <NameCard
                  name={nameOf(r)}
                  priceWei={r.order && r.order.status === "open" ? r.order.price : null}
                  href={`/shop/token/${r.nft.contractAddress.toLowerCase()}/${r.nft.tokenId}`}
                />
              </li>
            ))}
          </ul>
          {pages > 1 ? (
            <nav className="shop-lands__pager" aria-label="Pages">
              <Button size="sm" variant="secondary" disabled={query.page <= 1} onClick={() => update({ page: query.page - 1 })}>
                Previous
              </Button>
              <span className="shop-lands__count">
                Page {query.page} of {pages}
              </span>
              <Button size="sm" variant="secondary" disabled={query.page >= pages} onClick={() => update({ page: query.page + 1 })}>
                Next
              </Button>
            </nav>
          ) : null}
        </>
      ) : null}
    </section>
  );
}
