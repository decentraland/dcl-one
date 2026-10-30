// from: marketplace@f3956826:webapp/src/components/NamesPage/NamesPage.tsx (AssetBrowse, Section.ENS),
// webapp/src/components/ClaimYourName/* ("claim your own NAME" beside the resale list)
// Copyright 2020 Metaverse Holdings Ltd. Licensed under the Apache License, Version 2.0.
// NAMEs being resold. A resale is a secondary sale: MANA only (SHOP.md), so the cards show MANA and
// the buy happens on the NAME's token page, where a session without a web3 wallet gets the wallet and
// network guidance. The Shop's own registration is one tab away, for a NAME nobody holds yet.

import { useState } from "react";
import { useSearchParams } from "react-router";

import Button from "../../atoms/Button";
import EmptyState from "../../components/EmptyState";
import type { ShopClient } from "../api/shop";
import { reasonText } from "../payment/copy";
import { indexOutage } from "../land/api";
import { useResource } from "../land/useResource";
import { fetchNameResales, NAME_SORT_LABEL, nameOf, nameResaleQueryFromSearch, nameResaleQueryToSearch, NAMES_PAGE_SIZE, type NameResaleQuery } from "./api";
import NameCard from "./NameCard";
import "./names.css";

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
          title={query.search ? `No NAME matching "${query.search}" is on resale` : "No NAMEs are on resale right now"}
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
