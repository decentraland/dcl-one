// from: marketplace@3bf3662f:webapp/src/components/AssetPage/{TransactionHistory/{TransactionHistory,utils}.tsx,
// OwnersTable/{OwnersTable,utils}.tsx} (an asset's latest sales, five a page, and an item's tokens by issue number;
// both hidden while there is nothing to list), in the Shop's resale-panel styling.
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.

import { useState } from "react";
import { Link } from "react-router";

import { shortAddress } from "../../model/address";
import { ManaAmount } from "../../discovery/components/PriceTag";
import { t } from "../../discovery/intl";
import { tokenRoute } from "../../discovery/lib/routes";
import { useShopQuery } from "../../discovery/query";
import { useProfiles, type CreatorProfile } from "../../discovery/hooks";
import { dateText, epochMs, relativeText } from "../../land/format";
import { fetchOwners, fetchSales } from "../../market/lib/api";
import { shopPath } from "../../routes";
import "./resaleoffers.css";
import styles from "./TradeHistory.module.css";

export const HISTORY_PAGE = 5;

const WEEK_MS = 7 * 24 * 60 * 60 * 1000;

/** The marketplace's "When": relative within a week, the date after. */
export function saleWhen(timestamp: number, now: number = Date.now()): string {
  const ms = epochMs(timestamp);
  return now - ms > WEEK_MS ? dateText(ms) : relativeText(ms, now);
}

/** The MANA a sale settled in: Ethereum's (mainnet, Sepolia) or Polygon's. */
function saleNetwork(chainId: number): "polygon" | "ethereum" {
  return chainId === 1 || chainId === 11_155_111 ? "ethereum" : "polygon";
}

function saleType(type: string): string {
  if (type === "order") return t("history.typeListing");
  if (type === "bid") return t("history.typeBid");
  if (type === "mint") return t("history.typeMint");
  return type;
}

function Account({ address, profiles }: { address: string; profiles: Record<string, CreatorProfile> }) {
  return (
    <Link className={styles.account} to={shopPath("account", { address })} title={address}>
      {profiles[address.toLowerCase()]?.name || shortAddress(address)}
    </Link>
  );
}

/** A later page that failed: says so, with a retry, instead of an empty table that reads as "no more". */
function PageError({ onRetry }: { onRetry: () => void }) {
  return (
    <p className="shop-resales__empty" role="alert" data-testid="history-error">
      {t("history.loadError")}{" "}
      <button type="button" className={styles.retry} onClick={onRetry}>
        {t("loadMore.retry")}
      </button>
    </p>
  );
}

function Pager({ page, pages, label, onPage }: { page: number; pages: number; label: string; onPage: (page: number) => void }) {
  if (pages <= 1) return null;
  return (
    <nav className="shop-resales__pager" aria-label={label}>
      <button type="button" onClick={() => onPage(page - 1)} disabled={page === 0}>
        {t("overview.previous")}
      </button>
      <span>
        {page + 1} / {pages}
      </span>
      <button type="button" onClick={() => onPage(page + 1)} disabled={page >= pages - 1}>
        {t("overview.next")}
      </button>
    </nav>
  );
}

/** Latest sales of one token (`tokenId`) or of every token of an item (`itemId`). */
export function LatestSales({ contractAddress, tokenId, itemId }: { contractAddress: string; tokenId?: string; itemId?: string }) {
  const [page, setPage] = useState(0);
  const target = tokenId ? { tokenId } : itemId ? { itemId } : null;
  const q = useShopQuery(
    target ? ["asset-sales", contractAddress.toLowerCase(), tokenId ?? "", itemId ?? "", page] : null,
    (signal) => fetchSales({ contractAddress: contractAddress.toLowerCase(), ...target, first: HISTORY_PAGE, skip: page * HISTORY_PAGE }, undefined, signal),
    { staleMs: 60_000, keepPrevious: true },
  );
  const rows = q.data?.items ?? [];
  const profiles = useProfiles(rows.flatMap((s) => [s.seller, s.buyer]));
  // Hidden only while the first page is empty: a later page that fails or comes back empty keeps the pager.
  if (rows.length === 0 && page === 0) return null;
  const pages = Math.max(page + 1, Math.ceil((q.data?.total ?? 0) / HISTORY_PAGE));
  return (
    <section className="shop-resales" data-testid="latest-sales" aria-labelledby="shop-sales-title">
      <header className="shop-resales__head">
        <h2 id="shop-sales-title">{t("history.latestSales")}</h2>
      </header>
      <div className={styles.scroll}>
        <table className={styles.table}>
          <thead>
            <tr>
              <th scope="col">{t("history.from")}</th>
              <th scope="col">{t("history.to")}</th>
              <th scope="col">{t("history.type")}</th>
              <th scope="col">{t("history.when")}</th>
              <th scope="col">{t("history.price")}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((s) => (
              <tr key={s.id} data-testid="sale-row">
                <td>
                  <Account address={s.seller} profiles={profiles} />
                </td>
                <td>
                  <Account address={s.buyer} profiles={profiles} />
                </td>
                <td>{saleType(s.type)}</td>
                <td>{saleWhen(s.createdAt)}</td>
                <td>
                  <ManaAmount wei={s.manaWei} network={saleNetwork(s.chainId)} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {q.error && rows.length === 0 ? <PageError onRetry={q.refetch} /> : null}
      <Pager page={page} pages={pages} label={t("history.latestSales")} onPage={setPage} />
    </section>
  );
}

/** Who holds each token of an item, by issue number. `total` counts tokens, not distinct holders. */
export function ItemOwners({ contractAddress, itemId }: { contractAddress: string; itemId: string }) {
  const [page, setPage] = useState(0);
  const q = useShopQuery(
    ["item-owners", contractAddress.toLowerCase(), itemId, page],
    (signal) => fetchOwners({ contractAddress, itemId, first: HISTORY_PAGE, skip: page * HISTORY_PAGE, sortBy: "issuedId", orderDirection: "asc" }, undefined, signal),
    { staleMs: 60_000, keepPrevious: true },
  );
  const rows = q.data?.data ?? [];
  const profiles = useProfiles(rows.map((o) => o.ownerId));
  if (rows.length === 0 && page === 0) return null;
  const total = q.data?.total ?? rows.length;
  const pages = Math.max(page + 1, Math.ceil(total / HISTORY_PAGE));
  return (
    <section className="shop-resales" data-testid="item-owners" aria-labelledby="shop-owners-title">
      <header className="shop-resales__head">
        <h2 id="shop-owners-title">{t("history.owners")}</h2>
        <span>{t("history.copyCount", { count: total })}</span>
      </header>
      <ul className="shop-resales__list">
        {rows.map((o) => (
          <li key={o.tokenId} className="shop-resales__row" data-testid="owner-row">
            <span className={`shop-resales__who ${styles.who}`}>
              <strong>{t("itemDetail.resaleIssued", { issued: o.issuedId })}</strong>
              <Account address={o.ownerId} profiles={profiles} />
            </span>
            <span />
            <Link className="shop-resales__view" to={tokenRoute(contractAddress, o.tokenId)} aria-label={`${t("itemDetail.viewResale")} #${o.issuedId}`}>
              {t("itemDetail.viewResale")}
            </Link>
          </li>
        ))}
      </ul>
      {q.error && rows.length === 0 ? <PageError onRetry={q.refetch} /> : null}
      <Pager page={page} pages={pages} label={t("history.owners")} onPage={setPage} />
    </section>
  );
}
