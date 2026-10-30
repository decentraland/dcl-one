// from: marketplace@3bf3662f:webapp/src/components/RankingsTable/{RankingsTable.tsx,RankingsTable.css,utils.ts},
//       RankingsTable/RankingItemRow, RankingsTable/RankingCreatorRow, RankingsTable/RankingCollectorRow,
//       components/Rankings/TimeframeSelector, components/InfoTooltip, the translations under
//       home_page.analytics.rankings (locales/en.json), images/info.svg (vendored as ./info.svg, recoloured);
//       decentraland-ui@RarityBadge (small) and AssetImage's rarity wash; @dcl/schemas rarity colours.
// Copyright 2020 Metaverse Holdings Ltd. (marketplace, decentraland-ui, schemas), Apache-2.0.

import { useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { Link, useLocation, useNavigate } from "react-router";

import ManaMark from "../../../../atoms/ManaMark";
import { useMediaQuery } from "../../../../data/hooks/useMediaQuery";
import { fetchProfile, profileFaceUrl } from "../../../../data/catalyst/profile";
import Blockie from "../../../checkout/settings/Blockie";
import { useManaRate } from "../../../credits/hooks";
import { manaWeiToUsdCents } from "../../../checkout/lib/currency";
import { formatMana, parseManaWei } from "../../../model/money";
import { shopPath } from "../../../routes";
import { fetchRankings, type AnalyticsTimeframe, type CollectorRank, type CreatorRank, type ItemRank, type RankingEntity, type RankingsSortBy } from "../../lib/api";
import type { CatalogItem } from "@catalyst-api/market/CatalogItem";
import { shopClient } from "../../../api/shop";
import { toFavoriteItem, type FavoriteItem } from "../../lib/catalog";
import { LoadFailure, Loading } from "../../components/kit";
import infoIcon from "./info.svg?url";
import "./rankingstable.css";

const ENTITIES: RankingEntity[] = ["wearables", "emotes", "creators", "collectors"];
const ENTITY_TITLE: Record<RankingEntity, string> = { wearables: "Wearables", emotes: "Emotes", creators: "Creators", collectors: "Collectors" };
const TIMEFRAMES: { key: AnalyticsTimeframe; label: string }[] = [
  { key: "week", label: "7 days" },
  { key: "month", label: "30 days" },
  { key: "all", label: "All" },
];
const SORTS: { key: RankingsSortBy; label: string }[] = [
  { key: "most_volume", label: "Volume (MANA)" },
  { key: "most_sales", label: "Items (quantity)" },
];
const WEARABLE_CATEGORIES: Record<string, string> = {
  eyebrows: "Eyebrows", eyes: "Eyes", facial_hair: "Facial Hair", hair: "Hair", body_shape: "Body Shape", mouth: "Mouth",
  upper_body: "Upper Body", lower_body: "Lower Body", feet: "Feet", earring: "Earring", eyewear: "Eyewear", hat: "Hat",
  helmet: "Helmet", mask: "Mask", tiara: "Tiara", top_head: "Top Head", skin: "Skin", hands_wear: "Handwear",
};
const EMOTE_CATEGORIES: Record<string, string> = {
  dance: "Dance", stunt: "Stunt", greetings: "Greetings", fun: "Fun", poses: "Poses", reactions: "Reactions", horror: "Horror", miscellaneous: "Miscellaneous",
};
const RARITIES: [string, string, string, string][] = [
  ["unique", "Unique", "#FFE617", "#FFB626"],
  ["mythic", "Mythic", "#FB7DE3", "#FF63E1"],
  ["exotic", "Exotic", "#E4FFB8", "#CAFF73"],
  ["legendary", "Legendary", "#A657ED", "#842DDA"],
  ["epic", "Epic", "#6397F2", "#3D85E6"],
  ["rare", "Rare", "#3AD682", "#36CF75"],
  ["uncommon", "Uncommon", "#FF8563", "#ED6D4F"],
  ["common", "Common", "#D4E0E0", "#ABC1C1"],
];
const rarityOf = (r: string) => RARITIES.find(([k]) => k === r.toLowerCase());
const ALL = "all";
const TABS_PREFIX = "#rankings-";

type Filters = { sortBy: RankingsSortBy; category?: string; rarity?: string };

export function parseRankingsHash(hash: string): { entity: RankingEntity; timeframe: AnalyticsTimeframe; sortBy: RankingsSortBy } | null {
  if (!hash.includes(TABS_PREFIX)) return null;
  const [, entity, timeframe, ...rest] = hash.slice(hash.indexOf(TABS_PREFIX)).split("-");
  const sortBy = rest.join("-");
  const e = ENTITIES.includes(entity as RankingEntity) ? (entity as RankingEntity) : "wearables";
  const tf = (["day", "week", "month", "all"] as string[]).includes(timeframe ?? "") ? (timeframe as AnalyticsTimeframe) : "week";
  const s = SORTS.some((o) => o.key === sortBy) ? (sortBy as RankingsSortBy) : "most_volume";
  return { entity: e, timeframe: tf, sortBy: s };
}

const manaText = (wei: string) => {
  const v = parseManaWei(wei);
  return v === null ? "—" : formatMana(v, { group: true, decimals: 2 });
};

function Fiat({ wei }: { wei: string }) {
  const rate = useManaRate();
  if (!rate.data) return null;
  let cents: number;
  try {
    cents = manaWeiToUsdCents(wei, rate.data);
  } catch {
    return null;
  }
  if (!Number.isFinite(cents)) return null;
  return <span className="shop-rk__fiat">({(cents / 100).toLocaleString("en-US", { style: "currency", currency: "USD" })})</span>;
}

function Mana({ wei, network = "polygon", fiat = true }: { wei: string; network?: "polygon" | "ethereum"; fiat?: boolean }) {
  return (
    <span className="shop-rk__mana">
      <span className="shop-rk__mana-amount">
        <ManaMark size={14} network={network} />
        {manaText(wei)}
        <span className="u-sr-only"> MANA</span>
      </span>
      {fiat ? <Fiat wei={wei} /> : null}
    </span>
  );
}

function InfoTooltip({ content }: { content: string }) {
  return <img className="shop-rk__info" src={infoIcon} alt={content} title={content} width={14} height={14} />;
}

function TimeframeSelector({ value, onChange }: { value: AnalyticsTimeframe; onChange: (t: AnalyticsTimeframe) => void }) {
  return (
    <div className="shop-rk-timeframe" role="group" aria-label="Timeframe">
      {TIMEFRAMES.map((t) => (
        <button key={t.key} type="button" className={value === t.key ? "is-active" : undefined} aria-pressed={value === t.key} onClick={() => onChange(t.key)}>
          {t.label}
        </button>
      ))}
    </div>
  );
}

function Dropdown({ label, value, options, onChange, className }: { label: string; value: string; options: { value: string; text: string }[]; onChange: (v: string) => void; className?: string }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);
  const current = options.find((o) => o.value === value)?.text ?? options[0]?.text;
  return (
    <div ref={ref} className={`shop-rk-dropdown${className ? ` ${className}` : ""}${open ? " is-open" : ""}`}>
      <button type="button" className="shop-rk-dropdown__trigger" aria-haspopup="listbox" aria-expanded={open} aria-label={`${label}: ${current}`} onClick={() => setOpen((o) => !o)}>
        <span>{current}</span>
        <span className="shop-rk-dropdown__caret" aria-hidden="true" />
      </button>
      {open ? (
        <ul className="shop-rk-dropdown__menu" role="listbox" aria-label={label}>
          {options.map((o) => (
            <li key={o.value} role="option" aria-selected={o.value === value} className={o.value === value ? "is-active" : undefined} onClick={() => { onChange(o.value); setOpen(false); }}>
              {o.text}
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}

function useProfileName(address: string) {
  const q = useQuery({ queryKey: ["profile", address.toLowerCase()], queryFn: () => fetchProfile(address), retry: false, staleTime: 5 * 60_000 });
  return { name: q.data?.name || address.slice(0, 6), face: profileFaceUrl(q.data ?? null) };
}

function Profile({ address }: { address: string }) {
  const { name, face } = useProfileName(address);
  return (
    <Link className="shop-rk-profile" to={shopPath("account", { address })}>
      {face ? <img className="shop-rk-profile__face" src={face} alt="" width={45} height={45} /> : <Blockie seed={address} scale={7.5} className="shop-rk-profile__face" />}
      <span className="shop-rk-profile__name">{name}</span>
    </Link>
  );
}

function CreatorName({ address }: { address: string }) {
  const { name } = useProfileName(address);
  return (
    <Link className="shop-rk__creator" to={shopPath("account", { address })}>
      {name}
    </Link>
  );
}

function RarityBadge({ rarity }: { rarity: string }) {
  const r = rarityOf(rarity);
  return (
    <span className="shop-rk-rarity" style={{ background: r?.[2] ?? "#D4E0E0" }}>
      {r?.[1] ?? rarity}
    </span>
  );
}

function AssetImage({ item }: { item: FavoriteItem }) {
  const r = rarityOf(item.rarity);
  return (
    <span className="shop-rk-asset" style={r ? { backgroundImage: `radial-gradient(${r[2]}, ${r[3]})` } : undefined}>
      {item.thumbnail ? <img src={item.thumbnail} alt="" width={48} height={48} loading="lazy" decoding="async" /> : null}
    </span>
  );
}

function categoryText(item: RankedItem): string {
  if (item.wearableCategory) return WEARABLE_CATEGORIES[item.wearableCategory] ?? item.wearableCategory;
  if (item.emoteCategory) return EMOTE_CATEGORIES[item.emoteCategory] ?? item.emoteCategory;
  return "";
}

function Expand({ expanded, onToggle }: { expanded: boolean; onToggle: () => void }) {
  return (
    <button type="button" className="shop-rk-row__more" aria-expanded={expanded} onClick={onToggle}>
      <span>{expanded ? "Less" : "More"}</span>
      <i className={`shop-rk-row__caret${expanded ? " is-up" : ""}`} aria-hidden="true" />
    </button>
  );
}

type RankedItem = FavoriteItem & { wearableCategory?: string; emoteCategory?: string };
type ItemRow = { rank: ItemRank; item: RankedItem };

function dataCategory(data: Record<string, unknown> | undefined, key: "wearable" | "emote"): string | undefined {
  const d = data?.[key];
  const c = d && typeof d === "object" ? (d as { category?: unknown }).category : undefined;
  return typeof c === "string" && c ? c : undefined;
}

async function fetchRankedItems(ids: string[]): Promise<RankedItem[]> {
  if (ids.length === 0) return [];
  const rows = (await shopClient().catalog.items<{ data?: CatalogItem[] }>({ id: ids, first: ids.length })).data ?? [];
  return rows.map((row) => ({ ...toFavoriteItem(row), wearableCategory: dataCategory(row.data, "wearable"), emoteCategory: dataCategory(row.data, "emote") }));
}

function ItemRowView({ row, entity, mobile }: { row: ItemRow; entity: RankingEntity; mobile: boolean }) {
  const [expanded, setExpanded] = useState(false);
  const { item, rank } = row;
  const to = shopPath("itemDetail", { contractAddress: item.contractAddress, itemId: item.itemId });
  const network = item.chainId === 1 || item.chainId === 11155111 ? "ethereum" : "polygon";
  const name = (
    <span className="shop-rk-item__data">
      <Link className="shop-rk-item__name" to={to}>{item.name}</Link>
      <span className="shop-rk-item__by">
        by <CreatorName address={item.creator} />
      </span>
    </span>
  );
  if (mobile)
    return (
      <div className="shop-rk-mrow">
        <div className="shop-rk-mrow__main">
          <div className="shop-rk-item">
            <Link to={to} tabIndex={-1} aria-hidden="true"><AssetImage item={item} /></Link>
            <span className="shop-rk-item__data">
              <Link className="shop-rk-item__name" to={to}>{item.name}</Link>
              <span className="shop-rk-item__by">by <CreatorName address={item.creator} /></span>
              <span className="shop-rk-mrow__badge"><RarityBadge rarity={item.rarity} /></span>
            </span>
          </div>
          <div className="shop-rk-mrow__right">
            <Mana wei={rank.volume} network={network} />
            <Expand expanded={expanded} onToggle={() => setExpanded(!expanded)} />
          </div>
        </div>
        {expanded ? (
          <div className="shop-rk-mrow__more">
            <div><span>Category</span>{categoryText(item)}</div>
            <div><span>{entity === "emotes" ? "Emotes sold" : "Wearables sold"}</span>{rank.sales}</div>
          </div>
        ) : null}
      </div>
    );
  return (
    <tr>
      <td className="shop-rk-w7">
        <div className="shop-rk-item">
          <Link to={to} tabIndex={-1} aria-hidden="true"><AssetImage item={item} /></Link>
          {name}
        </div>
      </td>
      <td className="shop-rk-w2">{categoryText(item)}</td>
      <td className="shop-rk-w2"><RarityBadge rarity={item.rarity} /></td>
      <td className="shop-rk-w2">{rank.sales}</td>
      <td><Mana wei={rank.volume} network={network} /></td>
    </tr>
  );
}

function CreatorRowView({ entity, mobile }: { entity: CreatorRank; mobile: boolean }) {
  const [expanded, setExpanded] = useState(false);
  if (mobile)
    return (
      <div className="shop-rk-mrow">
        <div className="shop-rk-mrow__main">
          <div className="shop-rk-mrow__who">
            <Profile address={entity.id} />
            <span className="shop-rk-mrow__sub">{entity.collections} Collections</span>
          </div>
          <div className="shop-rk-mrow__right">
            <Mana wei={entity.earned} />
            <Expand expanded={expanded} onToggle={() => setExpanded(!expanded)} />
          </div>
        </div>
        {expanded ? (
          <div className="shop-rk-mrow__more">
            <div><span>Unique collectors</span>{entity.uniqueCollectors}</div>
            <div><span>Items sold</span>{entity.sales}</div>
          </div>
        ) : null}
      </div>
    );
  return (
    <tr>
      <td className="shop-rk-w5"><Profile address={entity.id} /></td>
      <td className="shop-rk-w3">{entity.collections}</td>
      <td className="shop-rk-w2">{entity.sales}</td>
      <td className="shop-rk-w3">{entity.uniqueCollectors}</td>
      <td><Mana wei={entity.earned} /></td>
    </tr>
  );
}

function CollectorRowView({ entity, mobile }: { entity: CollectorRank; mobile: boolean }) {
  const [expanded, setExpanded] = useState(false);
  if (mobile)
    return (
      <div className="shop-rk-mrow">
        <div className="shop-rk-mrow__main">
          <div className="shop-rk-mrow__who">
            <Profile address={entity.id} />
            <span className="shop-rk-mrow__sub">{entity.purchases} Collected items</span>
          </div>
          <div className="shop-rk-mrow__right">
            <Mana wei={entity.spent} />
            <Expand expanded={expanded} onToggle={() => setExpanded(!expanded)} />
          </div>
        </div>
        {expanded ? (
          <div className="shop-rk-mrow__more">
            <div><span>Mythic/Unique items Bought</span>{entity.uniqueAndMythicItems}</div>
            <div><span>Creators Supported</span>{entity.creatorsSupported}</div>
          </div>
        ) : null}
      </div>
    );
  return (
    <tr>
      <td className="shop-rk-w4"><Profile address={entity.id} /></td>
      <td className="shop-rk-w2">{entity.purchases}</td>
      <td className="shop-rk-w3">{entity.creatorsSupported}</td>
      <td className="shop-rk-w4">{entity.uniqueAndMythicItems}</td>
      <td><Mana wei={entity.spent} /></td>
    </tr>
  );
}

function Th({ children, tip }: { children: ReactNode; tip?: string }) {
  return (
    <th scope="col">
      <span className="shop-rk__th">
        {children}
        {tip ? <InfoTooltip content={tip} /> : null}
      </span>
    </th>
  );
}

function TableHeader({ entity }: { entity: RankingEntity }) {
  if (entity === "creators")
    return (
      <tr>
        <Th>Creator</Th>
        <Th>Collections</Th>
        <Th>Items sold</Th>
        <Th tip="Number of users who own at least 1 item from the Creator.">Unique collectors</Th>
        <Th tip="Includes only primary sales">Total Volume of Sales</Th>
      </tr>
    );
  if (entity === "collectors")
    return (
      <tr>
        <Th>Collector</Th>
        <Th>Items Bought</Th>
        <Th tip="Number of Creators the user bought at least 1 item from.">Creators Supported</Th>
        <Th tip="Number of Mythic or Unique items the user has bought.">Mythic/Unique items Bought</Th>
        <Th tip="Total amount of MANA spent in both primary and secondary markets.">Total Spent</Th>
      </tr>
    );
  const label = entity === "emotes" ? "Emote" : "Wearable";
  return (
    <tr>
      <Th>{label}</Th>
      <Th>Category</Th>
      <Th>Rarity</Th>
      <Th>{label}s sold</Th>
      <Th tip="Includes both primary and secondary sales.">Total Volume</Th>
    </tr>
  );
}

type RankingsData = { kind: "items"; rows: ItemRow[] } | { kind: "creators"; rows: CreatorRank[] } | { kind: "collectors"; rows: CollectorRank[] };

async function loadRankings(entity: RankingEntity, timeframe: AnalyticsTimeframe, filters: Filters): Promise<RankingsData> {
  const rows = await fetchRankings(entity, timeframe, { ...filters, first: 5 });
  if (entity === "creators") return { kind: "creators", rows: rows as CreatorRank[] };
  if (entity === "collectors") return { kind: "collectors", rows: rows as CollectorRank[] };
  const items = (rows as ItemRank[]).filter((r) => /^\d+$/.test(r.id.split("-")[1] ?? ""));
  const catalog = await fetchRankedItems(items.map((r) => r.id)).catch(() => [] as RankedItem[]);
  const byKey = new Map(catalog.map((c) => [c.key, c]));
  return { kind: "items", rows: items.flatMap((rank) => { const item = byKey.get(rank.id.toLowerCase()); return item ? [{ rank, item }] : []; }) };
}

export default function RankingsTable() {
  const location = useLocation();
  const navigate = useNavigate();
  const mobile = useMediaQuery("(max-width: 767px)");
  const [entity, setEntity] = useState<RankingEntity>("wearables");
  const [timeframe, setTimeframe] = useState<AnalyticsTimeframe>("week");
  const [filters, setFilters] = useState<Filters>({ sortBy: "most_volume" });
  const sectionRef = useRef<HTMLElement>(null);

  useEffect(() => {
    const parsed = parseRankingsHash(location.hash);
    if (!parsed) return;
    setEntity(parsed.entity);
    setTimeframe(parsed.timeframe);
    setFilters({ sortBy: parsed.sortBy });
    sectionRef.current?.scrollIntoView?.({ behavior: "smooth" });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const query = useQuery({
    queryKey: ["rankings", entity, timeframe, filters.sortBy, filters.category ?? ALL, filters.rarity ?? ALL],
    queryFn: () => loadRankings(entity, timeframe, filters),
  });

  const setHash = (e: RankingEntity, tf: AnalyticsTimeframe, sortBy: RankingsSortBy) =>
    navigate({ pathname: location.pathname, search: location.search, hash: `${TABS_PREFIX}${e}-${tf}-${sortBy}` }, { replace: true, preventScrollReset: true });

  const onTab = (e: RankingEntity) => {
    setEntity(e);
    setFilters({ sortBy: filters.sortBy });
    setHash(e, timeframe, filters.sortBy);
  };
  const onTimeframe = (tf: AnalyticsTimeframe) => {
    setTimeframe(tf);
    setHash(entity, tf, filters.sortBy);
  };
  const onSort = (v: string) => {
    const sortBy = v as RankingsSortBy;
    setFilters({ ...filters, sortBy });
    setHash(entity, timeframe, sortBy);
  };
  const onFilter = (name: "category" | "rarity") => (v: string) => setFilters({ ...filters, [name]: v === ALL ? undefined : v });

  const isItems = entity === "wearables" || entity === "emotes";
  const categories = entity === "emotes" ? EMOTE_CATEGORIES : WEARABLE_CATEGORIES;

  const content = () => {
    if (query.isPending) return <div className="shop-rk-card__state"><Loading label="Loading rankings…" /></div>;
    if (query.isError)
      return <div className="shop-rk-card__state"><LoadFailure error={query.error} what="Rankings" onRetry={() => void query.refetch()} /></div>;
    const data = query.data;
    if (data.rows.length === 0)
      return (
        <div className="shop-rk-empty" data-testid="shop-empty">
          <span className="shop-rk-empty__title">No results found for these filters</span>
          <span className="shop-rk-empty__sub">
            <button type="button" className="shop-rk-empty__action" onClick={() => setFilters({ sortBy: "most_volume" })}>Clear filters</button> or select different ones
          </span>
        </div>
      );
    const rows =
      data.kind === "items"
        ? data.rows.map((r) => <ItemRowView key={r.rank.id} row={r} entity={entity} mobile={mobile} />)
        : data.kind === "creators"
          ? data.rows.map((r) => <CreatorRowView key={r.id} entity={r} mobile={mobile} />)
          : data.rows.map((r) => <CollectorRowView key={r.id} entity={r} mobile={mobile} />);
    if (mobile)
      return (
        <div className="shop-rk-mlist">
          <div className="shop-rk-mlist__head">
            <span>{entity === "creators" ? "Creator" : entity === "collectors" ? "Collector" : entity === "emotes" ? "Emote" : "Wearable"}</span>
            <span>Total Volume</span>
          </div>
          {rows}
        </div>
      );
    return (
      <table className="shop-rk-table">
        <thead>
          <TableHeader entity={entity} />
        </thead>
        <tbody>{rows}</tbody>
      </table>
    );
  };

  return (
    <section className="shop-rk-table-section" ref={sectionRef} aria-labelledby="shop-rk-title">
      <header className="shop-rk-head">
        <div className="shop-rk-head__left">
          <h1 id="shop-rk-title" className="shop-rk-head__title">Rankings</h1>
          <span className="shop-rk-head__sub">Includes only Wearables and Emotes</span>
        </div>
        <div className="shop-rk-head__right">
          <Dropdown className="shop-rk-dropdown--sort" label="Sort by" value={filters.sortBy} options={SORTS.map((s) => ({ value: s.key, text: s.label }))} onChange={onSort} />
          <TimeframeSelector value={timeframe} onChange={onTimeframe} />
        </div>
      </header>
      <div className="shop-rk-card">
        {mobile ? (
          <Dropdown className="shop-rk-dropdown--entity" label="Ranking" value={entity} options={ENTITIES.map((e) => ({ value: e, text: ENTITY_TITLE[e] }))} onChange={(v) => onTab(v as RankingEntity)} />
        ) : (
          <div className="shop-rk-card__tabs">
            <div className="shop-rk-tabs" role="group" aria-label="Ranking">
              {ENTITIES.map((e) => (
                <button key={e} type="button" className={`shop-rk-tab${entity === e ? " is-active" : ""}`} aria-pressed={entity === e} onClick={() => onTab(e)}>
                  {ENTITY_TITLE[e]}
                </button>
              ))}
            </div>
            {isItems ? (
              <>
                <Dropdown
                  label="Category"
                  value={filters.category ?? ALL}
                  options={[{ value: ALL, text: "All categories" }, ...Object.entries(categories).map(([value, text]) => ({ value, text }))]}
                  onChange={onFilter("category")}
                />
                <Dropdown
                  label="Rarity"
                  value={filters.rarity ?? ALL}
                  options={[{ value: ALL, text: "All rarities" }, ...RARITIES.map(([value, text]) => ({ value, text }))]}
                  onChange={onFilter("rarity")}
                />
              </>
            ) : null}
          </div>
        )}
        {content()}
      </div>
    </section>
  );
}
