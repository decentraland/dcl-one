// from: shop@2d3bf62:app/src/pages/MyStore.tsx, app/src/hooks/useStoreStats.ts; marketplace@3bf3662f:webapp/src/components/AccountPage
//       (the store, on-sale and sales sections); catalyrst-v2 ui3/src/marketplace/components/MkMySalesHistory.tsx,
//       ui3/src/marketplace/pages/{MkAccountCollectionsSection,MkOnSaleOnRentAccountSections}.tsx
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.
import { useQuery } from "@tanstack/react-query";
import { Fragment, useEffect, useMemo, useState } from "react";
import { Link, Navigate, useNavigate, useSearchParams } from "react-router";

import Button from "../../../atoms/Button";
import EmptyState from "../../../components/EmptyState";
import { formatMana, parseManaWei, type ManaWei } from "../../model/money";
import { shopPath } from "../../routes";
import { useShopWallet } from "../../wallet/context";
import { EnsureQueryClient, formatDate, LoadFailure, Loading, ManaAmount, shortAddr } from "../../market/components/kit";
import { fetchOpenOrderById } from "../../market/lib/api";
import CreatorSales from "../components/CreatorSales";
import ImportListings from "../components/ImportListings";
import { IssueForm } from "../components/IssueModal";
import { PrimaryListForm } from "../components/PrimaryListModal";
import { useStore, useStoreStats, type StorePeriod } from "../hooks";
import { ShopSignInPanel } from "./SellerSignIn";
import StoreDashboard from "./StoreDashboard";
import type { PublishableItem } from "../lib/builder";
import { myStoreAccess, useShopCapabilities } from "../lib/featureFlags";
import { fetchSalesPage } from "../lib/sales";
import type { Delta } from "../lib/storeMetrics";
import type { StoreCollection, StoreItem, StoreStats } from "../lib/storeStats";

const PERIODS: { key: StorePeriod; label: string }[] = [
  { key: "7d", label: "7 days" },
  { key: "30d", label: "30 days" },
  { key: "all", label: "All time" },
];
const SALES_PER_PAGE = 5;

function mana(wei: bigint) {
  return <ManaAmount wei={wei as ManaWei} />;
}

function DeltaTag({ delta }: { delta: Delta | null }) {
  if (!delta) return null;
  if (delta.pct === null) return delta.current > 0 ? <span className="mk-badge" data-testid="store-delta">new</span> : null;
  const rounded = Math.round(delta.pct);
  return (
    <span className="mk-badge" data-testid="store-delta">
      {rounded === 0 ? "flat" : `${rounded > 0 ? "+" : ""}${rounded}%`}
    </span>
  );
}

export function StoreTiles({ stats, trend }: { stats: StoreStats; trend: ReturnType<typeof useStoreStats>["trend"] }) {
  return (
    <div className="mk-stats" aria-label="Store summary">
      <div className="mk-stat" data-testid="store-tile-sold">
        <span className="mk-stat__label">Sold</span>
        <span className="mk-stat__value">
          {stats.sold.toLocaleString("en-US")} <DeltaTag delta={trend.sold} />
        </span>
        <span className="mk-stat__foot">{stats.sold === 0 ? "No sales in this period" : `${stats.mints} from your store · ${stats.resales} resales`}</span>
      </div>
      <div className="mk-stat" data-testid="store-tile-earnings">
        <span className="mk-stat__label">Earnings</span>
        <span className="mk-stat__value">
          {mana(stats.earningsWei)} <DeltaTag delta={trend.earnings} />
        </span>
        <span className="mk-stat__foot">{stats.partial ? "Estimate: the period has more sales than were read" : "Paid to you in MANA"}</span>
      </div>
      <div className="mk-stat" data-testid="store-tile-listed">
        <span className="mk-stat__label">Listed in the Shop</span>
        <span className="mk-stat__value">{stats.listed}</span>
        <span className="mk-stat__foot">
          {stats.classic} in MANA only · {stats.neverListed} not listed · {stats.soldOut} sold out
        </span>
      </div>
      <div className="mk-stat" data-testid="store-tile-collectors">
        <span className="mk-stat__label">Collectors</span>
        <span className="mk-stat__value">{trend.collectors.total}</span>
        <span className="mk-stat__foot">{trend.collectors.repeat} came back</span>
      </div>
      {stats.royalties ? (
        <div className="mk-stat">
          <span className="mk-stat__label">Resales of your items</span>
          <span className="mk-stat__value">{stats.royalties.resales}</span>
          <span className="mk-stat__foot">{formatMana(stats.royalties.volumeWei as ManaWei, { group: true })} MANA traded</span>
        </div>
      ) : null}
    </div>
  );
}

function itemState(item: StoreItem) {
  switch (item.state) {
    case "discounted":
      return item.priceCredits !== null ? `On sale for ${item.priceCredits} credits` : "On sale";
    case "classic":
      return item.manaWei ? `Listed for ${formatMana((parseManaWei(item.manaWei) ?? 0n) as ManaWei, { group: true })} MANA only` : "Listed in MANA only";
    case "unlisted":
      return "Not listed";
    case "soldout":
      return "Sold out";
    case "unknown":
      return "Status unavailable";
  }
}

function CollectionRows({ collection, catalogue }: { collection: StoreCollection; catalogue: PublishableItem[] }) {
  const [open, setOpen] = useState(false);
  const [action, setAction] = useState<{ kind: "list" | "issue"; key: string } | null>(null);
  const itemFor = (key: string) => catalogue.find((i) => `${i.contractAddress.toLowerCase()}-${i.blockchainItemId}` === key);
  return (
    <Fragment>
      <tr data-testid="store-collection">
        <td>
          <button type="button" className="mk-link" aria-expanded={open} onClick={() => setOpen(!open)}>
            {collection.name}
          </button>
          {collection.exhausted ? <span className="mk-badge"> sold out</span> : null}
        </td>
        <td className="mk-num">{collection.items.length}</td>
        <td className="mk-num">{collection.listed}</td>
        <td className="mk-num">{collection.sold}</td>
        <td className="mk-num">{mana(collection.earningsWei)}</td>
        <td className="mk-num">
          {collection.claimed.toLocaleString("en-US")} / {collection.runTotal.toLocaleString("en-US")}
        </td>
      </tr>
      {open
        ? collection.items.map((item) => {
            const publishable = itemFor(item.key);
            const acting = action?.key === item.key ? action.kind : null;
            return (
              <tr key={item.key} data-testid="store-item">
                <td colSpan={6}>
                  <div className="mk-row">
                    <div className="mk-row__main">
                      <Link className="mk-link" to={shopPath("itemDetail", { contractAddress: collection.contractAddress, itemId: item.itemId })}>
                        {item.name}
                      </Link>
                      <span className="mk-row__sub" data-testid="store-item-state">
                        {itemState(item)} · {item.sold} sold · {item.left.toLocaleString("en-US")} left
                      </span>
                    </div>
                    <div className="mk-row__actions">
                      {publishable && item.state === "unlisted" && acting !== "list" ? (
                        <Button size="sm" onClick={() => setAction({ kind: "list", key: item.key })}>
                          Put on sale
                        </Button>
                      ) : null}
                      {item.state === "classic" ? (
                        <a className="mk-link" href="#store-import">
                          Move into the Shop
                        </a>
                      ) : null}
                      {publishable && item.left > 0 && acting !== "issue" ? (
                        <Button size="sm" variant="secondary" onClick={() => setAction({ kind: "issue", key: item.key })}>
                          Issue copies
                        </Button>
                      ) : null}
                    </div>
                    {publishable && acting === "list" ? <PrimaryListForm item={publishable} onClose={() => setAction(null)} /> : null}
                    {publishable && acting === "issue" ? <IssueForm item={publishable} onClose={() => setAction(null)} /> : null}
                  </div>
                </td>
              </tr>
            );
          })
        : null}
    </Fragment>
  );
}

export function CollectionsTable({ stats, catalogue }: { stats: StoreStats; catalogue: PublishableItem[] }) {
  if (stats.collections.length === 0)
    return <EmptyState variant="inline" title="No collections yet" subtitle="Publish a collection in the Builder to sell it here." data-testid="shop-empty" />;
  return (
    <table className="mk-table" aria-label="Collections">
      <thead>
        <tr>
          <th>Collection</th>
          <th className="mk-num">Items</th>
          <th className="mk-num">Listed</th>
          <th className="mk-num">Sold</th>
          <th className="mk-num">Earned</th>
          <th className="mk-num">Claimed</th>
        </tr>
      </thead>
      <tbody>
        {stats.collections.map((c) => (
          <CollectionRows key={c.contractAddress} collection={c} catalogue={catalogue} />
        ))}
      </tbody>
    </table>
  );
}

export function SalesHistory({ address }: { address: string }) {
  const [page, setPage] = useState(0);
  const query = useQuery({ queryKey: ["store-sales-page", address, page], queryFn: () => fetchSalesPage({ seller: address }, { first: SALES_PER_PAGE, skip: page * SALES_PER_PAGE }) });
  const pages = query.data ? Math.max(1, Math.ceil(query.data.total / SALES_PER_PAGE)) : 1;
  return (
    <section className="mk-panel" aria-label="Recent sales">
      <h2 className="mk-panel__title">Recent sales</h2>
      <p className="mk-hint">The latest sales across your collections</p>
      {query.isPending ? (
        <Loading label="Loading sales…" />
      ) : query.isError ? (
        <LoadFailure error={query.error} what="Your sales" onRetry={() => void query.refetch()} />
      ) : query.data.rows.length === 0 ? (
        <EmptyState variant="inline" title="No sales in this period yet." data-testid="shop-empty" />
      ) : (
        <>
          <table className="mk-table">
            <thead>
              <tr>
                <th>Date</th>
                <th>Kind</th>
                <th>Buyer</th>
                <th className="mk-num">Price</th>
              </tr>
            </thead>
            <tbody>
              {query.data.rows.map((row) => (
                <tr key={row.id} data-testid="store-sale-row">
                  <td>{formatDate(row.timestamp)}</td>
                  <td>{row.type === "mint" ? "New copy" : row.type === "bid" ? "Accepted bid" : "Resale"}</td>
                  <td>{shortAddr(row.buyer)}</td>
                  <td className="mk-num">{mana((parseManaWei(row.price) ?? 0n) as bigint)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <div className="mk-actions">
            <Button size="sm" variant="ghost" disabled={page === 0} onClick={() => setPage(page - 1)}>
              Previous
            </Button>
            <span className="mk-hint">
              Page {page + 1} of {pages}
            </span>
            <Button size="sm" variant="ghost" disabled={page + 1 >= pages} onClick={() => setPage(page + 1)}>
              Next
            </Button>
          </div>
        </>
      )}
    </section>
  );
}

export function MyStoreView({ address }: { address: string }) {
  const caps = useShopCapabilities();
  const [period, setPeriod] = useState<StorePeriod>("30d");
  const store = useStore(address);
  const data = useStoreStats(address, period);
  const names = useMemo(() => Object.fromEntries((data.catalogue ?? []).map((i) => [i.contractAddress.toLowerCase(), i.collectionName])), [data.catalogue]);
  const items = data.catalogue?.length ?? 0;
  return (
    <div className="mk-section">
      <div className="mk-page__head">
        <div>
          <p className="mk-hint">
            {data.stats ? `${data.stats.collections.length} collections · ${items} items` : ""}
            {store.data?.description ? ` · ${store.data.description}` : ""}
          </p>
        </div>
        <div className="mk-actions">
          <Link className="mk-link" to={shopPath("account", { address })}>
            View your public store
          </Link>
          <Link className="mk-link" to={shopPath("storeSettings")} state={{ from: shopPath("myStore") }}>
            Edit store
          </Link>
        </div>
      </div>
      <div className="mk-tabs" role="group" aria-label="Period">
        {PERIODS.map((p) => (
          <button key={p.key} type="button" className="mk-tab" aria-pressed={period === p.key} aria-current={period === p.key ? "page" : undefined} onClick={() => setPeriod(p.key)}>
            {p.label}
          </button>
        ))}
      </div>
      {data.needsSigner ? (
        <p className="mk-note" role="status">
          Your sign-in has expired. Sign in again to see your collections.
        </p>
      ) : data.isLoading ? (
        <Loading label="Loading your store…" />
      ) : data.error ? (
        <LoadFailure error={data.error} what="Your store" onRetry={data.refetch} />
      ) : data.stats ? (
        <>
          <StoreTiles stats={data.stats} trend={data.trend} />
          {data.stats.unattributed > 0 ? <p className="mk-hint">{data.stats.unattributed} sales are in collections not listed below (resales of other creators' items, or unpublished collections).</p> : null}
          <CollectionsTable stats={data.stats} catalogue={data.catalogue ?? []} />
        </>
      ) : null}
      {caps.creatorSales ? <CreatorSales address={address} saleable={data.saleable} names={names} /> : null}
      <section className="mk-panel" id="store-import" aria-label="Listings to move">
        <h2 className="mk-panel__title">Bring your listings into the Shop</h2>
        <ImportListings />
      </section>
      <SalesHistory address={address} />
    </div>
  );
}

function OrderFromLink({ address }: { address: string }) {
  const [params] = useSearchParams();
  const navigate = useNavigate();
  const orderId = params.get("order")?.trim() || null;
  const [state, setState] = useState<"looking" | "gone" | "failed" | null>(orderId ? "looking" : null);
  useEffect(() => {
    if (!orderId) return;
    let live = true;
    setState("looking");
    fetchOpenOrderById(address, orderId).then(
      (order) => {
        if (!live) return;
        if (order?.tokenId) navigate(shopPath("tokenCancel", { contractAddress: order.contractAddress.toLowerCase(), tokenId: order.tokenId }), { replace: true });
        else setState("gone");
      },
      () => live && setState("failed"),
    );
    return () => {
      live = false;
    };
  }, [address, orderId, navigate]);
  if (!state) return null;
  return (
    <p className="mk-note" role="status" data-testid="shop-order-from-link">
      {state === "looking"
        ? "Opening the listing from your link…"
        : state === "gone"
          ? "The listing from your link is no longer open, so there is nothing to cancel."
          : "We couldn't open the listing from your link. Find it in your items to cancel it."}
    </p>
  );
}

export default function MyStore() {
  const { account } = useShopWallet();
  const caps = useShopCapabilities();
  if (myStoreAccess(caps, account) === "off") return <Navigate to={shopPath("overview")} replace />;
  if (!account) return <ShopSignInPanel page="My Store" title="Sign in to view your store" body="See how your collections are selling, what needs attention, and the discounts you have running." />;
  return (
    <EnsureQueryClient>
      <OrderFromLink address={account} />
      <StoreDashboard address={account} />
    </EnsureQueryClient>
  );
}
