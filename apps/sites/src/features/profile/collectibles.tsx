import type { InfiniteData, UseInfiniteQueryResult } from "@tanstack/react-query";
import { useEffect, useId, useRef, type MouseEvent, type ReactNode, type Ref } from "react";
import { Link, useNavigate } from "react-router";

import { Button, Spinner, Tabs, TabsBar } from "@dcl/one-ui";
import { useBrokenImage } from "@dcl/one-ui/apps/sites/provisional/useBrokenImage";
import {
  BackpackBody,
  BackpackEarrings,
  BackpackEyebrows,
  BackpackEyes,
  BackpackEyewear,
  BackpackFacialHair,
  BackpackFeet,
  BackpackGenderFemale,
  BackpackGenderMale,
  BackpackGenderUnisex,
  BackpackHair,
  BackpackHandwear,
  BackpackHat,
  BackpackHead,
  BackpackHelmet,
  BackpackLowerBody,
  BackpackMask,
  BackpackMouth,
  BackpackSkins,
  BackpackTiara,
  BackpackUpperBody,
  GeneralMana,
  GeneralName,
  GeneralPolygonMana,
  GeneralRightArrow,
  GeneralSmartItem,
  ShopCredits,
} from "@dcl/one-ui/foundations/icons";

import type { CollectiblePage } from "@/data/lib/catalyst/marketplace/profile-collectibles";

import styles from "./collectibles.module.css";

export type CollectiblesQuery<T> = UseInfiniteQueryResult<InfiniteData<CollectiblePage<T>>, Error>;

export type CollectibleLinkProps = {
  to: string;
  className?: string | undefined;
  children?: ReactNode;
};

/** Opening an item or token is a client navigation, loaded on hover or focus. */
export function CollectibleLink({ to, ...props }: CollectibleLinkProps) {
  return <Link to={to} prefetch="intent" {...props} />;
}

/**
 * Every loaded page's rows, once each. A row can repeat across pages when a newer one shifts the
 * catalogue's offsets between two reads.
 */
export function loadedRows<T extends { id: string }>(data: InfiniteData<CollectiblePage<T>>): T[] {
  const seen = new Set<string>();
  const rows: T[] = [];
  for (const page of data.pages) {
    for (const row of page.items) {
      if (seen.has(row.id)) continue;
      seen.add(row.id);
      rows.push(row);
    }
  }
  return rows;
}

/** Whether the catalogue sent any row at all, so no row shown means none could be read, not an empty list. */
export function receivedAny(data: InfiniteData<CollectiblePage<unknown>>): boolean {
  return data.pages.some((page) => page.received > 0);
}

/** How many the catalogue holds, as the latest page reports it. */
export function catalogueTotal(data: InfiniteData<CollectiblePage<unknown>>): number {
  return data.pages[data.pages.length - 1]?.total ?? 0;
}

/**
 * Under a list: one button that stays mounted (so keyboard focus survives a click) reading Load more,
 * or Try again after the next page failed, plus a live region that announces the page loading and
 * its failure. Nothing at the end of the list. Button's `loading` keeps it focusable and refuses
 * a second click while the page loads.
 */
export function loadMore<T>(query: CollectiblesQuery<T>, noun: string, onRequest: () => void): ReactNode {
  const fetching = query.isFetchingNextPage;
  const failed = query.isFetchNextPageError && !fetching;
  if (!query.hasNextPage && !fetching) return null;
  return (
    <>
      <span role="status">
        {fetching ? (
          <span className={styles.visuallyHidden}>{`Loading more ${noun}`}</span>
        ) : failed ? (
          `More ${noun} could not be loaded.`
        ) : null}
      </span>
      <Button
        variant="secondary"
        size="md"
        loading={fetching}
        onClick={() => {
          onRequest();
          void query.fetchNextPage();
        }}
      >
        {failed ? "Try again" : "Load more"}
      </Button>
    </>
  );
}

type FocusRows = { expectRowsFrom: (index: number) => void };

/** Try again for a read that ended without rows to show: read every loaded page again. */
function tryAgain<T>(query: CollectiblesQuery<T>, focus: FocusRows): ReactNode {
  return (
    <Button
      variant="secondary"
      size="md"
      onClick={() => {
        focus.expectRowsFrom(0);
        void query.refetch();
      }}
    >
      Try again
    </Button>
  );
}

/**
 * The state of a list before it has rows to show: the first read failed (with no data yet, a Try
 * again refetch puts the query back to pending, so it shows as loading), is loading, or brought a
 * page whose every row failed to read (not an empty list; Load more when later pages may still read,
 * Try again otherwise). `null` once there are rows, or none to show.
 */
export function unreadyState<T extends { id: string }>(
  query: CollectiblesQuery<T>,
  rows: readonly T[],
  copy: { noun: string; unavailable: string; unreadable: string },
  focus: FocusRows,
): CollectiblesState | null {
  if (query.isError && !query.data) {
    return { kind: "error", message: copy.unavailable, action: tryAgain(query, focus) };
  }
  if (!query.data) return { kind: "loading", label: `Loading ${copy.noun}` };
  if (rows.length === 0 && receivedAny(query.data)) {
    return {
      kind: "error",
      message: copy.unreadable,
      action: loadMore(query, copy.noun, () => focus.expectRowsFrom(0)) ?? tryAgain(query, focus),
    };
  }
  return null;
}

/**
 * Card actions are links into the Shop and Create, which this router serves: a plain click
 * navigates in place, as the card's title does; a modified or middle click keeps the browser's own.
 */
export function useRouterClick(): (href: string) => (event: MouseEvent<HTMLAnchorElement>) => void {
  const navigate = useNavigate();
  return (href) => (event) => {
    if (
      event.defaultPrevented ||
      event.button !== 0 ||
      event.metaKey ||
      event.ctrlKey ||
      event.shiftKey ||
      event.altKey
    ) {
      return;
    }
    event.preventDefault();
    void navigate(href);
  };
}

/**
 * Where keyboard focus goes when the control that held it unmounts: the last Load more (the list
 * ended) or the first read's Try again. A click calls `expectRowsFrom` with the index of the first
 * row it will bring. When the read ends, and only if focus has fallen to the page body, the link of
 * that row takes it; when the read brought no new row (an empty list, only repeated rows, a
 * failure), the panel does. A control that stays mounted keeps the focus. `readAt` changes with
 * every read that ends (TanStack's data and error update times), so a read that ends within one
 * render still counts.
 */
export function useFocusNewRows(rows: number, readAt: number) {
  const listRef = useRef<HTMLUListElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const pending = useRef<number | null>(null);
  useEffect(() => {
    const from = pending.current;
    if (from === null) return;
    pending.current = null;
    const active = document.activeElement;
    if (active && active !== document.body) return;
    const row = listRef.current?.children[from]?.querySelector<HTMLElement>("a, button");
    (row ?? panelRef.current)?.focus();
  }, [rows, readAt]);
  return {
    listRef,
    panelRef,
    expectRowsFrom: (index: number) => {
      pending.current = index;
    },
  };
}

export type CategoryOption<T> = { value: T; label: string; icon?: ReactNode };

/** The tabpanel the category tabs control. */
export type CategoryPanel = {
  role: "tabpanel";
  id: string;
  "aria-labelledby": string;
};

/**
 * The category filter as the shared pill TabsBar, and the attributes of the tabpanel it controls.
 * Choosing a tab is a button press; the arrows, Home and End move between them.
 */
export function useCategoryTabs<T extends string | null>(
  label: string,
  options: readonly CategoryOption<T>[],
  selected: T,
  onSelect: (value: T) => void,
): { filters: ReactNode; panel: CategoryPanel } {
  const ids = useId();
  const tabId = (value: T) => `${ids}-${value ?? "all"}`;
  const panelId = `${ids}-panel`;
  const filters = (
    <TabsBar label={label} variant="pill">
      {options.map((option) => (
        <Tabs
          key={option.value ?? "all"}
          id={tabId(option.value)}
          variant="pill"
          selected={option.value === selected}
          aria-controls={panelId}
          onClick={() => onSelect(option.value)}
          label={
            <span className={styles.tabLabel}>
              {option.icon}
              {option.label}
            </span>
          }
        />
      ))}
    </TabsBar>
  );
  return {
    filters,
    panel: {
      role: "tabpanel",
      id: panelId,
      "aria-labelledby": tabId(selected),
    },
  };
}

export type CollectiblesState =
  | { kind: "loading"; label: string }
  /** A read failed: say so, with the way to try again. Never shown as an empty list. */
  | { kind: "error"; message: string; action?: ReactNode }
  /** Nothing to list: an empty message, announced by its title, or a one-line note. */
  | { kind: "empty"; label: string; children: ReactNode }
  /** `more` keeps Load more under a note when a filter matched nothing on the pages loaded so far. */
  | { kind: "empty-note"; message: string; more?: ReactNode }
  | {
      kind: "list";
      /** Announced when loading ends; shown above the list unless `showCount` is false. */
      count: string;
      showCount?: boolean;
      /** Cards in a grid, full-width rows (NAMEs), or the photo grid. */
      layout?: "grid" | "rows" | "photos";
      children: ReactNode;
      /** The list element, so the page can move focus to a row it loaded. */
      listRef?: Ref<HTMLUListElement>;
      /** Under the list: Load more, its progress, or why the next page failed. */
      more?: ReactNode;
    };

export type CollectiblesPanelProps = {
  /** The category tabs; shown above every state. */
  filters?: ReactNode;
  /** Beside the filters, e.g. a "View all" link. */
  aside?: ReactNode;
  /** The tabpanel attributes when `filters` are category tabs. */
  panel?: CategoryPanel;
  state: CollectiblesState;
  /** Makes the panel focusable from script, so focus has somewhere to go when a control leaves. */
  panelRef?: Ref<HTMLDivElement>;
};

/**
 * A tabpanel with nothing focusable inside is a tab stop itself, so the keyboard reaches its text;
 * otherwise the panel takes focus only from script, after a control it held went away.
 */
function panelTabIndex(state: CollectiblesState, tabpanel: boolean, scripted: boolean): number | undefined {
  const empty =
    state.kind === "loading" ||
    (state.kind === "empty-note" && !state.more) || (state.kind === "error" && !state.action);
  if (tabpanel && empty) return 0;
  return scripted ? -1 : undefined;
}

/** A profile's Creations, Assets or Photos tab body: the filters, then loading, an error, an empty state or the list. */
export function CollectiblesPanel({ filters, aside, panel, state, panelRef }: CollectiblesPanelProps) {
  const list = state.kind === "list" ? state : null;
  const countShown = list !== null && list.showCount !== false;
  const more = state.kind === "list" || state.kind === "empty-note" ? state.more : undefined;

  return (
    <div className={styles.root}>
      {filters || aside ? (
        <div className={styles.header}>
          {filters}
          {aside ? <div className={styles.aside}>{aside}</div> : null}
        </div>
      ) : null}
      <div
        {...panel}
        ref={panelRef}
        className={styles.panel}
        tabIndex={panelTabIndex(state, panel !== undefined, panelRef !== undefined)}
      >
        {/* One live region, always mounted, so a screen reader hears how loading ended. */}
        <p className={countShown ? styles.count : styles.status} role="status">
          {state.kind === "loading" ? (
            <>
              <Spinner variant="light" className={styles.spinner} aria-hidden="true" />
              <span className={styles.visuallyHidden}>{state.label}</span>
            </>
          ) : state.kind === "error" ? (
            state.message
          ) : state.kind === "empty-note" ? (
            <span className={styles.note}>{state.message}</span>
          ) : state.kind === "list" ? (
            countShown ? (
              state.count
            ) : (
              <span className={styles.visuallyHidden}>{state.count}</span>
            )
          ) : (
            // The empty message below shows this title; the region only announces it.
            <span className={styles.visuallyHidden}>{state.label}</span>
          )}
        </p>
        {state.kind === "error" && state.action ? <div className={styles.errorAction}>{state.action}</div> : null}
        {state.kind === "empty" ? state.children : null}
        {list ? (
          <ul ref={list.listRef} className={styles[list.layout ?? "grid"]}>
            {list.children}
          </ul>
        ) : null}
        {more ? <div className={styles.more}>{more}</div> : null}
      </div>
    </div>
  );
}

/** One cell of the list; wraps a CollectibleCard or a NameRow. */
export function CollectiblesItem({ children }: { children: ReactNode }) {
  return <li className={styles.item}>{children}</li>;
}

/** The header's link to the full list elsewhere, with its chevron. */
export function ViewAllLink({ href, label }: { href: string; label: string }) {
  return (
    <CollectibleLink to={href} className={styles.viewAll}>
      {label}
      <GeneralRightArrow size={16} aria-hidden="true" />
    </CollectibleLink>
  );
}

/** A NAME as a row: the NAME tile, the name and its `.dcl.eth` suffix, and its actions. */
export function NameRow({ name, actions }: { name: string; actions?: ReactNode }) {
  return (
    <div className={styles.name}>
      <span className={styles.nameTile} aria-hidden="true">
        <GeneralName size={26} />
      </span>
      <p className={styles.nameLabel}>
        {name}
        <span className={styles.nameSuffix}>.dcl.eth</span>
      </p>
      {actions ? <div className={styles.nameActions}>{actions}</div> : null}
    </div>
  );
}

export type CollectibleRarity = "common" | "uncommon" | "rare" | "epic" | "legendary" | "exotic" | "mythic" | "unique";

export type CollectibleCardProps = {
  title: string;
  imageUrl?: string;
  /** A decorative glyph shown instead of an image that is missing or fails, e.g. a NAME's. */
  placeholder?: ReactNode;
  /** The item's or token's own page; the whole card opens it. */
  href: string;
  /** Paints the image surface and the rarity tag. Without it the card has neither. */
  rarity?: CollectibleRarity;
  /** The line under the title, e.g. "By Alice". */
  byline?: ReactNode;
  /** In MANA on its network, or in credits. */
  price?:
    { currency: "mana"; amount: string; network: "ethereum" | "polygon" } | { currency: "credits"; amount: string };
  /** Shown where the price goes when there is none, e.g. "Not for sale". */
  priceFallback?: string;
  /** Glyphs next to the rarity tag; see `CollectibleBadges`. */
  badges?: ReactNode;
  /** Revealed over the badge row on hover or keyboard focus, e.g. Buy. */
  action?: ReactNode;
};

const RARITY_LABEL: Record<CollectibleRarity, string> = {
  common: "Common",
  uncommon: "Uncommon",
  rare: "Rare",
  epic: "Epic",
  legendary: "Legendary",
  exotic: "Exotic",
  mythic: "Mythic",
  unique: "Unique",
};

/**
 * A wearable, emote, NAME or LAND as the catalogue lists it: its image on the rarity's colour,
 * title, byline, price and badges. On hover or focus it lifts and slides its action in over the
 * badges, as the source does. The design system has no catalogue card yet, so it is drawn here.
 */
export function CollectibleCard({
  title,
  imageUrl,
  placeholder,
  href,
  rarity,
  byline,
  price,
  priceFallback,
  badges,
  action,
}: CollectibleCardProps) {
  const { broken, ref, onError } = useBrokenImage(imageUrl);
  const PriceMark =
    price?.currency === "credits" ? ShopCredits : price?.network === "ethereum" ? GeneralMana : GeneralPolygonMana;
  const rootClass = [styles.card, rarity && styles[rarity], action && styles.withAction].filter(Boolean).join(" ");

  return (
    <article className={rootClass}>
      <div className={styles.media}>
        {imageUrl && !broken ? (
          <img ref={ref} className={styles.image} src={imageUrl} alt="" loading="lazy" onError={onError} />
        ) : placeholder ? (
          <span className={styles.placeholder} aria-hidden="true">
            {placeholder}
          </span>
        ) : null}
      </div>
      <div className={styles.content}>
        <h3 className={styles.title}>
          {/* The title's link covers the card, so the whole card opens the item. */}
          <CollectibleLink to={href} className={styles.cardLink}>
            {title}
          </CollectibleLink>
        </h3>
        {byline ? <div className={styles.byline}>{byline}</div> : null}
        {price ? (
          <p className={styles.price}>
            <PriceMark size={32} aria-hidden="true" />
            <span>
              <span className={styles.visuallyHidden}>Price: </span>
              {price.amount}
              <span className={styles.visuallyHidden}>{price.currency === "credits" ? " credits" : " MANA"}</span>
            </span>
          </p>
        ) : priceFallback ? (
          <p className={styles.fallback}>{priceFallback}</p>
        ) : null}
        <div className={styles.badges}>
          {rarity ? <span className={styles.rarity}>{RARITY_LABEL[rarity]}</span> : null}
          {badges ? <span className={styles.info}>{badges}</span> : null}
        </div>
        {action ? <div className={styles.action}>{action}</div> : null}
      </div>
    </article>
  );
}

type IconComponent = typeof BackpackHat;

/** The catalogue's wearable slots, keyed as it spells them. */
const CATEGORY_ICON: Record<string, IconComponent> = {
  body_shape: BackpackBody,
  earring: BackpackEarrings,
  eyebrows: BackpackEyebrows,
  eyes: BackpackEyes,
  eyewear: BackpackEyewear,
  facial_hair: BackpackFacialHair,
  feet: BackpackFeet,
  hair: BackpackHair,
  hands_wear: BackpackHandwear,
  hat: BackpackHat,
  helmet: BackpackHelmet,
  lower_body: BackpackLowerBody,
  mask: BackpackMask,
  mouth: BackpackMouth,
  skin: BackpackSkins,
  tiara: BackpackTiara,
  top_head: BackpackHead,
  upper_body: BackpackUpperBody,
};

function categoryLabel(category: string): string {
  return category
    .split("_")
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
}

function Badge({ icon: Icon, label }: { icon: IconComponent; label: string }) {
  return (
    <span className={styles.badge} role="img" aria-label={label} title={label}>
      <Icon size={20} aria-hidden="true" />
    </span>
  );
}

export type CollectibleBadgesProps = {
  /** The wearable slot; a slot without a glyph (an emote's kind) shows none. */
  category?: string;
  /** `BaseMale` and `BaseFemale` together are unisex. */
  bodyShapes?: readonly string[];
  isSmart?: boolean;
};

/** A wearable's slot, body shape and smart badges, each named for assistive technology. */
export function CollectibleBadges({ category, bodyShapes = [], isSmart = false }: CollectibleBadgesProps) {
  const slot = category ? CATEGORY_ICON[category] : undefined;
  const male = bodyShapes.some((shape) => shape.toLowerCase().includes("basemale"));
  const female = bodyShapes.some((shape) => shape.toLowerCase().includes("basefemale"));
  const body =
    male && female
      ? { icon: BackpackGenderUnisex, label: "Unisex" }
      : male
        ? { icon: BackpackGenderMale, label: "For male" }
        : female
          ? { icon: BackpackGenderFemale, label: "For female" }
          : null;

  return (
    <>
      {slot && category ? <Badge icon={slot} label={categoryLabel(category)} /> : null}
      {body ? <Badge icon={body.icon} label={body.label} /> : null}
      {isSmart ? <Badge icon={GeneralSmartItem} label="Smart wearable" /> : null}
    </>
  );
}
