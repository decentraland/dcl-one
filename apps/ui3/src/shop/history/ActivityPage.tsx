// from: shop@2d3bf62:app/src/pages/Activity.tsx, app/src/pages/Activity.styles.ts,
//       app/src/components/EmptyState/{EmptyState.tsx,EmptyState.styles.ts} (the signed-out and empty panels),
//       app/src/assets/empty/{signin-empty,sales-empty}.svg, app/src/assets/credits-product.svg and
//       app/src/assets/names/name-glyph.svg (vendored in ./assets);
//       marketplace@f3956826:webapp/src/components/ActivityPage/ActivityPage.tsx
//       (the wallet's transactions, FETCH_USER_ACTIVITY_REQUEST). Marketplace UI: Apache-2.0, Copyright 2020
//       Metaverse Holdings Ltd. Supersedes v2 ui3/src/marketplace/pages/MkActivityPage.tsx and sites
//       marketplace.activity.tsx.
import { lazy, Suspense, useMemo, useState, type ComponentType, type ReactNode } from "react";
import { Link, useSearchParams } from "react-router";
import { useInfiniteQuery, useQuery, useQueryClient } from "@tanstack/react-query";

import { shortAddress } from "../model/address";
import { chainLabel } from "../model/chain";
import { manaWei } from "../model/money";
import { useShopWallet } from "../wallet/context";
import { explorerTxUrl } from "../checkout/chain/config";
import { SHOP_QUERY_KEYS } from "../checkout/lib/after-purchase";
import { fetchAssetDisplay, fetchTradeDisplay, fetchUserActivity, fetchUserSales } from "../checkout/lib/api";
import { formatAmount } from "../checkout/lib/currency";
import { EnsureQueryClient, useShopServices } from "../checkout/services";
import { creditOrderPill, fetchUserCreditOrders, fetchUserPurchases, resumeCreditOrder, type CreditOrder } from "../credits/credits";
import { CreditsAmount, ManaAmount } from "../credits/components/Amount";
import { buildActivityFeed, filterActivity, type ActivityEntry, type ActivityFilter, type SaleRecord } from "./activity";
import { foldOrderLines, purchaseOrderPill, type OrderLineItem, type PurchaseOrder } from "./purchases";
import signInIllustration from "./assets/signin-empty.svg";
import salesEmptyIllustration from "./assets/sales-empty.svg";
import creditsProduct from "./assets/credits-product.svg";
import nameGlyph from "./assets/name-glyph.svg";
import "./activitypage.css";

const PAGE = 24;
const FILTERS: { id: ActivityFilter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "purchases", label: "Purchases" },
  { id: "sales", label: "Sales" },
];

const EMPTY_BODY: Record<ActivityFilter, string> = {
  all: "Your purchases and sales will appear here.",
  purchases: "Explore the Shop and find something awesome to make it yours.",
  sales: "When someone buys one of your items it’ll appear here.",
};

const IMPORT = import.meta.glob<{ default?: ComponentType; ImportListings?: ComponentType }>(["../seller/components/ImportListings.tsx", "../components/ImportListings.tsx"]);

function formatDate(ms: number): string {
  try {
    return new Date(ms).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
  } catch {
    return "";
  }
}

function assetHref(contractAddress: string | null, ids: { tokenId?: string | null; itemId?: string | null }): string | null {
  if (!contractAddress) return null;
  if (ids.tokenId) return `/shop/token/${contractAddress}/${ids.tokenId}`;
  if (ids.itemId) return `/shop/item/${contractAddress}/${ids.itemId}`;
  return null;
}

function ShopEmpty({ icon, title, body, cta, solid, fill, testId }: { icon: string; title: string; body?: string; cta?: ReactNode; solid?: boolean; fill?: boolean; testId?: string }) {
  return (
    <div className="shop-activity-empty" data-fill={fill ? "true" : undefined} data-cta={solid ? "solid" : undefined} data-testid={testId}>
      <img className="shop-activity-empty__art" src={icon} alt="" aria-hidden="true" />
      <div className="shop-activity-empty__text">
        <p className="shop-activity-empty__title">{title}</p>
        {body ? <p className="shop-activity-empty__body">{body}</p> : null}
      </div>
      {cta}
    </div>
  );
}

function Line({ name, thumbnail, thumb, href, meta, children, loading, testId }: { name: string; thumbnail?: string; thumb?: ReactNode; href?: string | null; meta?: string; children?: ReactNode; loading?: boolean; testId?: string }) {
  const body = (
    <>
      {loading ? (
        <span className="shop-activity__thumb shop-activity__skeleton" aria-hidden="true" />
      ) : (
        thumb ?? (
          <span className="shop-activity__thumb" aria-hidden="true">
            {thumbnail ? <img src={thumbnail} alt="" loading="lazy" /> : null}
          </span>
        )
      )}
      <span className="shop-activity__lineinfo">
        {loading ? <span className="shop-activity__nameph shop-activity__skeleton" aria-label="Loading…" /> : <span className="shop-activity__linename" title={name}>{name}</span>}
        {meta ? <span className="shop-activity__linemeta">{meta}</span> : null}
      </span>
      {children ? <span className="shop-activity__lineprice">{children}</span> : null}
    </>
  );
  return href ? (
    <Link className="shop-activity__line" data-link="true" to={href} data-testid={testId}>
      {body}
    </Link>
  ) : (
    <div className="shop-activity__line" data-testid={testId}>
      {body}
    </div>
  );
}

function OrderLine({ item }: { item: OrderLineItem }) {
  const { client } = useShopServices();
  const resolvable = !!item.tradeId || (!!item.contractAddress && !!item.itemId);
  const { data, isLoading } = useQuery({
    queryKey: ["shop-order-line", item.tradeId, item.contractAddress, item.itemId],
    queryFn: () => (item.tradeId ? fetchTradeDisplay(item.tradeId, client) : fetchAssetDisplay(item.contractAddress!, { itemId: item.itemId }, client)),
    enabled: resolvable && !item.registeredName,
    staleTime: 5 * 60_000,
  });
  if (item.registeredName) {
    return (
      <Line
        name={`@${item.registeredName}`}
        meta="Decentraland NAME"
        href={null}
        testId="activity-name-line"
        thumb={
          <span className="shop-activity__thumb" data-kind="name" aria-hidden="true">
            <img src={nameGlyph} alt="" />
          </span>
        }
      >
        <CreditsAmount credits={item.credits} />
      </Line>
    );
  }
  return (
    <Line
      name={data?.name ?? "Item"}
      thumbnail={data?.thumbnail}
      href={assetHref(data?.contractAddress ?? item.contractAddress, { tokenId: data?.tokenId, itemId: data?.itemId ?? item.itemId })}
      meta={item.quantity > 1 ? `×${item.quantity}` : undefined}
      loading={resolvable && isLoading}
    >
      <CreditsAmount credits={item.credits} />
    </Line>
  );
}

const PILL_TEXT = { SETTLED: "Completed", PENDING: "Processing", FAILED: "Failed", UNFINISHED: "Unfinished", SOLD: "Sold" } as const;

function Card({ date, label, pill, total, lead, action, children, testId }: { date: number; label: string; pill: keyof typeof PILL_TEXT; total: ReactNode; lead?: ReactNode; action?: ReactNode; children?: ReactNode; testId: string }) {
  return (
    <article className="shop-activity__card" data-testid={testId}>
      <header className="shop-activity__cardhead">
        {lead}
        <span className="shop-activity__headleft">
          <span className="shop-activity__date">{formatDate(date)}</span>
          <span className="shop-activity__label">{label}</span>
        </span>
        <span className="shop-activity__right">
          {action}
          <span className="shop-activity__pill" data-status={pill}>
            {PILL_TEXT[pill]}
          </span>
          <span className="shop-activity__total">{total}</span>
        </span>
      </header>
      {children}
    </article>
  );
}

function OrderCard({ order }: { order: PurchaseOrder }) {
  const lines = foldOrderLines(order.lines);
  const units = lines.reduce((n, l) => n + l.quantity, 0);
  const pill = purchaseOrderPill(order.status);
  return (
    <Card date={order.createdAt} label={`Purchase · ${units} ${units === 1 ? "item" : "items"}`} pill={pill} total={<CreditsAmount credits={order.totalCredits} />} testId="purchase-order">
      {pill === "FAILED" ? <p className="shop-activity__note">This purchase didn't go through. The Credits it reserved are back in your balance.</p> : null}
      <div className="shop-activity__lines">
        {lines.map((l) => (
          <OrderLine key={l.key} item={l} />
        ))}
      </div>
    </Card>
  );
}

function SaleLine({ sale, meta }: { sale: { contractAddress: string; tokenId: string | null; itemId: string | null }; meta: string }) {
  const { client } = useShopServices();
  const { data, isLoading } = useQuery({
    queryKey: ["shop-asset-display", sale.contractAddress, sale.tokenId, sale.itemId],
    queryFn: () => fetchAssetDisplay(sale.contractAddress, { tokenId: sale.tokenId, itemId: sale.itemId }, client),
    staleTime: 5 * 60_000,
  });
  return <Line name={data?.name ?? "Item"} thumbnail={data?.thumbnail} href={assetHref(sale.contractAddress, sale)} meta={meta} loading={isLoading} />;
}

function Mana({ wei, chainId }: { wei: string; chainId: number | null }) {
  try {
    return <ManaAmount wei={manaWei(wei)} chainId={chainId ?? undefined} />;
  } catch {
    return <span>— MANA</span>;
  }
}

function CreditOrderCard({ order }: { order: CreditOrder }) {
  const { client, openExternal } = useShopServices();
  const qc = useQueryClient();
  const [state, setState] = useState<"idle" | "busy" | string>("idle");
  const pill = creditOrderPill(order.status);
  async function resume() {
    setState("busy");
    const r = await resumeCreditOrder(order.id, client);
    if (r.kind === "url" && /^https:\/\/([a-z0-9-]+\.)*stripe\.com\//i.test(r.url)) {
      openExternal(r.url);
      return;
    }
    setState(r.kind === "paid" ? "Already paid: the Credits are on the way." : r.kind === "expired" ? "This checkout expired." : "Couldn't reopen it right now.");
    void qc.invalidateQueries({ queryKey: [SHOP_QUERY_KEYS.creditOrders] });
  }
  return (
    <Card
      date={order.createdAt}
      label={`Credit purchase · Paid $${(order.usdCents / 100).toFixed(2)}`}
      pill={pill}
      lead={
        <span className="shop-activity__creditthumb" aria-hidden="true">
          <img src={creditsProduct} alt="" />
        </span>
      }
      action={
        order.status === "initiated" ? (
          <button type="button" className="shop-activity__resume" onClick={() => void resume()} disabled={state === "busy"} data-testid="resume-order">
            {state === "busy" ? "Opening…" : "Continue"}
          </button>
        ) : null
      }
      total={
        <span data-kind={pill === "UNFINISHED" ? undefined : "income"}>
          {pill === "UNFINISHED" ? "" : "+"}
          {formatAmount(order.credits)}
        </span>
      }
      testId="credit-order"
    >
      {state !== "idle" && state !== "busy" ? (
        <p className="shop-activity__note" role="status">
          {state}
        </p>
      ) : null}
    </Card>
  );
}

function Entry({ entry }: { entry: ActivityEntry }) {
  switch (entry.kind) {
    case "purchase":
      return <OrderCard order={entry.order} />;
    case "credit":
      return <CreditOrderCard order={entry.order} />;
    case "mana-purchase":
      return (
        <Card date={entry.createdAt} label="Purchase · paid with MANA" pill="SETTLED" total={<Mana wei={entry.sale.manaWei} chainId={entry.sale.chainId} />} testId="activity-mana-purchase">
          <div className="shop-activity__lines">
            <SaleLine sale={entry.sale} meta={`Bought from ${shortAddress(entry.sale.seller)}`} />
          </div>
        </Card>
      );
    case "sale":
      return (
        <Card date={entry.createdAt} label="Sale" pill="SOLD" total={<span data-kind="income">+<Mana wei={entry.sale.manaWei} chainId={entry.sale.chainId} /></span>} testId="activity-sale">
          <div className="shop-activity__lines">
            <SaleLine sale={entry.sale} meta={`Sold to ${shortAddress(entry.sale.counterparty)}`} />
          </div>
        </Card>
      );
  }
}

function Transactions({ address }: { address: string }) {
  const { client } = useShopServices();
  const { data, isError } = useQuery({ queryKey: [SHOP_QUERY_KEYS.activity, "wallet", address], queryFn: () => fetchUserActivity({}, client), staleTime: 30_000, retry: false });
  const items = (data?.items ?? []).filter((t) => typeof t.txHash === "string");
  if (isError || items.length === 0) return null;
  return (
    <section className="shop-activity__txs" aria-labelledby="shop-activity-txs">
      <h2 id="shop-activity-txs">Wallet transactions</h2>
      <ul>
        {items.slice(0, 10).map((t) => {
          const url = t.txHash && t.chainId ? explorerTxUrl(t.chainId, t.txHash) : null;
          return (
            <li key={t.txHash}>
              <span>{t.type ?? "Transaction"}</span>
              <span>{t.chainId ? chainLabel(t.chainId) : ""}</span>
              <span className="shop-activity__pill" data-status={t.status === "confirmed" ? "SETTLED" : t.status === "reverted" ? "FAILED" : "PENDING"}>
                {t.status ?? "pending"}
              </span>
              {url ? (
                <a href={url} target="_blank" rel="noopener noreferrer">
                  View
                </a>
              ) : null}
            </li>
          );
        })}
      </ul>
    </section>
  );
}

function ImportSection() {
  const load = IMPORT["../seller/components/ImportListings.tsx"] ?? IMPORT["../components/ImportListings.tsx"];
  const Import = useMemo(
    () =>
      load
        ? lazy(async () => {
            const mod = await load();
            return { default: (mod.default ?? mod.ImportListings ?? (() => null)) as ComponentType };
          })
        : null,
    [load],
  );
  if (!Import) {
    return (
      <ShopEmpty
        icon={salesEmptyIllustration}
        title="Import your listings"
        body="Bring listings from the classic Marketplace into your store."
        cta={
          <Link className="shop-activity-empty__cta" to="/shop/my-store">
            Go to my store
          </Link>
        }
      />
    );
  }
  return (
    <Suspense fallback={<div className="shop-activity__panelfallback" aria-busy="true" />}>
      <Import />
    </Suspense>
  );
}

function ActivityScreen() {
  const { client } = useShopServices();
  const wallet = useShopWallet();
  const account = wallet.account;
  const [params, setParams] = useSearchParams();
  const [filter, setFilter] = useState<ActivityFilter>("all");
  const listings = params.get("section") === "listings" || params.get("view") === "migrate";

  const purchases = useInfiniteQuery({
    queryKey: [SHOP_QUERY_KEYS.purchases, account],
    enabled: !!account && !listings && filter !== "sales",
    initialPageParam: 0,
    queryFn: ({ pageParam }) => fetchUserPurchases(account!, { all: true, first: PAGE, skip: pageParam }, client),
    getNextPageParam: (last, pages) => {
      const loaded = pages.reduce((n, p) => n + p.items.length, 0);
      return loaded < last.total ? loaded : undefined;
    },
  });
  const creditOrders = useQuery({
    queryKey: [SHOP_QUERY_KEYS.creditOrders, account],
    enabled: !!account && !listings && filter !== "sales",
    queryFn: () => fetchUserCreditOrders(account!, { first: 50 }, client),
  });
  const sales = useQuery({
    queryKey: ["shop-sales", account, "seller"],
    enabled: !!account && !listings && filter !== "purchases",
    queryFn: () => fetchUserSales(account!, { role: "seller", first: PAGE }, client),
  });
  const manaPurchases = useQuery({
    queryKey: ["shop-sales", account, "buyer"],
    enabled: !!account && !listings && filter !== "sales",
    queryFn: () => fetchUserSales(account!, { role: "buyer", first: PAGE }, client),
  });

  const feed = useMemo(
    () =>
      filterActivity(
        buildActivityFeed({
          purchases: purchases.data?.pages.flatMap((p) => p.items) ?? [],
          sales: (sales.data?.items ?? []) as SaleRecord[],
          creditOrders: creditOrders.data?.items ?? [],
          manaPurchases: manaPurchases.data?.items ?? [],
        }),
        filter,
      ),
    [purchases.data, sales.data, creditOrders.data, manaPurchases.data, filter],
  );

  if (!account) {
    const onSignIn = wallet.host?.onSignIn;
    return (
      <main className="shop-activity shop-activity--signedout" aria-label="Activity">
        <ShopEmpty
          icon={signInIllustration}
          title="Sign in to see your activity"
          body="Your purchases and sales show up here once you sign in."
          solid
          fill
          testId="activity-signed-out"
          cta={
            onSignIn ? (
              <button type="button" className="shop-activity-empty__cta" onClick={onSignIn}>
                Sign in
              </button>
            ) : undefined
          }
        />
      </main>
    );
  }

  function selectFilter(next: ActivityFilter) {
    setFilter(next);
    if (!listings) return;
    const q = new URLSearchParams(params);
    q.delete("section");
    q.delete("view");
    setParams(q, { replace: true });
  }

  const loading = purchases.isLoading || sales.isLoading || creditOrders.isLoading || manaPurchases.isLoading;
  const failed = purchases.isError && sales.isError;
  return (
    <main className="shop-activity" aria-labelledby="shop-activity-title">
      <div className="shop-activity__column">
        <h1 id="shop-activity-title" className="shop-activity__title">
          {listings ? "Import listings" : "Activity"}
        </h1>
        <div className="shop-activity__filters" role="tablist" aria-label="Activity">
          {FILTERS.map((f) => (
            <button
              key={f.id}
              type="button"
              role="tab"
              aria-selected={!listings && filter === f.id}
              data-active={!listings && filter === f.id}
              data-testid={`activity-filter-${f.id}`}
              className="shop-activity__filter"
              onClick={() => selectFilter(f.id)}
            >
              {f.label}
            </button>
          ))}
        </div>
        {listings ? (
          <ImportSection />
        ) : failed ? (
          <ShopEmpty
            icon={salesEmptyIllustration}
            title="We couldn't load your activity"
            body="Try again in a moment."
            testId="activity-error"
            cta={
              <button type="button" className="shop-activity-empty__cta" onClick={() => void purchases.refetch()}>
                Try again
              </button>
            }
          />
        ) : loading && feed.length === 0 ? (
          <div className="shop-activity__feed" role="status" aria-label="Loading activity">
            {Array.from({ length: 5 }).map((_, i) => (
              <div key={i} className="shop-activity__cardskeleton shop-activity__skeleton" />
            ))}
          </div>
        ) : feed.length === 0 ? (
          <ShopEmpty
            icon={salesEmptyIllustration}
            title="Nothing here... yet!"
            body={EMPTY_BODY[filter]}
            testId={`activity-empty-${filter}`}
            cta={
              <Link className="shop-activity-empty__cta" to="/shop/items">
                Explore Shop
              </Link>
            }
          />
        ) : (
          <>
            <div className="shop-activity__feed">
              {feed.map((e) => (
                <Entry key={e.id} entry={e} />
              ))}
              {purchases.isFetchingNextPage ? <div className="shop-activity__cardskeleton shop-activity__skeleton" /> : null}
            </div>
            {purchases.hasNextPage ? (
              <div className="shop-activity__more">
                <button type="button" className="shop-activity__morebtn" onClick={() => void purchases.fetchNextPage()} disabled={purchases.isFetchingNextPage}>
                  {purchases.isFetchingNextPage ? "Loading…" : "Load more"}
                </button>
              </div>
            ) : null}
          </>
        )}
        {listings ? null : <Transactions address={account} />}
      </div>
    </main>
  );
}

export default function ActivityPage() {
  return (
    <EnsureQueryClient>
      <ActivityScreen />
    </EnsureQueryClient>
  );
}
