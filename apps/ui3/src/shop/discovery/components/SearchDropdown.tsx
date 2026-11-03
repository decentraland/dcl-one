// from: shop@94584a5c:app/src/components/SearchDropdown/SearchDropdown.tsx
// The Shop's search box as an ARIA combobox: recent and popular searches while empty; while typing,
// a category/rarity facet when the whole query names one, then items, collections and creators from
// one /v3/catalog/suggest request (debounced, the stale one aborted) and "See all (N)". Enter with no
// active row runs the search on the grid; a repeat of the same destination replaces the history entry.

import { useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import { Link, useLocation, useNavigate, useSearchParams } from "react-router";

import { SHOP_BASE } from "../../routes";
import { EMPTY_SUGGESTIONS, fetchSuggestions, type Suggestions } from "../api";
import { t } from "../intl";
import { highlightMatches } from "../lib/highlight";
import { popularSearchesFor } from "../lib/popularSearches";
import { clearRecentSearches, getRecentSearches, recordSearch, removeRecentSearch } from "../lib/recent-searches";
import { detailRouteFor } from "../lib/routes";
import { clearedSearchUrl } from "../lib/searchClear";
import { facetId, facetsFor, facetUrl } from "../lib/searchFacets";
import { searchHistoryMode } from "../lib/searchHistory";
import { NO_ACTIVE_ROW, searchKeyAction, suggestionRowId, type SuggestionRow } from "../lib/suggestionNavigation";
import { rarityLabel } from "../lib/rarity";
import { subCategoryLabel } from "./Filters";
import Icon from "./Icon";
import "./search.css";

const DEBOUNCE_MS = 200;

function Highlighted({ text, query }: { text: string; query: string }) {
  return (
    <>
      {highlightMatches(text, query).map((seg, i) => (seg.match ? <mark key={i}>{seg.text}</mark> : <span key={i}>{seg.text}</span>))}
    </>
  );
}

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
      fetchSuggestions(query, {}, { signal: controller.signal }).then(
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

  const rows: (SuggestionRow & { label: React.ReactNode; sub?: string; section: string; thumb?: string })[] = useMemo(() => {
    if (!query) {
      const recents = recent.map((r) => ({ id: suggestionRowId("recent", r), kind: "recent" as const, section: "recent", label: r, activate: () => submit(r) }));
      const popular = popularSearchesFor(recent).map((p) => ({ id: suggestionRowId("popular", p), kind: "popular" as const, section: "popular", label: p, activate: () => submit(p) }));
      return [...recents, ...popular];
    }
    const out: (SuggestionRow & { label: React.ReactNode; sub?: string; section: string; thumb?: string })[] = [];
    for (const f of facets) {
      out.push({
        id: suggestionRowId("facet", facetId(f)),
        kind: "facet",
        section: "facets",
        label: f.kind === "rarity" ? rarityLabel(f.key) : subCategoryLabel(f.key) === f.key ? t(f.labelKey) : subCategoryLabel(f.key),
        sub: t("search.explore"),
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
        sub: item.creatorName ? t("search.byCreator", { name: item.creatorName }) : undefined,
        thumb: item.thumbnail,
        activate: () => path && go(path),
      });
    }
    for (const col of data.collections) {
      out.push({
        id: suggestionRowId("collection", col.contractAddress),
        kind: "collection",
        section: "collections",
        label: <Highlighted text={col.name} query={query} />,
        sub: col.creatorName ? t("search.byCreator", { name: col.creatorName }) : undefined,
        activate: () => go(`${SHOP_BASE}/collection/${col.contractAddress}`),
      });
    }
    for (const cr of data.creators) {
      out.push({
        id: suggestionRowId("creator", cr.address),
        kind: "creator",
        section: "creators",
        label: <Highlighted text={cr.name} query={query} />,
        thumb: cr.face,
        activate: () => go(`${SHOP_BASE}/items/creator/${cr.address}`),
      });
    }
    if (data.total > 0) out.push({ id: suggestionRowId("see-all", query), kind: "see-all", section: "see-all", label: t("search.seeAll", { count: data.total }), activate: () => submit(query) });
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
        <div className="shop-search__panel" data-testid="search-panel">
          {state.loading && query ? (
            <p className="shop-search__status" role="status">
              {t("search.searching")}
            </p>
          ) : null}
          {state.error && query ? (
            <p className="shop-search__status" role="alert">
              {t("search.error")}{" "}
              <button type="button" onClick={() => setRetry((n) => n + 1)}>
                {t("search.retry")}
              </button>
            </p>
          ) : null}
          {empty ? (
            <p className="shop-search__status" role="status">
              {t("search.noEntities", { query })}
            </p>
          ) : null}
          <ul id={listboxId} role="listbox" aria-label={t("search.suggestions")}>
            {sections.map(([section, heading]) => {
              const inSection = rows.map((r, i) => [r, i] as const).filter(([r]) => r.section === section);
              if (inSection.length === 0) return null;
              return (
                <li key={section} role="presentation">
                  {heading ? (
                    <div className="shop-search__heading" role="presentation">
                      <span>{heading}</span>
                      {section === "recent" ? (
                        <button
                          type="button"
                          onClick={() => {
                            clearRecentSearches();
                            setRecent([]);
                          }}
                        >
                          {t("search.clearRecent")}
                        </button>
                      ) : null}
                    </div>
                  ) : null}
                  <ul role="group" aria-label={heading || undefined}>
                    {inSection.map(([row, i]) => (
                      <li
                        key={row.id}
                        id={row.id}
                        role="option"
                        aria-selected={i === active}
                        className="shop-search__row"
                        data-kind={row.kind}
                        onMouseDown={(e) => e.preventDefault()}
                        onClick={() => row.activate("click")}
                      >
                        {row.thumb ? <img src={row.thumb} alt="" width={32} height={32} /> : null}
                        <span className="shop-search__label">{row.label}</span>
                        {row.sub ? <span className="shop-search__sub">{row.sub}</span> : null}
                        {row.kind === "recent" ? (
                          <button
                            type="button"
                            className="shop-search__remove"
                            aria-label={t("search.removeRecent", { query: String(row.label) })}
                            onClick={(e) => {
                              e.stopPropagation();
                              removeRecentSearch(String(row.label));
                              setRecent(getRecentSearches());
                            }}
                          >
                            <Icon name="close" size={14} />
                          </button>
                        ) : null}
                      </li>
                    ))}
                  </ul>
                </li>
              );
            })}
          </ul>
          {query && data.total > 0 ? null : query ? null : (
            <Link className="shop-search__browse" to={`${SHOP_BASE}/items`} onClick={() => setOpen(false)}>
              {t("assets.empty.cta")}
            </Link>
          )}
        </div>
      ) : null}
    </div>
  );
}
