// In-world Shop: the main menu's Shop page. The avatar on the left is the engine's preview (the
// Backpack's), wearing what the buyer selects or tries on, never saved to the look; the grid on the right
// reads the web Shop's catalogue through TanStack (catalogueQuery), a page at a time, and prices each
// row by the web Shop's own card rule. Selecting an item opens its panel beside the grid. BUY and BUY ITEM
// buy a primary sale with Credits through the web shell, which holds the wallet and shows the purchase
// dialog (purchaseQuery); resales, anything not sold in Credits, and buying outside the shell open on the
// web Shop. ADD TO CART keeps such a sale in the in-world cart (cartStore), whose popover under the
// header's cart button buys them all at once through the same dialog.

import { useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'
import type { CatalogItem } from '../../../shop/discovery/lib/catalogItem'
import { defaultWorn, wornUrns } from '../../../shop/discovery/lib/outfit'
import { BASE_FEMALE, BASE_MALE, type BodyShapeUrn } from '../../../shop/detail/lib/bodyShape'
import { REMOTE_FLAG } from '../../../shop/discovery/lib/remoteFlags'
import { Button, Chip, ChevronDown, ChevronLeft, EmptyState, MaskIcon, OptionMenu, Pager, ScrollThumb, SearchField, showConfirm, showToast, Spinner, Tabs, type TabItem } from '../../design'
import type { PreviewFocus, Wearable } from '../../engine/protocol'
import { registerCancelLayer } from '../../lib/cancelLayers'
import { subscribeHudScale } from '../../lib/hudScale'
import { EngineViewport } from '../engine/EngineViewport'
import { MainMenuShell } from '../menu/MainMenuShell'
import type { ProfileState, ShopState } from '../session/useEngineSession'
import allIcon from '../../assets/category-icons/all.png'
import upperBodyIcon from '../../assets/category-icons/upper_body.png'
import emotesIcon from '../../assets/backpack/icon-emotes.webp'
import filterIcon from '../../assets/backpack/icon-filter.webp'
import emptySearchArt from '../../assets/backpack/empty-search.webp'
import cartIcon from '../../assets/shop/cart.svg'
import { cartRequestLines, cartUnits, inWorldCart, inWorldCartInputFor, useInWorldCart } from './cartStore'
import { CATALOGUE_SORTS, categoriesFor, defaultFilters, pageCount, useShopCatalogue, type CatalogueFilters, type CatalogueKind } from './catalogueQuery'
import { useShopFlag } from './flagsQuery'
import { homeQueries, useNewest, useShopOutfits, useTrending, type OutfitSummary } from './homeQuery'
import { tryOnUrn, webShopUrl } from './itemLinks'
import { useCreditsBalance, useShopCapabilities, useShopPurchase } from './purchaseQuery'
import { categoryGlyph, CreatorName, SelectedItemPanel, ShopCard } from './ShopItems'
import { CreditsPill, EmoteControls, ShopCart, ShopCartBadge, ShopCartToast, ShopFilters, ShopHome, ShopItemSkeleton, type PressedWith } from './ui'
import styles from './InWorldShop.module.css'

type Section = 'all' | 'wearables' | 'emotes'
const SECTIONS: TabItem<Section>[] = [
  { id: 'all', label: 'All', icon: <MaskIcon src={allIcon} size={24} /> },
  { id: 'wearables', label: 'Wearables', icon: <MaskIcon src={upperBodyIcon} size={24} /> },
  { id: 'emotes', label: 'Emotes', icon: <MaskIcon src={emotesIcon} size={24} /> }
]
const KIND: Record<Exclude<Section, 'all'>, CatalogueKind> = { wearables: 'wearable', emotes: 'emote' }
const SEARCH_DEBOUNCE_MS = 350
/** The design's scroll thumb: 203px, whatever the page's length. */
const THUMB_LENGTH = 203
/** The stage image (assets/shop/stage.jpg) is the design's 1920×1080 frame, its top 70px under the top bar. */
const STAGE = { width: 1920, height: 1080, top: 70 }
/** How long "1 Item Added to Cart" stays up. */
const TOAST_MS = 3000
type Rect = { x: number; y: number; width: number; height: number }

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
  /** Where the page draws its stage image, for the engine to draw the same behind the avatar; null restores
   *  the Backpack's backdrop. */
  stage: (rect: Rect | null) => void
  /** Plays an emote on the preview avatar (again on every call); null stops it. */
  emote: (urn: string | null) => void
  /** No engine draws the avatar (the UI-only mock): the stage shows whole, without the avatar's window. */
  noEngine?: boolean
  /** Emotes tried on on the avatar (TRY ON, playing on select, STOP EMOTE). Off until the engine plays a
   *  preview avatar's emote by URN: the message reaches the scene, the avatar doesn't animate yet. */
  emoteTryOn?: boolean
}

/** Asks first, then opens the item on the web Shop in the browser. */
async function openOnWeb(item: CatalogItem): Promise<void> {
  const url = webShopUrl(item)
  if (url == null) return
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

export default function InWorldShop({ shop, profile, onNavigate, setEngineViewport, equipped, preview, focus, stage, emote, noEngine = false, emoteTryOn = false }: InWorldShopProps): React.JSX.Element | null {
  // The Shop opens on the wearables catalogue, as the design's every frame of this flow does.
  const [section, setSection] = useState<Section>('wearables')
  const queryClient = useQueryClient()
  const kind: CatalogueKind = section === 'emotes' ? 'emote' : 'wearable'
  const [filters, setFilters] = useState<CatalogueFilters>(() => defaultFilters('wearable'))
  const [draft, setDraft] = useState('')
  const [page, setPage] = useState(0)
  const [menu, setMenu] = useState<'sort' | 'filters' | null>(null)
  const [selected, setSelected] = useState<CatalogItem | null>(null)
  const [tryingOn, setTryingOn] = useState<string | null>(null)
  // The emote playing on the avatar (an item id), which STOP EMOTE stops.
  const [playing, setPlaying] = useState<string | null>(null)
  const [cartOpen, setCartOpen] = useState(false)
  // Each ADD TO CART shows the toast again (its key restarts it).
  const [toast, setToast] = useState(0)
  const cartArea = useRef<HTMLDivElement>(null)
  const cartButton = useRef<HTMLButtonElement>(null)
  const backdrop = useRef<HTMLDivElement>(null)
  const scroller = useRef<HTMLDivElement>(null)
  const homeScroller = useRef<HTMLDivElement>(null)

  // Any change of what is listed starts again at its first page, at the top. From the All home, a
  // filter or a sort is a way into the wearables catalogue.
  const patch = useCallback((next: Partial<CatalogueFilters>) => {
    setFilters((f) => ({ ...f, ...next }))
    setPage(0)
    setSection((s) => (s === 'all' ? 'wearables' : s))
  }, [])
  useEffect(() => {
    const search = draft.trim().toLowerCase()
    const t = setTimeout(() => {
      // Only a typed search leaves the All home; the debounce's first run (an empty box) doesn't.
      setFilters((f) => (f.search === search ? f : { ...f, search }))
      setPage(0)
      if (search !== '') setSection((s) => (s === 'all' ? 'wearables' : s))
    }, SEARCH_DEBOUNCE_MS)
    return () => clearTimeout(t)
  }, [draft])
  useEffect(() => {
    if (scroller.current) scroller.current.scrollTop = 0
  }, [filters, page])

  // The listings wait for the creator-sales answer (or its failure): read before it, they'd come without
  // their sale prices and be read again once it lands.
  const creatorSalesFlag = useShopFlag(REMOTE_FLAG.creatorSales)
  const flagsKnown = creatorSalesFlag !== undefined
  const creatorSales = creatorSalesFlag ?? false
  const grid = useShopCatalogue(filters, page, { creatorSales, enabled: shop.open && flagsKnown && section !== 'all' })
  const onHome = shop.open && section === 'all'
  const outfits = useShopOutfits(onHome)
  const trending = useTrending(creatorSales, onHome && flagsKnown)
  const newest = useNewest(creatorSales, onHome && flagsKnown)
  // Escape closes the filters first, as the HUD's other popovers (OptionMenu registers its own).
  useEffect(() => (menu === 'filters' ? registerCancelLayer(() => setMenu(null)) : undefined), [menu])
  const items = grid.data?.items ?? []
  const total = grid.data?.total ?? 0
  const capabilities = useShopCapabilities()
  const credits = useCreditsBalance(shop.open ? (capabilities?.buyer ?? null) : null)
  const purchase = useShopPurchase(() => onNavigate('backpack'))
  // A guest login's cart is the signed-out one, which the account takes over once one signs in.
  const cart = useInWorldCart(capabilities?.reason === 'guest' ? null : (capabilities?.buyer ?? null))
  // Escape or the cart's X closes it, the focus back on the cart button; a press anywhere else closes it
  // where the press went.
  const closeCart = useCallback((refocus: boolean) => {
    setCartOpen(false)
    if (refocus) cartButton.current?.focus()
  }, [])
  useEffect(() => {
    if (!cartOpen) return
    const away = (e: PointerEvent): void => {
      if (!cartArea.current?.contains(e.target as Node)) closeCart(false)
    }
    document.addEventListener('pointerdown', away, true)
    const unregister = registerCancelLayer(() => closeCart(true))
    return () => {
      document.removeEventListener('pointerdown', away, true)
      unregister()
    }
  }, [cartOpen, closeCart])
  useEffect(() => {
    if (toast === 0) return
    const t = setTimeout(() => setToast(0), TOAST_MS)
    return () => clearTimeout(t)
  }, [toast])
  // The panel shows the selected item as the listings now read it (its stock after a purchase), and its
  // own copy only while no listing holds it.
  const shown = selected == null ? null : ([...items, ...(trending.data ?? []), ...(newest.data ?? [])].find((i) => i.id === selected.id) ?? selected)

  // Only a preview this page made is undone: the Backpack shares the preview avatar.
  const previewing = useRef(false)
  // Every preview this page starts takes the next ticket, and closing, taking off or starting another
  // retires the one before: an outfit whose items answer late applies only if its ticket is still current.
  const previewTicket = useRef(0)
  const restoreLook = useCallback(() => {
    previewTicket.current++
    if (!previewing.current) return
    previewing.current = false
    preview(null)
  }, [preview])
  // Likewise only an emote this page played is stopped.
  const emoting = useRef(false)
  const stopEmote = useCallback(() => {
    setPlaying(null)
    if (!emoting.current) return
    emoting.current = false
    emote(null)
  }, [emote])
  // Opening frames the whole avatar (the Backpack may have left it on the feet); closing gives the
  // avatar its own look back, and a reopen starts with nothing selected, tried on or playing.
  useEffect(() => {
    if (shop.open) {
      focus('body')
      return
    }
    setSelected(null)
    setTryingOn(null)
    setMenu(null)
    setCartOpen(false)
    setToast(0)
    restoreLook()
    stopEmote()
  }, [shop.open, focus, restoreLook, stopEmote])
  useEffect(() => restoreLook, [restoreLook])
  useEffect(() => stopEmote, [stopEmote])

  // The engine draws the stage image behind the avatar where the page draws it, so the avatar's window
  // shows no seam: the backdrop's top-left, the image 70px above it, at the reference canvas's scale.
  useLayoutEffect(() => {
    const el = backdrop.current
    if (!shop.open || noEngine || el == null) return
    const push = (): void => {
      const r = el.getBoundingClientRect()
      const scale = el.offsetHeight > 0 ? r.height / el.offsetHeight : 1
      stage({ x: r.left, y: r.top - STAGE.top * scale, width: STAGE.width * scale, height: STAGE.height * scale })
    }
    push()
    const ro = new ResizeObserver(push)
    ro.observe(el)
    const unsubscribe = subscribeHudScale(push)
    return () => {
      ro.disconnect()
      unsubscribe()
      stage(null)
    }
  }, [shop.open, noEngine, stage])

  if (!shop.open) return null

  // The look arrives from the engine once asked for; until then a try-on would dress a bare avatar.
  const lookKnown = equipped.length > 0
  const wearOn = (item: CatalogItem): void => {
    const urn = tryOnUrn(item)
    if (urn == null || !lookKnown || item.category === 'emote') return
    previewTicket.current++
    setTryingOn(item.id)
    previewing.current = true
    preview([...equipped.filter((w) => w.category !== item.wearableCategory).map((w) => w.urn), urn])
  }
  const takeOff = (): void => {
    setTryingOn(null)
    restoreLook()
  }
  // An emote is tried on by playing it on the avatar, over whatever it wears.
  const playEmote = (item: CatalogItem): void => {
    const urn = tryOnUrn(item)
    if (urn == null || !emoteTryOn) return
    setPlaying(item.id)
    emoting.current = true
    emote(urn)
  }
  const toggleTryOn = (item: CatalogItem): void => {
    if (item.category === 'emote') playing === item.id ? stopEmote() : playEmote(item)
    else tryingOn === item.id ? takeOff() : wearOn(item)
  }
  // An outfit dresses the avatar in what of it fits the player's body shape, over nothing else.
  const wearOutfit = async (outfit: OutfitSummary): Promise<void> => {
    if (!lookKnown) return
    const ticket = ++previewTicket.current
    let items: CatalogItem[]
    try {
      items = await queryClient.fetchQuery(homeQueries.outfitItems(outfit))
    } catch {
      // The outfit's items didn't load: the avatar keeps what it wears.
      return
    }
    // Closed, taken off or replaced by another preview meanwhile.
    if (ticket !== previewTicket.current) return
    const body = equipped.find((w) => w.category === 'body_shape')?.urn ?? null
    // The engine's URN in whatever case it came; the outfit's fit is checked against the canonical one.
    const shape: BodyShapeUrn | null = body == null ? null : /basefemale$/i.test(body) ? BASE_FEMALE : BASE_MALE
    setSelected(null)
    setTryingOn(`outfit:${outfit.id}`)
    previewing.current = true
    preview([...(body != null ? [body] : []), ...wornUrns(items, defaultWorn(items, shape), shape)])
  }
  // In the web shell the purchase dialog opens over the page; anywhere else the web Shop sells it.
  const buy = (item: CatalogItem, input: PressedWith): void => {
    // A purchase that couldn't open here (the shell refused it or didn't answer) is offered on the web.
    if (purchase.available) void purchase.buy(item, input).then((opened) => (opened ? undefined : openOnWeb(item)))
    else void openOnWeb(item)
  }
  // The cart is bought where purchases run (the web shell); elsewhere ADD TO CART stays off.
  const addToCart = (item: CatalogItem): (() => void) | undefined => {
    const input = purchase.available ? inWorldCartInputFor(item) : null
    if (input == null || !inWorldCart.canAdd(input)) return undefined
    return () => {
      void inWorldCart.add(input).then((added) => {
        if (added) setToast((n) => n + 1)
      })
    }
  }
  const checkout = (input: PressedWith): void => {
    setCartOpen(false)
    void purchase.checkout(cartRequestLines(cart), input).then((opened) => {
      if (!opened) showToast("The purchase couldn't open. Try again in a moment.", { tone: 'error' })
    })
  }
  const select = (item: CatalogItem): void => {
    // Whatever is picked now, an outfit still loading is no longer wanted.
    previewTicket.current++
    setSelected(item)
    if (item.category === 'emote') playEmote(item)
    else wearOn(item)
  }
  const closePanel = (): void => {
    setSelected(null)
    takeOff()
    stopEmote()
  }
  const enterSection = (next: Section): void => {
    setSection(next)
    setSelected(null)
    takeOff()
    stopEmote()
    setMenu(null)
    // The All home's filters and sort lead into the wearables catalogue, so it keeps theirs.
    setFilters({ ...defaultFilters(next === 'all' ? 'wearable' : KIND[next]), search: draft.trim().toLowerCase() })
    setPage(0)
  }
  // The tab already open keeps its filters, page, panel and try-on when clicked again.
  const pickSection = (next: Section): void => {
    if (next !== section) enterSection(next)
  }
  // As on the web Shop, both rows lead to the catalogue: New keeps its order (on sale, newest first);
  // Trending has no order of its own there, so it opens the catalogue as it stands by default.
  const viewAll = (row: 'trending' | 'newest'): void => {
    enterSection('wearables')
    const search = draft.trim().toLowerCase()
    setFilters(row === 'newest' ? { ...defaultFilters('wearable'), status: 'on_sale', sort: 'newest', search } : { ...defaultFilters('wearable'), search })
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
        <div ref={backdrop} className={`${styles.backdrop} ${noEngine ? '' : styles.cutout}`.trim()} aria-hidden="true" />
        <header className={styles.head}>
          <h1 className={styles.title}>Shop</h1>
          <Tabs variant="shop" items={SECTIONS} value={section} onChange={pickSection} aria-label="Shop sections" />
          <div className={styles.tools}>
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
            <div className={styles.searchWrap}>
              <SearchField variant="light" value={draft} onChange={setDraft} placeholder="Search" />
            </div>
            {credits != null && <CreditsPill credits={credits} />}
            <div ref={cartArea} className={styles.cartArea}>
              <button
                ref={cartButton}
                type="button"
                className={styles.cart}
                aria-label={cart.length > 0 ? `Cart, ${cartUnits(cart)} ${cartUnits(cart) === 1 ? 'item' : 'items'}` : 'Cart'}
                aria-expanded={cartOpen}
                disabled={!purchase.available}
                title={purchase.available ? undefined : 'The cart is in the web Explorer'}
                onClick={() => {
                  // The cart says what was added: the toast goes as it opens.
                  setToast(0)
                  setCartOpen((o) => !o)
                }}
              >
                <MaskIcon src={cartIcon} size={24} />
              </button>
              {cart.length > 0 && <ShopCartBadge className={styles.cartBadge} count={cartUnits(cart)} />}
              {cartOpen && (
                <ShopCart
                  className={styles.cartPopover}
                  lines={cart.map((l) => ({
                    key: `${l.contract}:${l.itemId}`,
                    name: l.name,
                    creator: <CreatorName address={l.creator} />,
                    thumbnail: l.thumbnail,
                    rarity: l.rarity,
                    categoryIcon: categoryGlyph(l.category, 16),
                    quantity: l.quantity,
                    credits: l.credits,
                    canIncrement: inWorldCart.canAdd(l)
                  }))}
                  total={cart.reduce((n, l) => n + l.credits * l.quantity, 0)}
                  onClose={() => closeCart(true)}
                  onQuantity={(key, quantity) => {
                    const [contract, itemId] = key.split(':')
                    void inWorldCart.setQuantity({ contract, itemId }, quantity)
                  }}
                  onRemove={(key) => {
                    const [contract, itemId] = key.split(':')
                    void inWorldCart.remove({ contract, itemId })
                  }}
                  onCheckout={checkout}
                />
              )}
            </div>
          </div>
        </header>
        {/* Mounted while the page is, so a screen reader announces the toast as it lands in it. */}
        <div className={styles.toast} role="status" aria-live="polite">
          {toast > 0 && <ShopCartToast key={toast}>1 Item Added to Cart</ShopCartToast>}
        </div>

        <div className={styles.stage}>
          {/* The engine draws the avatar preview here, seen through the backdrop's hole. */}
          <div className={styles.preview}>
            <EngineViewport region="avatarPreview" report={setEngineViewport} />
          </div>
          {playing != null && (
            // The design's EMOTE controls, over the avatar while an emote plays.
            <EmoteControls className={styles.emoteControls} onStop={stopEmote} />
          )}
          {grid.isPending && section !== 'all' && (
            <div className={styles.avatarLoading} role="status" aria-label="Loading the avatar">
              <Spinner variant="shop" size={64} />
            </div>
          )}
          <section className={styles.content} aria-label={section === 'all' ? 'Shop home' : section === 'emotes' ? 'Emotes' : 'Wearables'}>
            {section === 'all' ? (
              <div className={`${styles.body} ${styles.homeBody} ${selected != null ? styles.withPanel : ''}`.trim()}>
                <div className={styles.scrollWrap}>
                <div ref={homeScroller} className={styles.scroller}>
                  <ShopHome
                    outfits={outfits.data}
                    // Loading until a row has items: its first read, and a retry after it failed.
                    trending={{ items: trending.data, pending: trending.data == null && (!flagsKnown || trending.fetchStatus === 'fetching'), error: trending.isError }}
                    newest={{ items: newest.data, pending: newest.data == null && (!flagsKnown || newest.fetchStatus === 'fetching'), error: newest.isError }}
                    onRetry={(row) => void (row === 'trending' ? trending : newest).refetch()}
                    onOutfit={(o) => void wearOutfit(o)}
                    onViewAll={viewAll}
                    renderCard={(item: CatalogItem) => (
                      <ShopCard
                        item={item}
                        selected={selected?.id === item.id}
                        triedOn={item.category === 'emote' ? playing === item.id : tryingOn === item.id}
                        lookKnown={lookKnown}
                        onSelect={() => select(item)}
                        onTryOn={() => toggleTryOn(item)}
                        onOpenOnWeb={() => void openOnWeb(item)}
                        onBuy={(input) => buy(item, input)}
                        emoteTryOn={emoteTryOn}
                      />
                    )}
                  />
                </div>
                <ScrollThumb target={homeScroller} length={THUMB_LENGTH} className={styles.thumb} />
                </div>
                {shown != null && <SelectedItemPanel key={shown.id} item={shown} onClose={closePanel} onOpenOnWeb={() => void openOnWeb(shown)} onBuy={(input) => buy(shown, input)} onAddToCart={addToCart(shown)} />}
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
                    // The server's own message (a database error…) means nothing to a buyer.
                    subtitle="Please try again in a moment."
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
                          triedOn={item.category === 'emote' ? playing === item.id : tryingOn === item.id}
                          lookKnown={lookKnown}
                          onSelect={() => select(item)}
                          onTryOn={() => toggleTryOn(item)}
                          onOpenOnWeb={() => void openOnWeb(item)}
                          onBuy={(input) => buy(item, input)}
                          emoteTryOn={emoteTryOn}
                        />
                      ))}
                    </div>
                    <Pager variant="shop" className={styles.pager} page={page} count={pageCount(total)} onChange={setPage} />
                  </>
                )}
              </div>
              <ScrollThumb target={scroller} length={THUMB_LENGTH} className={styles.thumb} />
              </div>
              {shown != null && <SelectedItemPanel key={shown.id} item={shown} onClose={closePanel} onOpenOnWeb={() => void openOnWeb(shown)} onBuy={(input) => buy(shown, input)} onAddToCart={addToCart(shown)} />}
            </div>
            </>
            )}
          </section>
        </div>
      </div>
    </MainMenuShell>
  )
}
