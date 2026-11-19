import { lazy, Suspense, useEffect, useState } from 'react'
import { Link, useLocation } from 'react-router'
import { afterLoadIdle } from '@/data/lib/creators/idle'
import '../../styles/tokens.css'
import styles from './SubNav.module.css'

// The Curation tab needs the visitor's session and the committee list. It loads once the page is idle, so the
// tabs themselves cost a page nothing and are there from its first render.
// A chunk that fails to load (a flaky network, a deploy that replaced it) only leaves the tab out.
const CurationTab = lazy(() =>
  import('./CurationTab').then(
    m => ({ default: m.CurationTab }),
    () => ({ default: () => null })
  )
)

type Props = {
  /** On a page whose top bar overlays its content (the landing): the tabs take their place under it, on the section's violet field. */
  landing?: boolean
}

/** The Create section's tabs: its own pages, and the ones Sites serves beside them. */
export function SubNav({ landing = false }: Props) {
  const { pathname } = useLocation()
  const current = (path: string, exact = false) =>
    pathname === path || (!exact && pathname.startsWith(`${path}/`)) ? 'page' : undefined

  const [idle, setIdle] = useState(false)
  useEffect(() => afterLoadIdle(() => setIdle(true)), [])

  const tabs = (
    <div className={styles.subnav} data-testid="subnav">
      <nav className={styles.tabs} data-testid="subnav-tabs">
        <Link to="/create" aria-current={current('/create', true)}>
          Overview
        </Link>
        <Link to="/create/collections" aria-current={current('/create/collections')}>
          Collections
        </Link>
        {idle && (
          <Suspense fallback={null}>
            <CurationTab current={current('/create/curation')} />
          </Suspense>
        )}
        <Link to="/create/scenes">Scenes</Link>
        <Link to="/create/land">Land/World</Link>
      </nav>
    </div>
  )

  if (!landing) return tabs
  return (
    <>
      <div className={styles.field} aria-hidden />
      <div className={styles.topBarSpace} aria-hidden />
      {tabs}
      <div className={styles.heroOffset} aria-hidden />
    </>
  )
}
