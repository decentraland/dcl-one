import { createRoot } from 'react-dom/client'
import { useEffect, useRef, useState } from 'react'
import { TitleBar, type TitleBarSection } from '@dcl/one-ui'
import {
  GeneralGraphics,
  GeneralAudio,
  GeneralControls,
  GeneralChat,
  GeneralLobby,
  GeneralSecondaryBarWearables,
  GeneralSendEmote,
  GeneralCalendar,
  GeneralBell,
  GeneralHost,
  GeneralAll,
  GeneralName,
  GeneralFilters,
  GeneralDownArrow,
  GeneralSearchBold,
  ShopCart
} from '@dcl/one-ui/foundations'
import { BrowseIcon, VisitedIcon, FavoriteIcon, MyPlacesIcon } from './titlebar-place-icons'
import '../../foundations/foundations.css'
import styles from './titlebar-browser.module.css'
const requested = new URLSearchParams(location.search).get('variant')
const variant =
  requested === 'backpack' ||
  requested === 'events' ||
  requested === 'places' ||
  requested === 'shop'
    ? requested
    : 'settings'
const definitions: Record<typeof variant, readonly [string, string, React.ReactNode][]> = {
  backpack: [
    ['wearables', 'Wearables', <GeneralSecondaryBarWearables size={24} />],
    ['emotes', 'Emotes', <GeneralSendEmote size={24} />]
  ],
  settings: [
    ['graphics', 'Graphics', <GeneralGraphics size={24} />],
    ['audio', 'Audio', <GeneralAudio size={24} />],
    ['controls', 'Controls', <GeneralControls size={24} />],
    ['chat', 'Chat', <GeneralChat size={24} />],
    ['lobby', 'Lobby', <GeneralLobby size={24} />]
  ],
  events: [
    ['calendar', 'Calendar', <GeneralCalendar size={24} />],
    ['remind', 'Remind Me', <GeneralBell size={24} />],
    ['host', 'Host', <GeneralHost size={24} />]
  ],
  places: [
    ['explore', 'Explore', <BrowseIcon />],
    ['recent', 'Recent', <VisitedIcon />],
    ['favorites', 'Favorites', <FavoriteIcon />],
    ['my-places', 'My Places', <MyPlacesIcon />]
  ],
  shop: [
    ['all', 'All', <GeneralAll size={24} />],
    ['wearables', 'Wearables', <GeneralSecondaryBarWearables size={24} />],
    ['emotes', 'Emotes', <GeneralSendEmote size={24} />],
    ['names', 'Names', <GeneralName size={24} />]
  ]
}
const sections: readonly TitleBarSection[] = definitions[variant].map(([id, label, icon]) => ({
  id,
  label,
  icon
}))
type Disclosure = ReturnType<typeof useDisclosure>
/** One custom panel pattern for every fixture popup: the trigger toggles it, Escape closes it and returns
 * focus to the trigger, and tabbing or pressing outside both closes it. */
function useDisclosure(id: string, initial = false) {
  const [open, setOpen] = useState(initial)
  const trigger = useRef<HTMLButtonElement>(null)
  const panel = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!open) return
    const close = (event: PointerEvent) => {
      const target = event.target as Node
      if (!trigger.current?.contains(target) && !panel.current?.contains(target)) setOpen(false)
    }
    document.addEventListener('pointerdown', close)
    return () => document.removeEventListener('pointerdown', close)
  }, [open])
  const leaves = (next: EventTarget | null) =>
    next instanceof Node && !trigger.current?.contains(next) && !panel.current?.contains(next)
  return {
    open,
    trigger: {
      ref: trigger,
      type: 'button' as const,
      'aria-expanded': open,
      'aria-controls': open ? id : undefined,
      onClick: () => setOpen(!open),
      onBlur: (event: React.FocusEvent) => {
        if (leaves(event.relatedTarget)) setOpen(false)
      }
    },
    panel: {
      id,
      ref: panel,
      tabIndex: -1,
      role: 'group' as const,
      onBlur: (event: React.FocusEvent) => {
        if (leaves(event.relatedTarget)) setOpen(false)
      },
      onKeyDown: (event: React.KeyboardEvent) => {
        if (!event.defaultPrevented && event.key === 'Escape') {
          event.preventDefault()
          event.stopPropagation()
          setOpen(false)
          trigger.current?.focus()
        }
      }
    }
  }
}
function Choices({
  legend,
  name,
  options,
  value,
  onChange
}: {
  legend: string
  name: string
  options: readonly (readonly [string, string])[]
  value: string
  onChange: (value: string) => void
}) {
  return (
    <fieldset>
      <legend>{legend}</legend>
      {options.map(([option, label]) => (
        <label key={option}>
          <input
            type="radio"
            name={name}
            checked={value === option}
            onChange={() => onChange(option)}
          />
          {label}
        </label>
      ))}
    </fieldset>
  )
}
const sortOptions = [
  ['trending', 'Trending'],
  ['active', 'Most Active']
] as const
const collections = [
  ['Decentraland', 'Decentraland'],
  ['Fixture collection', 'Fixture collection']
] as const
function Floating({
  disclosure,
  end = false,
  children
}: {
  disclosure: Disclosure
  /** Align to the trigger's end edge, for a trigger that sits away from the viewport's start. */
  end?: boolean
  children: React.ReactNode
}) {
  return disclosure.open ? (
    <div className={[styles.floating, end && styles.end].filter(Boolean).join(' ')}>{children}</div>
  ) : null
}
function App() {
  const [activeId, setActiveId] = useState(variant === 'backpack' ? 'emotes' : sections[0]!.id)
  const [query, setQuery] = useState('')
  const filterPanel = useDisclosure(
    'fixture-filters',
    variant === 'places' && !new URLSearchParams(location.search).has('closed')
  )
  const shopFilterPanel = useDisclosure('fixture-shop-filters')
  const sortPanel = useDisclosure('fixture-sort')
  const collectionPanel = useDisclosure('fixture-collections')
  const [sort, setSort] = useState('trending')
  const [compatible, setCompatible] = useState(false)
  const [onSale, setOnSale] = useState(false)
  const [count, setCount] = useState(0)
  const [notice, setNotice] = useState('')
  const [collection, setCollection] = useState('Decentraland')
  const sortLabel = sortOptions.find(([value]) => value === sort)![1]
  const search = (
    <div className={styles.search}>
      <GeneralSearchBold size={20} aria-hidden="true" />
      <input
        aria-label="Fixture search"
        value={query}
        placeholder="Search"
        onChange={(event) => setQuery(event.target.value)}
      />
    </div>
  )
  const filters = (
    <button {...filterPanel.trigger} className={styles.control}>
      <GeneralFilters size={24} aria-hidden="true" />
      Filter &amp; Sort
      <GeneralDownArrow size={24} aria-hidden="true" />
    </button>
  )
  const compatibleView = (
    <fieldset>
      <legend>view</legend>
      <label>
        <input
          type="checkbox"
          checked={compatible}
          onChange={(event) => setCompatible(event.target.checked)}
        />
        Compatible Only
      </label>
    </fieldset>
  )
  const menu = filterPanel.open ? (
    <div {...filterPanel.panel} className={styles.popup} aria-label="Fixture sort and view">
      <Choices
        legend="sort by"
        name="fixture-sort"
        options={sortOptions}
        value={sort}
        onChange={setSort}
      />
      {compatibleView}
    </div>
  ) : null
  const common = { sections, activeId, onSelect: setActiveId }
  return (
    <main className={styles.canvas}>
      <p>
        Static local toolbar fixture. Supplied controls use mock local state, no API, wallet or
        mutation service. <span role="status">{notice}</span>
      </p>
      {variant === 'settings' ? (
        <TitleBar {...common} variant="settings" />
      ) : variant === 'backpack' ? (
        <TitleBar
          {...common}
          variant="backpack"
          controls={{
            collection: (
              <div className={[styles.collection, styles.filterHost].join(' ')}>
                <span aria-hidden="true">collection</span>
                <button
                  {...collectionPanel.trigger}
                  className={[styles.control, styles.choice].join(' ')}
                  aria-label={'Fixture collection, ' + collection}
                >
                  <span className={styles.value}>{collection}</span>
                  <GeneralDownArrow size={24} aria-hidden="true" />
                </button>
                <Floating disclosure={collectionPanel}>
                  <div
                    {...collectionPanel.panel}
                    className={styles.popup}
                    aria-label="Fixture collections"
                  >
                    <Choices
                      legend="collection"
                      name="fixture-collection"
                      options={collections}
                      value={collection}
                      onChange={setCollection}
                    />
                  </div>
                </Floating>
              </div>
            ),
            filters: (
              <div className={styles.filterHost}>
                {filters}
                <Floating disclosure={filterPanel}>{menu}</Floating>
              </div>
            ),
            search
          }}
        />
      ) : variant === 'events' ? (
        <TitleBar
          {...common}
          variant="events"
          controls={{
            date: (
              <button className={styles.date} onClick={() => setNotice('Today selected')}>
                Today
              </button>
            ),
            create: (
              <a className={styles.create} href="#fixture-create">
                create hangout
              </a>
            )
          }}
        />
      ) : variant === 'places' ? (
        <TitleBar {...common} variant="places" controls={{ filters, search, filterMenu: menu }} />
      ) : (
        <TitleBar
          {...common}
          variant="shop"
          controls={{
            filters: (
              <div className={styles.filterHost}>
                <button {...shopFilterPanel.trigger} className={styles.control}>
                  <GeneralFilters size={24} aria-hidden="true" />
                  Filters
                  <GeneralDownArrow size={24} aria-hidden="true" />
                </button>
                <Floating disclosure={shopFilterPanel}>
                  <div
                    {...shopFilterPanel.panel}
                    className={styles.popup}
                    aria-label="Fixture shop filters"
                  >
                    <fieldset>
                      <legend>price</legend>
                      <label>
                        <input
                          type="checkbox"
                          checked={onSale}
                          onChange={(event) => setOnSale(event.target.checked)}
                        />
                        On Sale
                      </label>
                    </fieldset>
                    {compatibleView}
                  </div>
                </Floating>
              </div>
            ),
            sort: (
              <div className={styles.filterHost}>
                <button
                  {...sortPanel.trigger}
                  className={styles.control}
                  aria-label={'Fixture shop sort, ' + sortLabel}
                >
                  <span className={styles.value}>{sortLabel}</span>
                  <GeneralDownArrow size={24} aria-hidden="true" />
                </button>
                <Floating disclosure={sortPanel} end>
                  <div
                    {...sortPanel.panel}
                    className={styles.popup}
                    aria-label="Fixture shop sort options"
                  >
                    <Choices
                      legend="sort by"
                      name="fixture-shop-sort"
                      options={sortOptions}
                      value={sort}
                      onChange={setSort}
                    />
                  </div>
                </Floating>
              </div>
            ),
            search,
            cart: (
              <button
                className={styles.cart}
                aria-label={'Fixture cart, ' + count}
                onClick={() => setCount(count + 1)}
              >
                <ShopCart size={24} aria-hidden="true" />
                <span>{count}</span>
              </button>
            )
          }}
        />
      )}
      <button id="outside">After toolbar</button>
    </main>
  )
}
createRoot(document.getElementById('root')!).render(<App />)
