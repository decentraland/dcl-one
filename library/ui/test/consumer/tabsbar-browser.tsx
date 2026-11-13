import { createRoot } from 'react-dom/client'
import { useState } from 'react'
import { TabsBar } from '@dcl/one-ui'
import { GeneralFavorites, GeneralHistory, ShopCart } from '@dcl/one-ui/foundations'
import '../../foundations/foundations.css'
import styles from './tabsbar-browser.module.css'
const pageQuery = new URLSearchParams(location.search).get('page')
const page = pageQuery === 'shop' || pageQuery === 'learn' ? pageQuery : 'create'
const labels =
  page === 'create'
    ? ['Overview', 'Collections', 'Scenes', 'Land', 'Worlds']
    : page === 'shop'
      ? ['Overview', 'Campaign', 'Collectibles', 'My items', 'My store']
      : ['All articles', 'About Decentraland', 'Announcements', 'Community highlights']
const items = labels.map((label, index) => ({
  id: String(index),
  label,
  href: '#' + index,
  hiddenOnMobile: page === 'shop' && index === 1
}))
function App() {
  const [query, setQuery] = useState('')
  const [count, setCount] = useState(0)
  const [activeId, setActiveId] = useState(
    new URLSearchParams(location.search).get('active') ?? '0'
  )
  const common = { items, activeId, onSelect: setActiveId, label: 'Fixture sections' }
  return (
    <main className={styles.canvas}>
      <p>
        Static local composition fixture. Controls are supplied by this fixture, not migrated
        Searchbar or Shop actions.
      </p>
      {page === 'create' ? (
        <TabsBar {...common} page={page} />
      ) : (
        <TabsBar
          {...common}
          page={page}
          search={
            <input
              className={styles.search}
              aria-label="Fixture search"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
            />
          }
          actions={
            page === 'learn' ? (
              <button className={styles.sort}>Sort by</button>
            ) : (
              <>
                <button className={styles.credits}>Buy credits</button>
                <button className={styles.icon} aria-label="Fixture favorites">
                  <GeneralFavorites size={24} aria-hidden="true" />
                </button>
                <button className={styles.icon} aria-label="Fixture activity">
                  <GeneralHistory size={24} aria-hidden="true" />
                </button>
                <button
                  className={styles.icon}
                  aria-label={'Fixture cart, ' + count}
                  onClick={() => setCount(count + 1)}
                >
                  <ShopCart size={24} aria-hidden="true" />
                  {count}
                </button>
              </>
            )
          }
        />
      )}
      <button id="outside">After composition</button>
    </main>
  )
}
createRoot(document.getElementById('root')!).render(<App />)
