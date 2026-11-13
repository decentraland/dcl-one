import { createRoot } from 'react-dom/client'
import { useRef, useState } from 'react'
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
function App() {
  const [activeId, setActiveId] = useState(variant === 'backpack' ? 'emotes' : sections[0]!.id)
  const [query, setQuery] = useState('')
  const [open, setOpen] = useState(
    variant === 'places' && !new URLSearchParams(location.search).has('closed')
  )
  const [sort, setSort] = useState('trending')
  const [compatible, setCompatible] = useState(false)
  const [count, setCount] = useState(0)
  const [notice, setNotice] = useState('')
  const [collection, setCollection] = useState('Decentraland')
  const filterRef = useRef<HTMLButtonElement>(null)
  const menuRef = useRef<HTMLDivElement>(null)
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
    <button
      ref={filterRef}
      onBlur={(event) => {
        if (event.relatedTarget && !menuRef.current?.contains(event.relatedTarget)) setOpen(false)
      }}
      className={styles.control}
      aria-expanded={open}
      aria-controls={open ? 'fixture-filters' : undefined}
      onClick={() => setOpen(!open)}
    >
      <GeneralFilters size={24} aria-hidden="true" />
      Filter &amp; Sort
      <GeneralDownArrow size={24} aria-hidden="true" />
    </button>
  )
  const menu = open ? (
    <div
      id="fixture-filters"
      ref={menuRef}
      tabIndex={-1}
      onBlur={(event) => {
        if (
          event.relatedTarget &&
          !event.currentTarget.contains(event.relatedTarget) &&
          event.relatedTarget !== filterRef.current
        )
          setOpen(false)
      }}
      className={styles.popup}
      role="group"
      aria-label="Fixture sort and view"
      onKeyDown={(event) => {
        if (!event.defaultPrevented && event.key === 'Escape') {
          event.preventDefault()
          event.stopPropagation()
          setOpen(false)
          filterRef.current?.focus()
        }
      }}
    >
      <fieldset>
        <legend>sort by</legend>
        {(
          [
            ['trending', 'Trending'],
            ['active', 'Most Active']
          ] as const
        ).map(([value, label]) => (
          <label key={value}>
            <input
              type="radio"
              name="fixture-sort"
              checked={sort === value}
              onChange={() => setSort(value)}
            />
            {label}
          </label>
        ))}
      </fieldset>
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
              <label className={styles.collection}>
                collection
                <select
                  aria-label="Fixture collection"
                  value={collection}
                  onChange={(event) => setCollection(event.target.value)}
                >
                  <option>Decentraland</option>
                  <option>Fixture collection</option>
                </select>
              </label>
            ),
            filters: (
              <div className={styles.filterHost}>
                {filters}
                {menu ? <div className={styles.floating}>{menu}</div> : null}
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
              <button className={styles.control} onClick={() => setNotice('Shop filter action')}>
                <GeneralFilters size={24} aria-hidden="true" />
                Filters
                <GeneralDownArrow size={24} aria-hidden="true" />
              </button>
            ),
            sort: (
              <select
                className={styles.control}
                aria-label="Fixture shop sort"
                value={sort}
                onChange={(event) => setSort(event.target.value)}
              >
                <option value="trending">sort by</option>
                <option value="active">Most Active</option>
              </select>
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
