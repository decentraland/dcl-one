import { createRoot } from 'react-dom/client'
import { useEffect, useRef, useState } from 'react'
import { Button, TitleBar, type TitleBarSection } from '@dcl/one-ui'
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
  GeneralAll,
  GeneralName,
  GeneralFilters,
  GeneralDownArrow,
  GeneralUpArrow,
  GeneralAdd,
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
const searchFocus =
  new URLSearchParams(location.search).get('searchfocus') === 'offset' ? 'offset' : undefined
const definitions: Record<typeof variant, readonly [string, string, React.ReactNode][]> = {
  backpack: [
    ['wearables', 'Wearables', <GeneralSecondaryBarWearables size={20} />],
    ['emotes', 'Emotes', <GeneralSendEmote size={20} />]
  ],
  settings: [
    ['graphics', 'Graphics', <GeneralGraphics size={20} />],
    ['audio', 'Audio', <GeneralAudio size={20} />],
    ['controls', 'Controls', <GeneralControls size={20} />],
    ['chat', 'Chat', <GeneralChat size={20} />],
    ['lobby', 'Lobby', <GeneralLobby size={20} />]
  ],
  events: [
    ['calendar', 'Calendar', <GeneralCalendar size={20} />],
    ['remind', 'Remind Me', <GeneralBell size={20} />]
  ],
  places: [
    ['explore', 'Explore', <BrowseIcon />],
    ['recent', 'Recent', <VisitedIcon />],
    ['favorites', 'Favorites', <FavoriteIcon />],
    ['my-places', 'My Places', <MyPlacesIcon />]
  ],
  shop: [
    ['all', 'All', <GeneralAll size={20} />],
    ['wearables', 'Wearables', <GeneralSecondaryBarWearables size={20} />],
    ['emotes', 'Emotes', <GeneralSendEmote size={20} />],
    ['names', 'Names', <GeneralName size={20} />]
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
      },
      onKeyDown: (event: React.KeyboardEvent) => {
        if (open && !event.defaultPrevented && event.key === 'Escape') {
          event.preventDefault()
          event.stopPropagation()
          setOpen(false)
        }
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
/** Figma draws general/down-arrow on a closed trigger; an open one points up. */
function Chevron({ open }: { open: boolean }) {
  return open ? (
    <GeneralUpArrow size={24} aria-hidden="true" />
  ) : (
    <GeneralDownArrow size={24} aria-hidden="true" />
  )
}
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
  const [sort, setSort] = useState('trending')
  const [compatible, setCompatible] = useState(false)
  const [onSale, setOnSale] = useState(false)
  const [count, setCount] = useState(0)
  const [notice, setNotice] = useState('')
  const sortLabel = sortOptions.find(([value]) => value === sort)![1]
  const search = (
    <div className={styles.search} data-focus={searchFocus}>
      <GeneralSearchBold className={styles.searchIcon} aria-hidden="true" />
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
      <GeneralFilters size={20} aria-hidden="true" />
      Filter &amp; Sort
      <Chevron open={filterPanel.open} />
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
              <Button
                variant="outline"
                size="md"
                href="#fixture-create"
                leadingIcon={<GeneralAdd size={20} aria-hidden="true" />}
              >
                create hangout
              </Button>
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
                  <GeneralFilters size={20} aria-hidden="true" />
                  Filters
                  <Chevron open={shopFilterPanel.open} />
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
                  aria-label={'Sort by, ' + sortLabel}
                >
                  <span className={styles.value}>Sort by</span>
                  <Chevron open={sortPanel.open} />
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
                <ShopCart size={30} aria-hidden="true" />
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
