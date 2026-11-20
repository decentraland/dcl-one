import {
  useEffect,
  useId,
  useRef,
  useState,
  type ComponentType,
  type MouseEvent,
  type ReactNode,
  type Ref
} from 'react'
import {
  Button,
  ShopCard,
  ShopCardPrice,
  Spinner,
  Tabs,
  TabsBar,
  type ShopCardCreator,
  type ShopCardLinkProps,
  type ShopCardProps
} from '../../../index'
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
  GeneralName,
  GeneralRightArrow,
  GeneralSmartItem
} from '../../../foundations/icons'

import styles from './collectibles.module.css'

export type CollectibleLinkProps = {
  to: string
  className?: string | undefined
  children?: ReactNode
}

/**
 * Under a list: one button that stays mounted (so keyboard focus survives a click) reading Load more,
 * or Try again after the next page failed, plus a live region that announces the page loading and
 * its failure. Nothing at the end of the list. Button's `loading` keeps it focusable and refuses
 * a second click while the page loads.
 */
export function LoadMore({
  noun,
  fetching,
  failed,
  onRequest
}: {
  noun: string
  fetching: boolean
  failed: boolean
  onRequest: () => void
}) {
  return (
    <>
      <span role="status">
        {fetching ? (
          <span className={styles.visuallyHidden}>{`Loading more ${noun}`}</span>
        ) : failed ? (
          `More ${noun} could not be loaded.`
        ) : null}
      </span>
      <Button variant="secondary" size="md" loading={fetching} onClick={onRequest}>
        {failed ? 'Try again' : 'Load more'}
      </Button>
    </>
  )
}

/**
 * Where focus goes after a read, and whether the reader asked for the read under way: `retry` is
 * `reading` from a Try again until that read ends, then `done` until the next read ends.
 */
export type FocusRows = {
  expectRowsFrom: (index: number) => void
  retryAgain: () => void
  retry: 'idle' | 'reading' | 'done'
}

/**
 * Where keyboard focus goes when the control that held it unmounts: the last Load more (the list
 * ended) or the first read's Try again. A click calls `expectRowsFrom` with the index of the first
 * row it will bring. When the read ends, and only if focus has fallen to the page body, the link of
 * that row takes it; when the read brought no new row, the panel's error action (a failed read's
 * Try again or Load more) does, or else the panel (an empty list, only repeated rows). A control that stays mounted keeps the focus. `readAt` changes with
 * every read that ends (TanStack's data and error update times), so a read that ends within one
 * render still counts.
 */
export function useFocusNewRows(rows: number, readAt: number) {
  const listRef = useRef<HTMLUListElement>(null)
  const panelRef = useRef<HTMLDivElement>(null)
  const pending = useRef<number | null>(null)
  const [retry, setRetry] = useState<FocusRows['retry']>('idle')
  useEffect(() => {
    // A read ended: the reader's Try again is done, and an earlier one no longer describes it.
    setRetry((now) => (now === 'reading' ? 'done' : 'idle'))
  }, [readAt])
  useEffect(() => {
    const from = pending.current
    if (from === null) return
    pending.current = null
    const active = document.activeElement
    if (active && active !== document.body) return
    const row = listRef.current?.children[from]?.querySelector<HTMLElement>(FOCUSABLE)
    const action = panelRef.current?.querySelector<HTMLElement>(
      `[data-error-action] :is(${FOCUSABLE})`
    )
    ;(row ?? action ?? panelRef.current)?.focus()
  }, [rows, readAt])
  return {
    listRef,
    panelRef,
    retry,
    expectRowsFrom: (index: number) => {
      pending.current = index
    },
    retryAgain: () => {
      pending.current = 0
      setRetry('reading')
    }
  }
}

/** What a reader can tab to: not a pointer-only target. */
const FOCUSABLE = 'a:not([tabindex="-1"]), button:not([tabindex="-1"])'

export type CategoryOption<T> = { value: T; label: string; icon?: ReactNode }

/** The tabpanel the category tabs control. */
type CategoryPanel = {
  role: 'tabpanel'
  id: string
  'aria-labelledby': string
}

/**
 * The category filter as the shared pill TabsBar, and the attributes of the tabpanel it controls.
 * Choosing a tab is a button press; the arrows, Home and End move between them.
 */
export function useCategoryTabs<T extends string | null>(
  label: string,
  options: readonly CategoryOption<T>[],
  selected: T,
  onSelect: (value: T) => void
): { filters: ReactNode; panel: CategoryPanel } {
  const ids = useId()
  const tabId = (value: T) => `${ids}-${value ?? 'all'}`
  const panelId = `${ids}-panel`
  const filters = (
    <TabsBar label={label} variant="pill">
      {options.map((option) => (
        <Tabs
          key={option.value ?? 'all'}
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
  )
  return {
    filters,
    panel: {
      role: 'tabpanel',
      id: panelId,
      'aria-labelledby': tabId(selected)
    }
  }
}

export type CollectiblesState =
  | { kind: 'loading'; label: string }
  /** A read failed: say so, with the way to try again. Never shown as an empty list. */
  | { kind: 'error'; message: string; action?: ReactNode }
  /** Nothing to list: an empty message, announced by its title, or a one-line note. */
  | { kind: 'empty'; label: string; children: ReactNode }
  /** `more` keeps Load more under a note when a filter matched nothing on the pages loaded so far. */
  | { kind: 'empty-note'; message: string; more?: ReactNode }
  | {
      kind: 'list'
      /** Announced when loading ends; shown above the list unless `showCount` is false. */
      count: string
      showCount?: boolean
      /** Cards in a grid, full-width rows (NAMEs), or the photo grid. */
      layout?: 'grid' | 'rows' | 'photos'
      children: ReactNode
      /** The list element, so the page can move focus to a row it loaded. */
      listRef?: Ref<HTMLUListElement>
      /** Under the list: Load more, its progress, or why the next page failed. */
      more?: ReactNode
    }

type CollectiblesPanelProps = {
  /** The category tabs; shown above every state. */
  filters?: ReactNode
  /** Beside the filters, e.g. a "View all" link. */
  aside?: ReactNode
  /** The tabpanel attributes when `filters` are category tabs. */
  panel?: CategoryPanel
  state: CollectiblesState
  /** Makes the panel focusable from script, so focus has somewhere to go when a control leaves. */
  panelRef?: Ref<HTMLDivElement>
}

/**
 * A tabpanel with nothing focusable inside is a tab stop itself, so the keyboard reaches its text;
 * otherwise the panel takes focus only from script, after a control it held went away.
 */
function panelTabIndex(
  state: CollectiblesState,
  tabpanel: boolean,
  scripted: boolean
): number | undefined {
  const empty =
    state.kind === 'loading' ||
    (state.kind === 'empty-note' && !state.more) ||
    (state.kind === 'error' && !state.action)
  if (tabpanel && empty) return 0
  return scripted ? -1 : undefined
}

/** A profile's Creations, Assets or Photos tab body: the filters, then loading, an error, an empty state or the list. */
export function CollectiblesPanel({
  filters,
  aside,
  panel,
  state,
  panelRef
}: CollectiblesPanelProps) {
  const list = state.kind === 'list' ? state : null
  const countShown = list !== null && list.showCount !== false
  const more = state.kind === 'list' || state.kind === 'empty-note' ? state.more : undefined

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
          {state.kind === 'loading' ? (
            <>
              <Spinner variant="light" className={styles.spinner} aria-hidden="true" />
              <span className={styles.visuallyHidden}>{state.label}</span>
            </>
          ) : state.kind === 'error' ? (
            state.message
          ) : state.kind === 'empty-note' ? (
            <span className={styles.note}>{state.message}</span>
          ) : state.kind === 'list' ? (
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
        {state.kind === 'error' && state.action ? (
          <div className={styles.errorAction} data-error-action="">
            {state.action}
          </div>
        ) : null}
        {state.kind === 'empty' ? state.children : null}
        {list ? (
          <ul ref={list.listRef} className={styles[list.layout ?? 'grid']}>
            {list.children}
          </ul>
        ) : null}
        {more ? <div className={styles.more}>{more}</div> : null}
      </div>
    </div>
  )
}

/** One cell of the list; wraps a ShopCard or a NameRow. */
export function CollectiblesItem({ children }: { children: ReactNode }) {
  return <li className={styles.item}>{children}</li>
}

/** The header's link to the full list elsewhere, with its chevron. */
export function ViewAllLink({
  href,
  label,
  linkComponent: Link
}: {
  href: string
  label: string
  linkComponent: ComponentType<CollectibleLinkProps>
}) {
  return (
    <Link to={href} className={styles.viewAll}>
      {label}
      <GeneralRightArrow size={16} aria-hidden="true" />
    </Link>
  )
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
  )
}

type IconComponent = typeof BackpackHat

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
  upper_body: BackpackUpperBody
}

function categoryLabel(category: string): string {
  return category
    .split('_')
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ')
}

function Badge({ icon: Icon, label }: { icon: IconComponent; label: string }) {
  return (
    <span className={styles.badge} role="img" aria-label={label} title={label}>
      <Icon size={16} aria-hidden="true" />
    </span>
  )
}

/** A wearable's slot glyph for the card's corner, named for assistive technology; none for a slot without one. */
export function categoryGlyph(category: string | undefined): ReactNode | undefined {
  const slot = category ? CATEGORY_ICON[category] : undefined
  return slot && category ? <Badge icon={slot} label={categoryLabel(category)} /> : undefined
}

/**
 * A wearable's body shape and smart badges, each named for assistive technology (`BaseMale` and
 * `BaseFemale` together are unisex); none without either.
 */
export function collectibleBadges({
  bodyShapes,
  isSmart
}: {
  bodyShapes: readonly string[]
  isSmart: boolean
}): ReactNode | undefined {
  const male = bodyShapes.some((shape) => shape.toLowerCase().includes('basemale'))
  const female = bodyShapes.some((shape) => shape.toLowerCase().includes('basefemale'))
  const body =
    male && female
      ? { icon: BackpackGenderUnisex, label: 'Unisex' }
      : male
        ? { icon: BackpackGenderMale, label: 'For male' }
        : female
          ? { icon: BackpackGenderFemale, label: 'For female' }
          : null
  if (!body && !isSmart) return undefined
  return (
    <>
      {body ? <Badge icon={body.icon} label={body.label} /> : null}
      {isSmart ? <Badge icon={GeneralSmartItem} label="Smart wearable" /> : null}
    </>
  )
}

/** The price as the Shop's card rule decided it: credits, MANA on its network, or why there is none. */
type CreationCardItem = {
  name: string
  thumbnail?: string
  rarity?: ShopCardProps['rarity']
  network: 'ethereum' | 'polygon'
  payment:
    | { kind: 'credits'; credits: string }
    | { kind: 'mana'; mana: string }
    | { kind: 'creditsUnavailable' | 'none' }
  traits: { category?: string; bodyShapes: readonly string[]; isSmart: boolean }
}

function creationPrice(item: CreationCardItem) {
  const payment = item.payment
  if (payment.kind === 'credits') return <ShopCardPrice kind="credits" amount={payment.credits} />
  if (payment.kind === 'mana')
    return <ShopCardPrice kind="mana" amount={payment.mana} network={item.network} />
  // A credit-eligible sale whose quote failed says so; anything else unpriced is not for sale.
  return (
    <ShopCardPrice
      kind="none"
      label={payment.kind === 'creditsUnavailable' ? 'Credits price unavailable' : 'Not for sale'}
    />
  )
}

/**
 * A catalogue item as the public ShopCard: its price by the Shop's rule and, unless `actions` is
 * off, Buy when it is for sale or View in Shop when it is not.
 */
export function CreationCard({
  item,
  creator,
  actions = true,
  href,
  onNavigate,
  linkComponent
}: {
  item: CreationCardItem
  creator?: ShopCardCreator
  actions?: boolean
  href: string
  onNavigate: (event: MouseEvent<HTMLAnchorElement>) => void
  linkComponent: ComponentType<ShopCardLinkProps>
}) {
  return (
    <ShopCard
      name={item.name}
      image={item.thumbnail}
      rarity={item.rarity}
      creator={creator}
      price={creationPrice(item)}
      categoryIcon={categoryGlyph(item.traits.category)}
      badges={collectibleBadges(item.traits)}
      href={href}
      linkComponent={linkComponent}
      variant="catalogue"
      headingLevel={3}
      actions={
        !actions ? undefined : item.payment.kind !== 'none' ? (
          <Button href={href} onClick={onNavigate} size="md" aria-label={`Buy ${item.name}`}>
            Buy
          </Button>
        ) : (
          <Button
            href={href}
            onClick={onNavigate}
            variant="secondary"
            size="md"
            aria-label={`View in Shop: ${item.name}`}
          >
            View in Shop
          </Button>
        )
      }
    />
  )
}
