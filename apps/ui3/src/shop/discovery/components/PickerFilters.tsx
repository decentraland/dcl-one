// from: marketplace@be1caa15:webapp/src/components/AssetFilters/{CreatorsFilter/CreatorsFilter.tsx,CollectionFilter/CollectionFilter.tsx}
// (the browse sidebar's creators filter: the top creators on focus, a debounced search by name, the picks as removable
// pills; and its single-collection filter, searched by name and limited to collections on sale while browsing what is),
// in the Shop's filter styling.
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.

import { useEffect, useId, useState, type KeyboardEvent } from "react";

import { shortAddress } from "../../model/address";
import { fetchCollection, searchCollections, searchCreators } from "../api";
import { useProfiles } from "../hooks";
import { t } from "../intl";
import { useShopQuery } from "../query";
import { loadTopCreators } from "./TopCreators";
import Icon from "./Icon";
import styles from "./PickerFilters.module.css";
import { shopClient } from "../../api/host";

export const PICKER_DEBOUNCE_MS = 300;

type Option = { value: string; label: string };

/** Picked creators' names, falling back to the short address while (or if) a profile has none. */
export function useCreatorNames(addresses: string[]): Record<string, string> {
  const profiles = useProfiles(addresses);
  return Object.fromEntries(addresses.map((a) => [a, profiles[a.toLowerCase()]?.name || shortAddress(a)]));
}

/** The picked collection's name (the collection page's cache entry), the short address until it loads. */
export function useCollectionName(contractAddress: string | null): string | null {
  const meta = useShopQuery(contractAddress ? ["collection-meta", contractAddress] : null, (signal) => fetchCollection(contractAddress!, { client: shopClient(), signal }), { staleMs: 5 * 60_000 });
  if (!contractAddress) return null;
  return meta.data?.name || shortAddress(contractAddress);
}

function useDebounced(value: string, ms: number): string {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(value), ms);
    return () => clearTimeout(timer);
  }, [value, ms]);
  return debounced;
}

/** A search box whose suggestions open on focus; picking one hands its value up and clears the box. */
function SearchPicker({
  label,
  placeholder,
  queryKey,
  load,
  exclude = [],
  emptyHeader,
  onPick,
  testId,
}: {
  label: string;
  placeholder: string;
  queryKey: readonly unknown[];
  load: (term: string, signal: AbortSignal) => Promise<Option[]>;
  exclude?: string[];
  emptyHeader?: string;
  onPick: (value: string) => void;
  testId: string;
}) {
  const [term, setTerm] = useState("");
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const listId = useId();
  const debounced = useDebounced(term.trim(), PICKER_DEBOUNCE_MS);
  const settled = debounced === term.trim();
  const q = useShopQuery(open ? [...queryKey, debounced] : null, (signal) => load(debounced, signal), { staleMs: 60_000 });
  // While the box is ahead of the debounce, the last answer is for an older term: offer none of it.
  const options = settled ? (q.data ?? []).filter((o) => !exclude.includes(o.value)) : [];
  const loading = !settled || q.isLoading;
  const current = Math.min(active, options.length - 1);

  const pick = (o: Option) => {
    onPick(o.value);
    setTerm("");
    setActive(0);
    setOpen(false);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape") setOpen(false);
    else if (e.key === "ArrowDown") {
      e.preventDefault();
      setOpen(true);
      setActive(Math.min(current + 1, Math.max(options.length - 1, 0)));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive(Math.max(current - 1, 0));
    } else if (e.key === "Enter" && open && options[current]) {
      e.preventDefault();
      pick(options[current]!);
    }
  };

  return (
    <div
      className={styles.picker}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setOpen(false);
      }}
    >
      <span className={styles.box}>
        <Icon name="search" size={16} />
        <input
          type="text"
          role="combobox"
          aria-label={label}
          aria-expanded={open}
          aria-controls={open ? listId : undefined}
          aria-autocomplete="list"
          aria-activedescendant={open && options[current] ? `${listId}-${current}` : undefined}
          placeholder={placeholder}
          value={term}
          data-testid={`${testId}-input`}
          onFocus={() => setOpen(true)}
          onChange={(e) => {
            setTerm(e.target.value);
            setActive(0);
            setOpen(true);
          }}
          onKeyDown={onKeyDown}
        />
      </span>
      {open ? (
        <div className={styles.menu} data-testid={`${testId}-menu`}>
          {!term.trim() && emptyHeader && options.length ? <p className={styles.header}>{emptyHeader}</p> : null}
          {loading && !options.length ? <p className={styles.note}>{t("search.searching")}</p> : null}
          {q.error && !loading ? <p className={styles.note} role="alert">{t("picker.loadError")}</p> : null}
          {!loading && !q.error && !options.length && term.trim() ? <p className={styles.note}>{t("picker.noResults")}</p> : null}
          <ul id={listId} role="listbox" aria-label={label}>
            {options.map((o, i) => (
              <li
                key={o.value}
                id={`${listId}-${i}`}
                role="option"
                aria-selected={i === current}
                className={styles.option}
                data-testid={`${testId}-option`}
                // mousedown keeps the focus in the box, so the blur above does not close the menu before the pick.
                onMouseDown={(e) => e.preventDefault()}
                onMouseEnter={() => setActive(i)}
                onClick={() => pick(o)}
              >
                {o.label}
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </div>
  );
}

function Pills({ items, onRemove, testId }: { items: Option[]; onRemove: (value: string) => void; testId: string }) {
  if (!items.length) return null;
  return (
    <ul className={styles.pills}>
      {items.map((o) => (
        <li key={o.value}>
          <button type="button" className={styles.pill} onClick={() => onRemove(o.value)} aria-label={t("picker.remove", { name: o.label })} data-testid={`${testId}-pill`}>
            <span>{o.label}</span>
            <Icon name="close" size={14} />
          </button>
        </li>
      ))}
    </ul>
  );
}

async function loadCreators(term: string, signal: AbortSignal): Promise<Option[]> {
  if (term) return (await searchCreators(term, 10, { client: shopClient(), signal })).map((c) => ({ value: c.address, label: c.name }));
  return (await loadTopCreators(signal)).map((c) => ({ value: c.address.toLowerCase(), label: c.name }));
}

/** Any number of creators: the top creators on focus, a search by name or NAME as the box is typed in. */
export function CreatorFilter({ creators, onChange }: { creators: string[]; onChange: (creators: string[]) => void }) {
  const names = useCreatorNames(creators);
  return (
    <div className={styles.filter} data-testid="creator-filter">
      <SearchPicker
        label={t("picker.creators")}
        placeholder={t("picker.searchCreators")}
        queryKey={["picker-creators"]}
        load={loadCreators}
        exclude={creators}
        emptyHeader={t("picker.topCreators")}
        onPick={(address) => onChange([...creators, address])}
        testId="creator-picker"
      />
      <Pills items={creators.map((a) => ({ value: a, label: names[a]! }))} onRemove={(a) => onChange(creators.filter((x) => x !== a))} testId="creator" />
    </div>
  );
}

/**
 * One collection, searched by name. `onSale` limits the search to collections the creator still sells, for a grid of
 * creator listings only: the collections route's on-sale flag knows nothing of resales.
 */
export function CollectionFilter({ collection, onSale, onChange }: { collection: string | null; onSale: boolean; onChange: (collection: string | null) => void }) {
  const name = useCollectionName(collection);
  return (
    <div className={styles.filter} data-testid="collection-filter">
      <SearchPicker
        label={t("picker.collection")}
        placeholder={t("picker.searchCollections")}
        queryKey={["picker-collections", onSale]}
        load={async (term, signal) => (await searchCollections(term, { onSale }, { client: shopClient(), signal })).map((c) => ({ value: c.contractAddress.toLowerCase(), label: c.name }))}
        exclude={collection ? [collection] : []}
        onPick={(address) => onChange(address)}
        testId="collection-picker"
      />
      <Pills items={collection ? [{ value: collection, label: name! }] : []} onRemove={() => onChange(null)} testId="collection" />
    </div>
  );
}
