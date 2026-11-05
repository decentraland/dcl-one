// In-world Shop: the main menu's Shop page. The avatar on the left is the engine's preview (the
// Backpack's), wearing what the buyer tries on, never saved to the look; the grid on the right reads
// the web Shop's catalogue through TanStack (wearablesQuery) and prices each row by the web Shop's own
// card rule. Item detail and buying come in their own steps.

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { formatCredits, formatCreditsFull } from '../../../shop/checkout/lib/currency'
import CreditMark from '../../../shop/credits/components/CreditMark'
import { itemUrn } from '../../../shop/detail/lib/urn'
import { cardPrice, type CardPrice } from '../../../shop/discovery/cardPrice'
import type { CatalogItem } from '../../../shop/discovery/lib/catalogItem'
import { REMOTE_FLAG, useRemoteFlag } from '../../../shop/discovery/lib/remoteFlags'
import { saleDiscountPct } from '../../../shop/discovery/lib/sale'
import { CHAIN } from '../../../shop/model/chain'
import { formatMana } from '../../../shop/model/money'
import { SIGNED_OUT } from '../../../shop/model/settlement'
import { Button, Chip, ChevronDown, ChevronLeft, EmptyState, ManaIcon, MaskIcon, OptionMenu, SearchField, ShopItemCard, Spinner, Tabs, type TabItem } from '../../design'
import type { PreviewFocus, Wearable } from '../../engine/protocol'
import { shortAddr } from '../../lib/identity'
import { CategoryIcon } from '../backpack/categoryIcons'
import { EngineViewport } from '../engine/EngineViewport'
import { MainMenuShell } from '../menu/MainMenuShell'
import { useProfile } from '../session/profileStore'
import type { ProfileState, ShopState } from '../session/useEngineSession'
import upperBodyIcon from '../../assets/category-icons/upper_body.png'
import { SHOP_PAGE_SIZE, useShopWearables, WEARABLE_CATEGORIES, WEARABLE_SORTS, type WearableCategory, type WearableFilters, type WearableSort } from './wearablesQuery'
import styles from './InWorldShop.module.css'

type Section = 'wearables'
const SECTIONS: TabItem<Section>[] = [{ id: 'wearables', label: 'Wearables', icon: <MaskIcon src={upperBodyIcon} size={24} /> }]
const SEARCH_DEBOUNCE_MS = 350
/** How close to the end of the grid (px) the next page starts loading. */
const LOAD_AHEAD_PX = 600

export interface InWorldShopProps {
  shop: ShopState
  profile: ProfileState
  onNavigate: (page: string) => void
  setEngineViewport: (region: 'map' | 'avatarPreview' | 'lobby', rect: { x: number; y: number; width: number; height: number } | null) => void
  /** The look the avatar wears now (empty until the engine has sent it): try-on swaps one category of it. */
  equipped: Wearable[]
  /** The Backpack's preview: a wearable set on the preview avatar, never saved; null restores the look. */
  preview: (urns: string[] | null) => void
  /** Frames the preview avatar, whose framing the Backpack shares. */
  focus: (focus: PreviewFocus) => void
}

/** The item a try-on puts on: its own URN, else its collections-v2 URN where one exists (Polygon). */
export function tryOnUrn(item: CatalogItem): string | null {
  if (item.urn) return item.urn
  return item.chainId === CHAIN.POLYGON || item.chainId === CHAIN.AMOY ? itemUrn(item) : null
}

export default function InWorldShop({ shop, profile, onNavigate, setEngineViewport, equipped, preview, focus }: InWorldShopProps): React.JSX.Element | null {
  const [category, setCategory] = useState<WearableCategory>('all')
  const [sort, setSort] = useState<WearableSort>('newest')
  const [draft, setDraft] = useState('')
  const [search, setSearch] = useState('')
  const [sortOpen, setSortOpen] = useState(false)
  const [tryingOn, setTryingOn] = useState<string | null>(null)
  const scroller = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const t = setTimeout(() => setSearch(draft.trim().toLowerCase()), SEARCH_DEBOUNCE_MS)
    return () => clearTimeout(t)
  }, [draft])

  const creatorSales = useRemoteFlag(REMOTE_FLAG.creatorSales)
  const filters = useMemo<WearableFilters>(() => ({ category, sort, search }), [category, sort, search])
  const grid = useShopWearables(filters, { creatorSales, enabled: shop.open })
  // Offset pages over a live feed can repeat a row; it shows once.
  const items = useMemo(() => {
    const seen = new Set<string>()
    return (grid.data?.pages.flatMap((page) => page.items) ?? []).filter((item) => {
      if (seen.has(item.id)) return false
      seen.add(item.id)
      return true
    })
  }, [grid.data])
  const total = grid.data?.pages[0]?.total ?? 0

  // A new filter set starts at the top of the grid.
  useEffect(() => {
    if (scroller.current) scroller.current.scrollTop = 0
  }, [filters])

  // Only a preview this page made is undone: the Backpack shares the preview avatar.
  const previewing = useRef(false)
  const restoreLook = useCallback(() => {
    if (!previewing.current) return
    previewing.current = false
    preview(null)
  }, [preview])
  // Opening frames the whole avatar (the Backpack may have left it on the feet); closing gives the
  // avatar its own look back, and a reopen starts with nothing tried on.
  useEffect(() => {
    if (shop.open) {
      focus('body')
      return
    }
    setTryingOn(null)
    setSortOpen(false)
    restoreLook()
  }, [shop.open, focus, restoreLook])
  useEffect(() => restoreLook, [restoreLook])

  if (!shop.open) return null

  // The look arrives from the engine once asked for; until then a try-on would dress a bare avatar.
  const lookKnown = equipped.length > 0
  const toggleTryOn = (item: CatalogItem): void => {
    const urn = tryOnUrn(item)
    if (urn == null || !lookKnown) return
    if (tryingOn === item.id) {
      setTryingOn(null)
      restoreLook()
      return
    }
    setTryingOn(item.id)
    previewing.current = true
    preview([...equipped.filter((w) => w.category !== item.wearableCategory).map((w) => w.urn), urn])
  }
  const pickSort = (id: string): void => {
    const next = WEARABLE_SORTS.find((s) => s.id === id)
    if (next != null) setSort(next.id)
    setSortOpen(false)
  }
  const clearSearch = (): void => {
    setDraft('')
    setSearch('')
  }
  const exploreAll = (): void => {
    clearSearch()
    setCategory('all')
  }
  const onScroll = (e: React.UIEvent<HTMLDivElement>): void => {
    const el = e.currentTarget
    if (grid.hasNextPage && !grid.isFetching && el.scrollTop + el.clientHeight >= el.scrollHeight - LOAD_AHEAD_PX) void grid.fetchNextPage()
  }

  const p = profile.data
  const sortLabel = WEARABLE_SORTS.find((s) => s.id === sort)?.label ?? WEARABLE_SORTS[0].label
  // A new filter set shows the last one's rows, dimmed, until it answers.
  const stale = grid.isPlaceholderData
  // Only a narrowed feed has somewhere wider to go.
  const narrowed = search !== '' || category !== 'all'
  return (
    <MainMenuShell
      active="shop"
      profileName={p?.name}
      profilePicture={p?.picture}
      profileAddress={p?.address}
      profileClaimed={p?.hasClaimedName}
      onNavigate={onNavigate}
      onClose={shop.toggle}
      transparentBody
    >
      <div className={styles.page}>
        <div className={styles.backdrop} aria-hidden="true" />
        <header className={styles.head}>
          <h1 className={styles.title}>Shop</h1>
          <Tabs variant="section" items={SECTIONS} value="wearables" onChange={() => undefined} aria-label="Shop sections" />
          <div className={styles.tools}>
            <div className={styles.sortWrap}>
              <Button variant="light" className={styles.toolBtn} aria-label={`Sort by: ${sortLabel}`} aria-expanded={sortOpen} onClick={() => setSortOpen((open) => !open)}>
                Sort by
                <ChevronDown size={18} />
              </Button>
              {sortOpen && (
                <OptionMenu
                  className={styles.sortMenu}
                  onClose={() => setSortOpen(false)}
                  sections={[{ label: 'Sort by', options: WEARABLE_SORTS, value: sort, onChange: pickSort }]}
                />
              )}
            </div>
            <div className={styles.searchWrap}>
              <SearchField variant="light" value={draft} onChange={setDraft} placeholder="Search" />
            </div>
          </div>
        </header>

        <div className={styles.stage}>
          {/* The engine draws the avatar preview here, seen through the backdrop's hole. */}
          <div className={styles.preview}>
            <EngineViewport region="avatarPreview" report={setEngineViewport} />
          </div>
          <section className={styles.content} aria-label="Wearables">
            <div className={styles.chips} role="group" aria-label="Categories">
              {WEARABLE_CATEGORIES.map((c) => (
                <Chip key={c.key} variant="dark" label={c.label} icon={<MaskIcon src={c.icon} size={26} />} selected={c.key === category} onClick={() => setCategory(c.key)} />
              ))}
            </div>
            {search !== '' && (
              <div className={styles.results}>
                <button type="button" className={styles.back} aria-label="Exit search" onClick={clearSearch}>
                  <ChevronLeft size={18} />
                </button>
                <span>
                  Results for ‘{search}’{grid.data != null && !stale ? ` (${total})` : ''}
                </span>
              </div>
            )}
            <div ref={scroller} className={styles.scroller} onScroll={onScroll}>
              {grid.isPending ? (
                <div className={styles.grid} role="status" aria-label="Loading wearables">
                  {Array.from({ length: SHOP_PAGE_SIZE / 4 }, (_, i) => (
                    <span key={i} className={styles.skeleton} />
                  ))}
                </div>
              ) : grid.isError && items.length === 0 ? (
                <EmptyState
                  variant="inline"
                  tone="error"
                  title="Couldn't load the Shop"
                  subtitle={grid.error instanceof Error ? grid.error.message : 'Please try again.'}
                  actions={[{ label: 'Retry', onClick: () => void grid.refetch() }]}
                />
              ) : items.length === 0 ? (
                narrowed ? (
                  <EmptyState
                    variant="inline"
                    title="No matches found"
                    subtitle="Try another search or explore what’s popular right now."
                    actions={[{ label: 'EXPLORE WEARABLES', onClick: exploreAll }]}
                  />
                ) : (
                  <EmptyState variant="inline" title="Nothing on sale right now" subtitle="Check back soon." />
                )
              ) : (
                <>
                  <div className={`${styles.grid} ${stale ? styles.stale : ''}`.trim()} aria-busy={stale}>
                    {items.map((item) => (
                      <ShopCard key={item.id} item={item} triedOn={tryingOn === item.id} lookKnown={lookKnown} onTryOn={() => toggleTryOn(item)} />
                    ))}
                  </div>
                  {grid.isFetchingNextPage && (
                    <div className={styles.more}>
                      <Spinner />
                    </div>
                  )}
                </>
              )}
            </div>
          </section>
        </div>
      </div>
    </MainMenuShell>
  )
}

function ShopCard({ item, triedOn, lookKnown, onTryOn }: { item: CatalogItem; triedOn: boolean; lookKnown: boolean; onTryOn: () => void }): React.JSX.Element {
  const creator = useProfile(item.creator)
  // Signed out until the Shop knows the buyer's wallet: credits for primary sales, MANA for resales.
  const price = cardPrice(item, SIGNED_OUT)
  const discount = price.kind === 'credits' && price.compareAt != null ? `-${saleDiscountPct(price.compareAt, price.credits)}%` : undefined
  const tryable = tryOnUrn(item) != null
  return (
    <ShopItemCard
      name={item.name}
      thumbnail={item.thumbnail}
      rarity={item.rarity}
      creator={creator?.name || shortAddr(item.creator)}
      creatorPicture={creator?.picture}
      price={<PriceLine price={price} />}
      discount={discount}
      categoryIcon={item.wearableCategory != null ? <CategoryIcon category={item.wearableCategory} size={16} /> : undefined}
      selected={triedOn}
      onSelect={tryable && lookKnown ? onTryOn : undefined}
      actions={
        tryable ? (
          <Button variant="secondary" size="sm" disabled={!lookKnown} title={lookKnown ? undefined : 'Loading your look…'} onClick={onTryOn}>
            {triedOn ? 'TAKE OFF' : 'TRY ON'}
          </Button>
        ) : undefined
      }
    />
  )
}

function PriceLine({ price }: { price: CardPrice }): React.JSX.Element {
  switch (price.kind) {
    case 'credits':
      return (
        <span title={`${formatCreditsFull(price.credits)} Credits`}>
          <CreditMark size={16} /> {formatCredits(price.credits)}
        </span>
      )
    case 'mana':
      return (
        <span title={`${formatMana(price.mana, { group: true })} MANA`}>
          <ManaIcon size={16} /> {formatMana(price.mana, { group: true, decimals: 2 })}
        </span>
      )
    case 'manaAtCheckout':
      return (
        <span>
          <ManaIcon size={16} /> MANA
        </span>
      )
    case 'creditsUnavailable':
      return <span>Price unavailable</span>
    case 'soldOut':
      return <span>Sold out</span>
    case 'notForSale':
      return <span>Not for sale</span>
  }
}
