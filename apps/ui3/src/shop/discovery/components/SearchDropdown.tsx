// from: shop@94584a5c:app/src/components/SearchDropdown/SearchDropdown.tsx (behaviour);
//       shop@3a322cf9:app/src/components/SearchDropdown/SearchDropdown.tsx + .styles.ts (the panel: popular searches
//       as chips, recent rows with their removals beside the listbox, 40px thumbs, "By {creator}" sublines, the
//       collection mosaic, bold matches, the pinned See all)
// The Shop's search box as an ARIA combobox: recent and popular searches while empty; while typing,
// a category/rarity facet when the whole query names one, then items, collections and creators from
// one /v3/catalog/suggest request (debounced, the stale one aborted) and "See all (N)". Enter with no
// active row runs the search on the grid; a repeat of the same destination replaces the history entry.

import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useLocation, useNavigate, useSearchParams } from "react-router";

import { ShopSearch, type ShopSearchRow } from "@dcl/one-ui";
import { SHOP_BASE } from "../../routes";
import { EMPTY_SUGGESTIONS, fetchSuggestions, type Suggestions } from "@shop/discovery/api";
import { t } from "@shop/discovery/intl";
import { highlightMatches } from "../lib/highlight";
import { popularSearchesFor } from "../lib/popularSearches";
import { clearRecentSearches, getRecentSearches, recordSearch, removeRecentSearch } from "../lib/recent-searches";
import { detailRouteFor } from "../lib/routes";
import { clearedSearchUrl } from "../lib/searchClear";
import { facetId, facetsFor, facetUrl, type Facet } from "../lib/searchFacets";
import { searchHistoryMode } from "../lib/searchHistory";
import { NO_ACTIVE_ROW, searchKeyAction, suggestionRowId } from "../lib/suggestionNavigation";
import { shortAddress } from "@shop/model/address";
import { rarityColor, rarityLabel } from "@shop/discovery/lib/rarity";
import { CollectionThumb } from "./CollectionCard";
import { subCategoryLabel } from "./Filters";
import Icon from "./Icon";
import "./search.css";
import { shopReads } from "../reads";

const DEBOUNCE_MS = 200;

function Highlighted({ text, query }: { text: string; query: string }) {
  return <>{highlightMatches(text, query).map((seg, i) => (seg.match ? <mark key={i}>{seg.text}</mark> : <span key={i}>{seg.text}</span>))}</>;
}

/** "By {creator}": the name the suggestion carries, else the short address (no profile fetched per row). */
function byCreator(address: string | null | undefined, name: string | null | undefined): string | undefined {
  if (!address && !name) return undefined;
  return t("search.byCreator", { name: name || shortAddress(address ?? "") });
}

function facetLabel(f: Facet): string {
  if (f.kind === "rarity") return rarityLabel(f.key);
  return subCategoryLabel(f.key) === f.key ? t(f.labelKey) : subCategoryLabel(f.key);
}

/** Where a facet sits: "Wearables · Accessories" for a category, "Rarity" for a rarity. */
function facetPath(f: Facet): string {
  return f.kind === "category" ? f.parents.map((key) => t(key)).join(" · ") : t("assets.rarity");
}

function IconThumb({ children }: { children: ReactNode }) {
  return (
    <span className="shop-search__thumb" data-variant="icon">
      {children}
    </span>
  );
}

function FacetThumb({ facet }: { facet: Facet }) {
  return (
    <IconThumb>{facet.kind === "rarity" ? <span className="shop-search__dot" style={{ background: rarityColor(facet.key) }} /> : <Icon name={facet.icon ?? "search"} size={18} />}</IconThumb>
  );
}

export default function SearchDropdown({ debounceMs = DEBOUNCE_MS }: { debounceMs?: number }) {
  const navigate = useNavigate();
  const location = useLocation();
  const [params] = useSearchParams();
  const [value, setValue] = useState(() => params.get("q") ?? "");
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(NO_ACTIVE_ROW);
  const [recent, setRecent] = useState<string[]>(() => getRecentSearches());
  const [state, setState] = useState<{ query: string; data: Suggestions; loading: boolean; error: boolean }>({ query: "", data: EMPTY_SUGGESTIONS, loading: false, error: false });
  const [retry, setRetry] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const query = value.trim();

  useEffect(() => {
    setValue(params.get("q") ?? "");
  }, [params]);

  useEffect(() => {
    if (!query) {
      setState({ query: "", data: EMPTY_SUGGESTIONS, loading: false, error: false });
      return;
    }
    const controller = new AbortController();
    setState((s) => ({ ...s, loading: true, error: false }));
    const timer = setTimeout(() => {
      fetchSuggestions(query, {}, shopReads(controller.signal)).then(
        (data) => !controller.signal.aborted && setState({ query, data, loading: false, error: false }),
        (e) => !controller.signal.aborted && (e as { name?: string })?.name !== "AbortError" && setState({ query, data: EMPTY_SUGGESTIONS, loading: false, error: true }),
      );
    }, debounceMs);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [query, retry, debounceMs]);

  const go = useCallback(
    (target: string) => {
      setOpen(false);
      setActive(NO_ACTIVE_ROW);
      navigate(target, { replace: searchHistoryMode(location, target) === "replace" });
    },
    [navigate, location],
  );

  const submit = useCallback(
    (term: string) => {
      const q = term.trim();
      if (!q) return;
      recordSearch(q);
      setRecent(getRecentSearches());
      const next = new URLSearchParams(location.pathname === `${SHOP_BASE}/items` ? location.search : "");
      next.set("q", q);
      go(`${SHOP_BASE}/items?${next.toString()}`);
    },
    [go, location],
  );

  const facets = useMemo(() => (query ? facetsFor(query) : []), [query]);
  const data = state.query === query ? state.data : EMPTY_SUGGESTIONS;

  const rows: ShopSearchRow[] = useMemo(() => {
    if (!query) {
      const recents = recent.map((r) => ({ id: suggestionRowId("recent", r), kind: "recent" as const, section: "recent", label: r, activate: () => submit(r) }));
      const popular = popularSearchesFor(recent).map((p) => ({ id: suggestionRowId("popular", p), kind: "popular" as const, section: "popular", label: p, activate: () => submit(p) }));
      return [...recents, ...popular];
    }
    const out: ShopSearchRow[] = [];
    for (const f of facets) {
      out.push({
        id: suggestionRowId("facet", facetId(f)),
        kind: "facet",
        section: "facets",
        label: facetLabel(f),
        title: facetLabel(f),
        sub: facetPath(f) || undefined,
        thumb: <FacetThumb facet={f} />,
        activate: () => go(facetUrl(f)),
      });
    }
    for (const item of data.items) {
      const path = detailRouteFor(item);
      out.push({
        id: suggestionRowId("item", item.id),
        kind: "item",
        section: "items",
        label: <Highlighted text={item.name} query={query} />,
        title: item.name,
        sub: byCreator(item.creator, item.creatorName),
        thumb: <span className="shop-search__thumb">{item.thumbnail ? <img src={item.thumbnail} alt="" /> : null}</span>,
        activate: () => path && go(path),
      });
    }
    for (const col of data.collections) {
      out.push({
        id: suggestionRowId("collection", col.contractAddress),
        kind: "collection",
        section: "collections",
        label: <Highlighted text={col.name} query={query} />,
        title: col.name,
        sub: byCreator(col.creator, col.creatorName),
        thumb: (
          <CollectionThumb
            contractAddress={col.contractAddress}
            className="shop-search__coll"
            fallback={
              <IconThumb>
                <Icon name="search" size={18} />
              </IconThumb>
            }
          />
        ),
        activate: () => go(`${SHOP_BASE}/collection/${col.contractAddress}`),
      });
    }
    for (const cr of data.creators) {
      out.push({
        id: suggestionRowId("creator", cr.address),
        kind: "creator",
        section: "creators",
        label: <Highlighted text={cr.name} query={query} />,
        title: cr.name,
        thumb: (
          <span className="shop-search__thumb" data-variant="round">
            {cr.face ? <img src={cr.face} alt="" /> : null}
          </span>
        ),
        activate: () => go(`${SHOP_BASE}/items/creator/${cr.address}`),
      });
    }
    if (data.total > 0)
      out.push({ id: suggestionRowId("see-all", query), kind: "see-all", section: "see-all", label: t("search.seeAll", { count: data.total.toLocaleString() }), activate: () => submit(query) });
    return out;
  }, [query, recent, facets, data, go, submit]);

  useEffect(() => setActive(NO_ACTIVE_ROW), [query]);

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    const action = searchKeyAction(
      { key: e.key, altKey: e.altKey, ctrlKey: e.ctrlKey, metaKey: e.metaKey, shiftKey: e.shiftKey, isComposing: e.nativeEvent.isComposing },
      { open, activeIndex: active, count: rows.length },
    );
    if (!action) return;
    e.preventDefault();
    if (action.type === "move") {
      setOpen(true);
      setActive(action.index);
    } else if (action.type === "activate") rows[active]?.activate("keyboard");
    else if (action.type === "submit") submit(value);
    else if (action.type === "close") setOpen(false);
    else if (action.type === "clear") clear();
  };

  const clear = () => {
    setValue("");
    const target = clearedSearchUrl(location.pathname, location.search);
    if (target) navigate(target, { replace: true });
    inputRef.current?.focus();
  };

  const empty = !!query && !state.loading && !state.error && state.query === query && rows.length === facets.length && data.items.length === 0;

  return (
    <ShopSearch
      inputRef={inputRef}
      value={value}
      open={open}
      active={active}
      recent={recent}
      rows={rows}
      loading={Boolean(state.loading && query && data.items.length === 0)}
      failed={Boolean(state.error && query)}
      empty={empty}
      onOpenChange={setOpen}
      onChange={setValue}
      onSubmit={submit}
      onKeyDown={onKeyDown}
      onClear={clear}
      onRetry={() => setRetry((n) => n + 1)}
      onClearRecent={() => {
        clearRecentSearches();
        setRecent([]);
      }}
      onRemoveRecent={(term) => {
        removeRecentSearch(term);
        setRecent(getRecentSearches());
      }}
    />
  );
}
