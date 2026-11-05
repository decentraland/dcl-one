// In-world Shop: the main menu's Shop page. The avatar on the left is the engine's preview (the
// Backpack's), wearing what the buyer selects or tries on, never saved to the look; the grid on the right
// reads the web Shop's catalogue through TanStack (catalogueQuery), a page at a time, and prices each
// row by the web Shop's own card rule. Selecting an item opens its panel beside the grid. Buying is the
// next flow; resales and anything not sold in credits open on the web Shop.

import { useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useRef, useState } from 'react'
import { cardPrice } from '../../../shop/discovery/cardPrice'
import type { CatalogItem } from '../../../shop/discovery/lib/catalogItem'
import { defaultWorn, wornUrns } from '../../../shop/discovery/lib/outfit'
import type { BodyShapeUrn } from '../../../shop/detail/lib/bodyShape'
import { REMOTE_FLAG, useRemoteFlag } from '../../../shop/discovery/lib/remoteFlags'
import { saleDiscountPct } from '../../../shop/discovery/lib/sale'
import { itemUrn } from '../../../shop/detail/lib/urn'
import { CHAIN } from '../../../shop/model/chain'
import { SIGNED_OUT } from '../../../shop/model/settlement'
import { Button, Chip, ChevronDown, ChevronLeft, EmptyState, MaskIcon, OptionMenu, Pager, ScrollThumb, SearchField, ShopItemCard, ShopItemSkeleton, showConfirm, Spinner, Tabs, type TabItem } from '../../design'
import type { PreviewFocus, Wearable } from '../../engine/protocol'
import { siteUrl } from '../../lib/baseDomain'
import { shortAddr } from '../../lib/identity'
import { withUtm } from '../../lib/utm'
import { CategoryIcon } from '../backpack/categoryIcons'
import { EngineViewport } from '../engine/EngineViewport'
import { MainMenuShell } from '../menu/MainMenuShell'
import { useProfile } from '../session/profileStore'
import type { ProfileState, ShopState } from '../session/useEngineSession'
import allIcon from '../../assets/category-icons/all.png'
import upperBodyIcon from '../../assets/category-icons/upper_body.png'
import emotesIcon from '../../assets/backpack/icon-emotes.webp'
import filterIcon from '../../assets/backpack/icon-filter.webp'
import emptySearchArt from '../../assets/backpack/empty-search.webp'
import cartIcon from '../../assets/shop/cart.svg'
import externalIcon from '../../assets/shop/external.svg'
import { CATALOGUE_SORTS, categoriesFor, defaultFilters, pageCount, useShopCatalogue, type CatalogueFilters, type CatalogueKind } from './catalogueQuery'
import { buyableInWorld, PriceTag } from './PriceTag'
import { ShopFilters } from './ShopFilters'
import { ShopHome } from './ShopHome'
import { homeQueries, type OutfitSummary } from './homeQuery'
import { ShopItemPanel } from './ShopItemPanel'
import styles from './InWorldShop.module.css'

type Section = 'all' | 'wearables' | 'emotes'
const SECTIONS: TabItem<Section>[] = [
  { id: 'all', label: 'All', icon: <MaskIcon src={allIcon} size={24} /> },
  { id: 'wearables', label: 'Wearables', icon: <MaskIcon src={upperBodyIcon} size={24} /> },
  { id: 'emotes', label: 'Emotes', icon: <MaskIcon src={emotesIcon} size={24} /> }
]
const KIND: Record<Exclude<Section, 'all'>, CatalogueKind> = { wearables: 'wearable', emotes: 'emote' }
const SEARCH_DEBOUNCE_MS = 350

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

/** The item's page on the web Shop, where a resale or a MANA-only sale is bought. */
export const webShopUrl = (item: CatalogItem): string => withUtm(siteUrl(`/shop/item/${item.contractAddress.toLowerCase()}/${item.itemId ?? ''}`))

/** Asks first, then opens the item on the web Shop in the browser. */
async function openOnWeb(item: CatalogItem): Promise<void> {
  const url = webShopUrl(item)
  const go = await showConfirm({
    title: 'You will be redirected to the web Shop',
    body: (
      <>
        Continuing will open the link in your browser.
        <br />
        <span className={styles.link}>{url.split('?')[0]}</span>
      </>
    ),
    confirmLabel: 'CONTINUE',
    cancelLabel: 'CANCEL'
  })
  if (go) window.open(url, '_blank', 'noopener')
}

export default function InWorldShop({ shop, profile, onNavigate, setEngineViewport, equipped, preview, focus }: InWorldShopProps): React.JSX.Element | null {
  const [section, setSection] = useState<Section>('all')
  const queryClient = useQueryClient()
  const kind: CatalogueKind = section === 'emotes' ? 'emote' : 'wearable'
  const [filters, setFilters] = useState<CatalogueFilters>(() => defaultFilters('wearable'))
  const [draft, setDraft] = useState('')
  const [page, setPage] = useState(0)
  const [menu, setMenu] = useState<'sort' | 'filters' | null>(null)
  const [selected, setSelected] = useState<CatalogItem | null>(null)
  const [tryingOn, setTryingOn] = useState<string | null>(null)
  const scroller = useRef<HTMLDivElement>(null)
  const homeScroller = useRef<HTMLDivElement>(null)

  // Any change of what is listed starts again at its first page, at the top.
  const patch = useCallback((next: Partial<CatalogueFilters>) => {
    setFilters((f) => ({ ...f, ...next }))
    setPage(0)
  }, [])
  useEffect(() => {
    const t = setTimeout(() => patch({ search: draft.trim().toLowerCase() }), SEARCH_DEBOUNCE_MS)
    return () => clearTimeout(t)
  }, [draft, patch])
  useEffect(() => {
    if (scroller.current) scroller.current.scrollTop = 0
  }, [filters, page])

  const creatorSales = useRemoteFlag(REMOTE_FLAG.creatorSales)
  const grid = useShopCatalogue(filters, page, { creatorSales, enabled: shop.open && section !== 'all' })
  const items = grid.data?.items ?? []
  const total = grid.data?.total ?? 0

  // Only a preview this page made is undone: the Backpack shares the preview avatar.
  const previewing = useRef(false)
  const restoreLook = useCallback(() => {
    if (!previewing.current) return
    previewing.current = false
    preview(null)
  }, [preview])
  // Opening frames the whole avatar (the Backpack may have left it on the feet); closing gives the
  // avatar its own look back, and a reopen starts with nothing selected or tried on.
  useEffect(() => {
    if (shop.open) {
      focus('body')
      return
    }
    setSelected(null)
    setTryingOn(null)
    setMenu(null)
    restoreLook()
  }, [shop.open, focus, restoreLook])
  useEffect(() => restoreLook, [restoreLook])

  if (!shop.open) return null

  // The look arrives from the engine once asked for; until then a try-on would dress a bare avatar.
  const lookKnown = equipped.length > 0
  const wearOn = (item: CatalogItem): void => {
    const urn = tryOnUrn(item)
    if (urn == null || !lookKnown || item.category === 'emote') return
    setTryingOn(item.id)
    previewing.current = true
    preview([...equipped.filter((w) => w.category !== item.wearableCategory).map((w) => w.urn), urn])
  }
  const takeOff = (): void => {
    setTryingOn(null)
    restoreLook()
  }
  const toggleTryOn = (item: CatalogItem): void => (tryingOn === item.id ? takeOff() : wearOn(item))
  // An outfit dresses the avatar in what of it fits the player's body shape, over nothing else.
  const wearOutfit = async (outfit: OutfitSummary): Promise<void> => {
    if (!lookKnown) return
    const items = await queryClient.fetchQuery(homeQueries.outfitItems(outfit))
    const body = equipped.find((w) => w.category === 'body_shape')?.urn ?? null
    const shape = body as BodyShapeUrn | null
    setSelected(null)
    setTryingOn(`outfit:${outfit.id}`)
    previewing.current = true
    preview([...(body != null ? [body] : []), ...wornUrns(items, defaultWorn(items, shape), shape)])
  }
  const select = (item: CatalogItem): void => {
    setSelected(item)
    wearOn(item)
  }
  const closePanel = (): void => {
    setSelected(null)
    takeOff()
  }
  const pickSection = (next: Section): void => {
    setSection(next)
    setSelected(null)
    takeOff()
    setMenu(null)
    if (next !== 'all') {
      setFilters({ ...defaultFilters(KIND[next]), search: draft.trim().toLowerCase() })
      setPage(0)
    }
  }
  const viewAll = (row: 'trending' | 'newest'): void => {
    pickSection('wearables')
    setFilters({ ...defaultFilters('wearable'), status: 'on_sale', sort: 'newest', search: draft.trim().toLowerCase() })
    void row
  }
  const pickSort = (id: string): void => {
    const next = CATALOGUE_SORTS.find((s) => s.id === id)
    if (next != null) patch({ sort: next.id })
    setMenu(null)
  }
  const clearSearch = (): void => {
    setDraft('')
    patch({ search: '' })
  }
  const exploreAll = (): void => {
    setDraft('')
    patch({ ...defaultFilters(kind) })
  }

  const p = profile.data
  const sortLabel = CATALOGUE_SORTS.find((s) => s.id === filters.sort)?.label ?? CATALOGUE_SORTS[0].label
  // A new page or filter set shows the last rows, dimmed, until it answers.
  const stale = grid.isPlaceholderData
  // Only a narrowed feed has somewhere wider to go.
  const narrowed = JSON.stringify({ ...filters, sort: 'newest' }) !== JSON.stringify(defaultFilters(kind))
  const categories = categoriesFor(kind)
  const selectedPrice = selected != null ? cardPrice(selected, SIGNED_OUT) : null
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
          <Tabs variant="shop" items={SECTIONS} value={section} onChange={pickSection} aria-label="Shop sections" />
          <div className={styles.tools}>
            {section !== 'all' && (
              <>
                <div className={styles.menuWrap}>
                  <Button variant="light" className={styles.toolBtn} aria-expanded={menu === 'filters'} onClick={() => setMenu((m) => (m === 'filters' ? null : 'filters'))}>
                    <MaskIcon src={filterIcon} size={24} />
                    Filters
                    <ChevronDown size={18} />
                  </Button>
                  {menu === 'filters' && (
                    <ShopFilters className={styles.menu} filters={filters} onChange={patch} onClose={() => setMenu(null)} showDeals={creatorSales} showSmart={kind === 'wearable'} />
                  )}
                </div>
                <div className={styles.menuWrap}>
                  <Button variant="light" className={`${styles.toolBtn} ${styles.sortBtn}`} aria-label={`Sort by: ${sortLabel}`} aria-expanded={menu === 'sort'} onClick={() => setMenu((m) => (m === 'sort' ? null : 'sort'))}>
                    Sort by
                    <ChevronDown size={18} />
                  </Button>
                  {menu === 'sort' && (
                    <OptionMenu className={styles.menu} onClose={() => setMenu(null)} sections={[{ label: 'Sort by', options: CATALOGUE_SORTS, value: filters.sort, onChange: pickSort }]} />
                  )}
                </div>
              </>
            )}
            <div className={styles.searchWrap}>
              <SearchField variant="light" value={draft} onChange={setDraft} placeholder="Search" />
            </div>
            <button type="button" className={styles.cart} aria-label="Cart" disabled title="The cart is coming soon">
              <MaskIcon src={cartIcon} size={24} />
            </button>
          </div>
        </header>

        <div className={styles.stage}>
          {/* The engine draws the avatar preview here, seen through the backdrop's hole. */}
          <div className={styles.preview}>
            <EngineViewport region="avatarPreview" report={setEngineViewport} />
          </div>
          {grid.isPending && section !== 'all' && (
            <div className={styles.avatarLoading} role="status" aria-label="Loading the avatar">
              <Spinner size={72} />
            </div>
          )}
          <section className={styles.content} aria-label={section === 'all' ? 'Shop home' : section === 'emotes' ? 'Emotes' : 'Wearables'}>
            {section === 'all' ? (
              <div className={`${styles.body} ${styles.homeBody} ${selected != null ? styles.withPanel : ''}`.trim()}>
                <div className={styles.scrollWrap}>
                <div ref={homeScroller} className={styles.scroller}>
                  <ShopHome
                    enabled={shop.open}
                    creatorSales={creatorSales}
                    onOutfit={(o) => void wearOutfit(o)}
                    onViewAll={viewAll}
                    renderCard={(item) => (
                      <ShopCard
                        item={item}
                        selected={selected?.id === item.id}
                        triedOn={tryingOn === item.id}
                        lookKnown={lookKnown}
                        onSelect={() => select(item)}
                        onTryOn={() => toggleTryOn(item)}
                        onOpenOnWeb={() => void openOnWeb(item)}
                      />
                    )}
                  />
                </div>
                <ScrollThumb target={homeScroller} className={styles.thumb} />
                </div>
                {selected != null && selectedPrice != null && (
                  <ShopItemPanel key={selected.id} item={selected} price={selectedPrice} urn={tryOnUrn(selected)} onClose={closePanel} onOpenOnWeb={() => void openOnWeb(selected)} />
                )}
              </div>
            ) : (
            <>
            <div className={styles.chips} role="group" aria-label="Categories">
              {categories.map((c) => (
                <Chip
                  key={c.key}
                  variant="dark"
                  label={c.label}
                  icon={c.icon != null ? <MaskIcon src={c.icon} size={26} /> : undefined}
                  selected={c.key === filters.category}
                  onClick={() => patch({ category: c.key })}
                />
              ))}
            </div>
            {filters.search !== '' && (
              <div className={styles.results}>
                <button type="button" className={styles.back} aria-label="Exit search" onClick={clearSearch}>
                  <ChevronLeft size={18} />
                </button>
                <span>
                  Results for ‘{filters.search}’{grid.data != null && !stale ? ` (${total})` : ''}
                </span>
              </div>
            )}
            <div className={`${styles.body} ${selected != null ? styles.withPanel : ''}`.trim()}>
              <div className={styles.scrollWrap}>
              <div ref={scroller} className={styles.scroller}>
                {grid.isPending ? (
                  <div className={styles.grid} role="status" aria-label="Loading the catalogue">
                    {Array.from({ length: 18 }, (_, i) => (
                      <ShopItemSkeleton key={i} />
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
                      icon={<img src={emptySearchArt} alt="" width={72} height={72} />}
                      title="No matches found"
                      subtitle="Try another search or explore what’s popular right now."
                      actions={[{ label: `EXPLORE ${kind === 'emote' ? 'EMOTES' : 'WEARABLES'}`, onClick: exploreAll }]}
                    />
                  ) : (
                    <EmptyState variant="inline" title="Nothing on sale right now" subtitle="Check back soon." />
                  )
                ) : (
                  <>
                    <div className={`${styles.grid} ${stale ? styles.stale : ''}`.trim()} aria-busy={stale}>
                      {items.map((item) => (
                        <ShopCard
                          key={item.id}
                          item={item}
                          selected={selected?.id === item.id}
                          triedOn={tryingOn === item.id}
                          lookKnown={lookKnown}
                          onSelect={() => select(item)}
                          onTryOn={() => toggleTryOn(item)}
                          onOpenOnWeb={() => void openOnWeb(item)}
                        />
                      ))}
                    </div>
                    <Pager variant="shop" className={styles.pager} page={page} count={pageCount(total)} onChange={setPage} />
                  </>
                )}
              </div>
              <ScrollThumb target={scroller} className={styles.thumb} />
              </div>
              {selected != null && selectedPrice != null && (
                <ShopItemPanel key={selected.id} item={selected} price={selectedPrice} urn={tryOnUrn(selected)} onClose={closePanel} onOpenOnWeb={() => void openOnWeb(selected)} />
              )}
            </div>
            </>
            )}
          </section>
        </div>
      </div>
    </MainMenuShell>
  )
}

function ShopCard({
  item,
  selected,
  triedOn,
  lookKnown,
  onSelect,
  onTryOn,
  onOpenOnWeb
}: {
  item: CatalogItem
  selected: boolean
  triedOn: boolean
  lookKnown: boolean
  onSelect: () => void
  onTryOn: () => void
  onOpenOnWeb: () => void
}): React.JSX.Element {
  const creator = useProfile(item.creator)
  // Signed out until the Shop knows the buyer's wallet: credits for primary sales, MANA for resales.
  const price = cardPrice(item, SIGNED_OUT)
  const discount = price.kind === 'credits' && price.compareAt != null ? `-${saleDiscountPct(price.compareAt, price.credits)}%` : undefined
  const tryable = item.category !== 'emote' && tryOnUrn(item) != null
  const icon = item.category === 'emote' ? <MaskIcon src={emotesIcon} size={16} /> : item.wearableCategory != null ? <CategoryIcon category={item.wearableCategory} size={16} /> : undefined
  return (
    <ShopItemCard
      name={item.name}
      thumbnail={item.thumbnail}
      rarity={item.rarity}
      creator={creator?.name || shortAddr(item.creator)}
      creatorPicture={creator?.picture}
      price={<PriceTag price={price} size={15} />}
      discount={discount}
      categoryIcon={icon}
      selected={selected || triedOn}
      onSelect={onSelect}
      actions={
        <>
          {tryable && (
            <Button variant="secondary" size="card" className={styles.cardAction} disabled={!lookKnown} title={lookKnown ? undefined : 'Loading your look…'} onClick={onTryOn}>
              {triedOn ? 'TAKE OFF' : 'TRY ON'}
            </Button>
          )}
          {buyableInWorld(price) ? (
            <Button size="card" className={styles.cardAction} onClick={onSelect}>
              BUY
            </Button>
          ) : (
            <Button variant="secondary" size="card" className={styles.cardAction} onClick={onOpenOnWeb}>
              VIEW
              <MaskIcon src={externalIcon} size={20} />
            </Button>
          )}
        </>
      }
    />
  )
}
