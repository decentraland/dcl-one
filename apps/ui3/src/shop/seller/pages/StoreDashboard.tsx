// from: shop@2d3bf62:app/src/pages/MyStore.tsx (the page shell: Masthead with the avatar, name, title, summary,
//       "View public store" and "Edit store"; the "Store performance" head with its range trigger; the summary
//       tiles; the collections panel; "Your audience"), app/src/pages/MyStore.styles.ts, app/src/intl/en.json (myStore.*)
import { useEffect, useMemo, useRef, useState } from "react";
import { Link } from "react-router";

import { formatMana, type ManaWei } from "../../model/money";
import { shopPath } from "../../routes";
import { LoadFailure, Loading } from "../../market/components/kit";
import Icon from "../../discovery/components/Icon";
import { useProfile } from "../../discovery/hooks";
import CreatorSales from "../components/CreatorSales";
import ImportListings from "../components/ImportListings";
import { useCreatorSales, useStore, useStoreStats, type StorePeriod } from "../hooks";
import { useShopCapabilities } from "../lib/featureFlags";
import type { Delta } from "../lib/storeMetrics";
import type { StoreStats } from "../lib/storeStats";
import { CollectionsTable, SalesHistory } from "./MyStore";
import calendarIcon from "./assets/calendar.svg?url";
import penIcon from "./assets/pen.svg?url";
import "./sellerpages.css";
import "./storedashboard.css";

const PERIODS: { key: StorePeriod; label: string }[] = [
  { key: "7d", label: "Last 7 days" },
  { key: "30d", label: "Last 30 days" },
  { key: "all", label: "All time" },
];

const plural = (n: number, one: string, other: string) => `${n.toLocaleString("en-US")} ${n === 1 ? one : other}`;

function MaskIcon({ src, size = 20 }: { src: string; size?: number }) {
  return <span className="seller-ico" aria-hidden="true" style={{ width: size, height: size, WebkitMaskImage: `url("${src}")`, maskImage: `url("${src}")` }} />;
}

function DeltaTag({ delta }: { delta: Delta | null }) {
  if (!delta) return null;
  if (delta.pct === null) return delta.current > 0 ? <span className="seller-delta" data-testid="store-delta">new</span> : null;
  const rounded = Math.round(delta.pct);
  if (rounded === 0) return null;
  return (
    <span className="seller-delta" data-dir={rounded > 0 ? "up" : "down"} data-testid="store-delta">
      {rounded > 0 ? "▲" : "▼"} {Math.abs(rounded)}%
    </span>
  );
}

function PeriodTrigger({ period, onPick }: { period: StorePeriod; onPick: (p: StorePeriod) => void }) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent | KeyboardEvent) => {
      if (e instanceof KeyboardEvent ? e.key === "Escape" : !root.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", close);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", close);
    };
  }, [open]);
  const label = PERIODS.find((p) => p.key === period)!.label;
  return (
    <div className="seller-range" ref={root}>
      <button type="button" className="seller-range__trigger" aria-expanded={open} aria-haspopup="menu" aria-label={`Period: ${label}`} onClick={() => setOpen(!open)} data-testid="store-period-trigger">
        <MaskIcon src={calendarIcon} size={16} />
        <span>{label}</span>
        <span className="seller-range__chev" data-open={open ? "" : undefined} aria-hidden="true" />
      </button>
      {open ? (
        <div className="seller-range__menu" role="menu" aria-label="Period">
          {PERIODS.map((p) => (
            <button
              key={p.key}
              type="button"
              role="menuitemradio"
              aria-checked={p.key === period}
              className="seller-range__option"
              onClick={() => {
                onPick(p.key);
                setOpen(false);
              }}
            >
              {p.label}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function Tiles({ stats, trend, discounts }: { stats: StoreStats; trend: ReturnType<typeof useStoreStats>["trend"]; discounts: number }) {
  const total = stats.listed + stats.classic + stats.neverListed + stats.soldOut;
  return (
    <section className="seller-tiles" aria-label="Store summary">
      <div className="seller-tile" data-testid="store-tile-sold">
        <span className="seller-tile__key">Items sold</span>
        <span className="seller-tile__value">
          {stats.sold.toLocaleString("en-US")} <DeltaTag delta={trend.sold} />
        </span>
        <span className="seller-tile__foot">
          {stats.sold === 0 ? "Nothing sold in this period" : stats.resales > 0 ? `${plural(stats.mints, "first sale", "first sales")} · ${stats.resales.toLocaleString("en-US")} resold` : plural(stats.mints, "first sale", "first sales")}
        </span>
      </div>
      <div className="seller-tile" data-testid="store-tile-earnings">
        <span className="seller-tile__key">Earnings</span>
        <span className="seller-tile__value">
          {formatMana(stats.earningsWei as ManaWei, { group: true, decimals: 2 })} <span className="seller-tile__unit">MANA</span> <DeltaTag delta={trend.earnings} />
        </span>
        <span className="seller-tile__foot">
          {stats.partial ? (
            <>
              <span className="seller-estimate">Estimate</span> covers the most recent sales of the period
            </>
          ) : (
            "Paid to you this period"
          )}
        </span>
      </div>
      <div className="seller-tile" data-testid="store-tile-listed">
        <span className="seller-tile__key">Listed now</span>
        <span className="seller-tile__value">
          {stats.listed.toLocaleString("en-US")} {total > 0 ? <span className="seller-tile__unit">of {total.toLocaleString("en-US")}</span> : null}
        </span>
        <span className="seller-tile__foot">{total === 0 ? "No items published yet" : stats.neverListed === 0 ? "Everything is listed" : `${stats.neverListed.toLocaleString("en-US")} never listed`}</span>
      </div>
      <div className="seller-tile" data-testid="store-tile-discounts">
        <span className="seller-tile__key">Discounts running</span>
        <span className="seller-tile__value">{discounts}</span>
        <span className="seller-tile__foot">{discounts > 0 ? "Details below" : "None right now"}</span>
      </div>
    </section>
  );
}

function Audience({ stats, trend }: { stats: StoreStats; trend: ReturnType<typeof useStoreStats>["trend"] }) {
  const { total, repeat, repeatPct } = trend.collectors;
  return (
    <>
      <div className="seller-section-head">
        <h2 className="seller-section-title">Your audience</h2>
        <p className="seller-section-sub">Who is buying, and what they are worth to you.</p>
      </div>
      <section className="seller-tiles seller-tiles--two" aria-label="Your audience">
        <div className="seller-tile" data-testid="store-tile-collectors">
          <span className="seller-tile__key">Collectors</span>
          <span className="seller-tile__value">{total.toLocaleString("en-US")}</span>
          <span className="seller-tile__foot">{total === 0 ? "Nobody has bought yet" : repeat === 0 ? "Nobody has bought twice" : `${repeat.toLocaleString("en-US")} came back, ${Math.round(repeatPct)}%`}</span>
        </div>
        {stats.royalties ? (
          <div className="seller-tile" data-testid="store-tile-royalties">
            <span className="seller-tile__key">Royalties</span>
            <span className="seller-tile__value">
              ≈ {formatMana(((stats.royalties.volumeWei * 25_000n) / 1_000_000n) as ManaWei, { group: true, decimals: 2 })} <span className="seller-tile__unit">MANA</span>
            </span>
            <span className="seller-tile__foot">
              {plural(stats.royalties.resales, "resale", "resales")} · {formatMana(stats.royalties.volumeWei as ManaWei, { group: true, decimals: 2 })} MANA traded
            </span>
          </div>
        ) : null}
      </section>
    </>
  );
}

export default function StoreDashboard({ address }: { address: string }) {
  const caps = useShopCapabilities();
  const [period, setPeriod] = useState<StorePeriod>("30d");
  const store = useStore(address);
  const profile = useProfile(address);
  const data = useStoreStats(address, period);
  const discounts = useCreatorSales(address, caps.creatorSales);
  const running = (discounts.data ?? []).filter((s) => s.status === "active" || s.status === "scheduled").length;
  const names = useMemo(() => Object.fromEntries((data.catalogue ?? []).map((i) => [i.contractAddress.toLowerCase(), i.collectionName])), [data.catalogue]);
  const items = (data.stats?.collections ?? []).reduce((n, c) => n + c.items.length, 0);
  const face = profile?.picture;
  const name = profile?.name ? profile.name.charAt(0).toUpperCase() + profile.name.slice(1) : null;
  return (
    <div className="seller-page">
      <div className="seller-store" data-testid="my-store">
        <header className="seller-masthead">
          <div className="seller-identity">
            <span className="seller-avatar" style={face ? { backgroundImage: `url(${face})` } : undefined} data-testid="store-avatar" aria-hidden="true" />
            <div className="seller-identity__text">
              {name ? <p className="seller-eyebrow">{name}</p> : null}
              <div className="seller-titleblock">
                <h1 className="seller-title">My Store</h1>
                <p className="seller-sub">
                  {data.stats ? `${plural(data.stats.collections.length, "collection", "collections")} · ${plural(items, "item", "items")}` : <span className="seller-sub__bar" />}
                  {store.data?.description ? <span className="seller-sub__desc"> · {store.data.description}</span> : null}
                </p>
              </div>
            </div>
          </div>
          <div className="seller-store-actions">
            <a className="seller-view-public" href={shopPath("creator", { address })} target="_blank" rel="noopener noreferrer" data-testid="store-view-public">
              View public store
              <Icon name="external-link" size={22} />
            </a>
            <Link className="seller-edit-store" to={shopPath("storeSettings")} state={{ from: shopPath("myStore") }} data-testid="store-edit">
              <MaskIcon src={penIcon} />
              Edit store
            </Link>
          </div>
        </header>

        <div className="seller-perf-head">
          <h2 className="seller-perf-title">Store performance</h2>
          <PeriodTrigger period={period} onPick={setPeriod} />
        </div>

        {data.needsSigner ? (
          <p className="seller-note" role="status">
            Your sign-in has expired. Sign in again to see your collections.
          </p>
        ) : data.isLoading ? (
          <Loading label="Loading your store…" />
        ) : data.error ? (
          <LoadFailure error={data.error} what="Your store" onRetry={data.refetch} />
        ) : data.stats ? (
          <>
            <Tiles stats={data.stats} trend={data.trend} discounts={running} />
            <section className="seller-panel" aria-labelledby="store-coll-h">
              <div className="seller-list-head">
                <h2 className="seller-panel__title" id="store-coll-h">
                  Collections ({data.stats.collections.length})
                </h2>
              </div>
              {data.stats.collections.length === 0 ? <p className="seller-empty-line">You haven't published a collection yet.</p> : <CollectionsTable stats={data.stats} catalogue={data.catalogue ?? []} />}
              {data.stats.unattributed > 0 ? <p className="seller-empty-line">{data.stats.unattributed} sales are in collections not listed here (resales of other creators' items, or unpublished collections).</p> : null}
            </section>
          </>
        ) : null}
        {caps.creatorSales ? (
          <div className="seller-panel seller-panel--host">
            <CreatorSales address={address} saleable={data.saleable} names={names} />
          </div>
        ) : null}
        <div className="seller-panel seller-panel--host">
          <SalesHistory address={address} />
        </div>
        {data.stats ? <Audience stats={data.stats} trend={data.trend} /> : null}
        <section className="seller-panel" id="store-import" aria-labelledby="store-import-h">
          <h2 className="seller-panel__title" id="store-import-h">
            Bring your listings into the Shop
          </h2>
          <ImportListings />
        </section>
      </div>
    </div>
  );
}
