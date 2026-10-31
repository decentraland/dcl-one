// from: shop@2d3bf62:app/src/components/FilterBar/FilterBar.tsx (+ FilterBar.styles.ts),
//       app/src/components/Dropdown/Dropdown.tsx (+ Dropdown.styles.ts), app/src/components/Chevron

import { useEffect, useRef, useState } from "react";

import { t } from "../intl";
import type { ShopSort } from "../api";
import { RARITIES } from "../lib/rarity";
import Icon from "./Icon";
import "./filterbar.css";

export { RARITIES };

export type SortOption = { key: string; label: string; server: ShopSort };

export const SORTS: SortOption[] = [
  { key: "newest", label: "filterBar.sortNewest", server: "newest" },
  { key: "price-asc", label: "filterBar.sortCheapest", server: "cheapest" },
  { key: "price-desc", label: "filterBar.sortMostExpensive", server: "most_expensive" },
  { key: "name", label: "filterBar.sortName", server: "name" },
];

export const RELEVANCE_SORT: SortOption = { key: "relevance", label: "filterBar.sortRelevance", server: "relevance" };

export const DEALS_SORTS: SortOption[] = [{ key: "discount", label: "filterBar.sortBiggestDiscount", server: "discount" }, ...SORTS];

export type FilterChip = { key: string; label: string; onRemove: () => void };

export default function FilterBar({
  sort,
  onSort,
  sortOptions = SORTS,
  total,
  loading,
  query,
  onOpenFilters,
  chips,
  onClearChips,
}: {
  sort: string;
  onSort: (key: string) => void;
  sortOptions?: SortOption[];
  total: number | null;
  loading: boolean;
  query?: string;
  onOpenFilters?: () => void;
  chips?: FilterChip[];
  onClearChips?: () => void;
}) {
  return (
    <div className="shop-filterbar" data-testid="browse-toolbar">
      <p className="shop-filterbar__count" data-testid="browse-count" aria-busy={loading || undefined} aria-live="polite">
        {loading ? (
          <span className="shop-filterbar__shimmer" aria-hidden data-testid="browse-count-skeleton" />
        ) : (
          <>
            {total === null ? t("filterBar.countUnavailable") : t("filterBar.count", { count: total })}
            {query ? ` ${t("filterBar.forQuery", { query })}` : ""}
          </>
        )}
      </p>
      {chips && chips.length ? (
        <div className="shop-filterbar__chips" data-testid="filter-chips">
          {chips.map((c) => (
            <button key={c.key} type="button" className="shop-filterbar__chip" onClick={c.onRemove} aria-label={t("filterBar.removeFilter", { filter: c.label })}>
              <span>{c.label}</span>
              <Icon name="close" size={14} />
            </button>
          ))}
          {onClearChips ? (
            <button type="button" className="shop-filterbar__clear" onClick={onClearChips}>
              {t("filterBar.clearAll")}
            </button>
          ) : null}
        </div>
      ) : null}
      <div className="shop-filterbar__right">
        <SortDropdown value={sort} options={sortOptions} onChange={onSort} />
        {onOpenFilters ? (
          <button type="button" className="shop-filterbar__pill" data-testid="open-filters" onClick={onOpenFilters}>
            {t("filterBar.filters")}
            <Icon name="filter" size={20} className="shop-filterbar__pillicon" />
          </button>
        ) : null}
      </div>
    </div>
  );
}

export function SortDropdown({ value, options, onChange }: { value: string; options: readonly { key: string; label: string }[]; onChange: (key: string) => void }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const onPointer = (e: PointerEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("pointerdown", onPointer);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("pointerdown", onPointer);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);
  return (
    <div className="shop-filterbar__sort" ref={ref} data-testid="sort-select" data-value={value}>
      <button
        type="button"
        className="shop-filterbar__sorttrigger"
        data-dropdown-trigger
        data-open={open || undefined}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={t("filterBar.sortBy")}
        onClick={() => setOpen((o) => !o)}
      >
        <span>{t("filterBar.sortBy")}</span>
        <Icon name="chevron-down" size={24} />
      </button>
      {open ? (
        <ul className="shop-filterbar__sortmenu" role="listbox" aria-label={t("filterBar.sortBy")}>
          {options.map((o) => (
            <li key={o.key} role="none">
              <button
                type="button"
                role="option"
                aria-selected={o.key === value}
                data-active={o.key === value || undefined}
                data-value={o.key}
                onClick={() => {
                  onChange(o.key);
                  setOpen(false);
                }}
              >
                {t(o.label)}
              </button>
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}
