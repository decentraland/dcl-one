// from: shop@94584a5c:app/src/components/SearchDropdown/SearchDropdown.tsx (behaviour);
//       shop@3a322cf9:app/src/components/SearchDropdown/SearchDropdown.tsx + .styles.ts (the panel: popular searches
//       as chips, recent rows with their removals beside the listbox, 40px thumbs, "By {creator}" sublines, the
//       collection mosaic, bold matches, the pinned See all)
// The Shop's search box as an ARIA combobox: recent and popular searches while empty; while typing,
// a category/rarity facet when the whole query names one, then items, collections and creators from
// one /v3/catalog/suggest request (debounced, the stale one aborted) and "See all (N)". Enter with no
// active row runs the search on the grid; a repeat of the same destination replaces the history entry.

import { useCallback, useEffect, useId, useMemo, useRef, useState, type ReactNode } from "react";
import { useLocation, useNavigate, useSearchParams } from "react-router";

import { SHOP_BASE } from "../../routes";
import { EMPTY_SUGGESTIONS, fetchSuggestions, type Suggestions } from "../api";
import { t } from "../intl";
import { highlightMatches } from "../lib/highlight";
import { popularSearchesFor } from "../lib/popularSearches";
import { clearRecentSearches, getRecentSearches, recordSearch, removeRecentSearch } from "../lib/recent-searches";
import { detailRouteFor } from "../lib/routes";
import { clearedSearchUrl } from "../lib/searchClear";
import { facetId, facetsFor, facetUrl, type Facet } from "../lib/searchFacets";
import { searchHistoryMode } from "../lib/searchHistory";
import { NO_ACTIVE_ROW, searchKeyAction, suggestionRowId, type SuggestionRow } from "../lib/suggestionNavigation";
import { shortAddress } from "../../model/address";
import { rarityColor, rarityLabel } from "../lib/rarity";
import { CollectionThumb } from "./CollectionCard";
import { subCategoryLabel } from "./Filters";
import Icon from "./Icon";
import "./search.css";
import { shopClient } from "../../api/host";

const DEBOUNCE_MS = 200;

function Highlighted({ text, query }: { text: string; query: string }) {
  return (
    <>
      {highlightMatches(text, query).map((seg, i) => (seg.match ? <mark key={i}>{seg.text}</mark> : <span key={i}>{seg.text}</span>))}
    </>
  );
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
  return <IconThumb>{facet.kind === "rarity" ? <span className="shop-search__dot" style={{ background: rarityColor(facet.key) }} /> : <Icon name={facet.icon ?? "search"} size={18} />}</IconThumb>;
}

type Row = SuggestionRow & { section: string; label: ReactNode; title?: string; sub?: string; thumb?: ReactNode };

export default function SearchDropdown({ debounceMs = DEBOUNCE_MS }: { debounceMs?: number }) {
  const navigate = useNavigate();
  const location = useLocation();
  const [params] = useSearchParams();
  const listboxId = useId();
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
      fetchSuggestions(query, {}, { client: shopClient(), signal: controller.signal }).then(
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

  const rows: Row[] = useMemo(() => {
    if (!query) {
      const recents = recent.map((r) => ({ id: suggestionRowId("recent", r), kind: "recent" as const, section: "recent", label: r, activate: () => submit(r) }));
      const popular = popularSearchesFor(recent).map((p) => ({ id: suggestionRowId("popular", p), kind: "popular" as const, section: "popular", label: p, activate: () => submit(p) }));
      return [...recents, ...popular];
    }
    const out: Row[] = [];
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
    if (data.total > 0) out.push({ id: suggestionRowId("see-all", query), kind: "see-all", section: "see-all", label: t("search.seeAll", { count: data.total.toLocaleString() }), activate: () => submit(query) });
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

  const sections: [string, string][] = query
    ? [
        ["facets", t("search.explore")],
        ["items", t("search.items")],
        ["collections", t("search.collections")],
        ["creators", t("search.creators")],
        ["see-all", ""],
      ]
    : [
        ["recent", t("search.recent")],
        ["popular", t("search.popular")],
      ];
  const empty = !!query && !state.loading && !state.error && state.query === query && rows.length === facets.length && data.items.length === 0;

  return (
    <div className="shop-search" data-testid="search" onBlur={(e) => !e.currentTarget.contains(e.relatedTarget as Node) && setOpen(false)}>
      <form
        role="search"
        onSubmit={(e) => {
          e.preventDefault();
          submit(value);
        }}
      >
        <Icon name="search" size={20} className="shop-search__icon" />
        <input
          ref={inputRef}
          type="search"
          role="combobox"
          aria-label={t("search.label")}
          aria-expanded={open}
          aria-controls={listboxId}
          aria-autocomplete="list"
          aria-activedescendant={active >= 0 ? rows[active]?.id : undefined}
          placeholder={t("search.placeholder")}
          autoComplete="off"
          spellCheck={false}
          value={value}
          onChange={(e) => {
            setValue(e.target.value);
            setOpen(true);
          }}
          onFocus={() => setOpen(true)}
          onKeyDown={onKeyDown}
          data-testid="search-input"
        />
        {value ? (
          <button type="button" className="shop-search__clear" onClick={clear} aria-label={t("search.clear")}>
            <Icon name="close" size={16} />
          </button>
        ) : null}
      </form>
      {open ? (
        <div className="shop-search__panel" data-testid="search-panel" onMouseDown={(e) => (e.target as HTMLElement).closest("button") || e.preventDefault()}>
          {state.loading && query && data.items.length === 0 ? (
            <p className="shop-search__empty" role="status">
              {t("search.searching")}
            </p>
          ) : null}
          {state.error && query ? (
            <p className="shop-search__empty" role="alert">
              {t("search.error")}{" "}
              <button type="button" className="shop-search__link" onClick={() => setRetry((n) => n + 1)}>
                {t("search.retry")}
              </button>
            </p>
          ) : null}
          {empty ? (
            <p className="shop-search__empty" role="status">
              {t("search.noEntities", { query })}
            </p>
          ) : null}
          {!query && recent.length > 0 ? (
            <div className="shop-search__head">
              <span>{t("search.recent")}</span>
              <button
                type="button"
                className="shop-search__link"
                onClick={() => {
                  clearRecentSearches();
                  setRecent([]);
                }}
              >
                {t("search.clearRecent")}
              </button>
            </div>
          ) : null}
          <div className={query ? undefined : "shop-search__recentarea"}>
            <div id={listboxId} role="listbox" aria-label={t("search.suggestions")}>
              {sections.map(([section, heading]) => {
                const inSection = rows.map((r, i) => [r, i] as const).filter(([r]) => r.section === section);
                if (inSection.length === 0) return null;
                const option = (row: Row, i: number) => ({
                  id: row.id,
                  role: "option" as const,
                  "aria-selected": i === active,
                  "data-active": i === active || undefined,
                  "data-kind": row.kind,
                  onClick: () => row.activate("click"),
                });
                if (section === "recent") {
                  return (
                    <div key={section} role="group" aria-label={heading}>
                      {inSection.map(([row, i]) => (
                        <div key={row.id} className="shop-search__recent" {...option(row, i)}>
                          <Icon name="search" size={16} className="shop-search__recenticon" />
                          <span className="shop-search__recenttext">{row.label}</span>
                        </div>
                      ))}
                    </div>
                  );
                }
                if (section === "popular") {
                  return (
                    <div key={section} role="group" aria-label={heading}>
                      <div className="shop-search__head" role="presentation">
                        <span>{heading}</span>
                      </div>
                      <div className="shop-search__chips" role="none">
                        {inSection.map(([row, i]) => (
                          <span key={row.id} className="shop-search__chip" {...option(row, i)}>
                            {row.label}
                          </span>
                        ))}
                      </div>
                    </div>
                  );
                }
                if (section === "see-all") {
                  return inSection.map(([row, i]) => (
                    <div key={row.id} className="shop-search__seeall" {...option(row, i)}>
                      {row.label}
                    </div>
                  ));
                }
                return (
                  <div key={section} role="group" aria-label={heading}>
                    <div className="shop-search__head" role="presentation">
                      <span>{heading}</span>
                    </div>
                    {inSection.map(([row, i]) => (
                      <div key={row.id} className="shop-search__row" {...option(row, i)}>
                        {row.thumb}
                        <span className="shop-search__text">
                          <span className="shop-search__name" title={row.title}>
                            {row.label}
                          </span>
                          {row.sub ? <span className="shop-search__sub">{row.sub}</span> : null}
                        </span>
                      </div>
                    ))}
                  </div>
                );
              })}
            </div>
            {/* The removals are controls, not options: beside the listbox, one per recent row, in the tab order. */}
            {!query && recent.length > 0 ? (
              <ul className="shop-search__removes" aria-label={t("search.recent")}>
                {recent.map((term) => (
                  <li key={term}>
                    <button
                      type="button"
                      className="shop-search__remove"
                      aria-label={t("search.removeRecent", { query: term })}
                      onClick={() => {
                        removeRecentSearch(term);
                        setRecent(getRecentSearches());
                      }}
                    >
                      <Icon name="close" size={14} />
                    </button>
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
        </div>
      ) : null}
    </div>
  );
}
