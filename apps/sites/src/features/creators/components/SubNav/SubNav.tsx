import { lazy, Suspense, useEffect, useState } from 'react'
import { Link, useLocation } from 'react-router'
import { TabsBar, type TabsBarItem, type TabsBarLinkProps } from '@dcl/one-ui'
import { afterLoadIdle } from '@/data/lib/creators/idle'
import '../../styles/tokens.css'
import styles from './SubNav.module.css'

// The Curation tab needs the visitor's session and the committee list. It loads once the page is idle, so the
// tabs themselves cost a page nothing and are there from its first render.
// A chunk that fails to load (a flaky network, a deploy that replaced it) only leaves the tab out.
const CurationAvailability = lazy(() =>
  import('./CurationAvailability').then(
    m => ({ default: m.CurationAvailability }),
    () => ({ default: () => null })
  )
)

// Each creators page mounts its own tabs: once the first page went idle, later ones show the tab from their first render
// instead of hiding it again until the next idle callback.
let idleReached = false

const overview = { id: 'overview', href: '/create', label: 'Overview' }
const collections = { id: 'collections', href: '/create/collections', label: 'Collections' }
const curation = { id: 'curation', href: '/create/curation', label: 'Curation' }
const landWorld = { id: 'land-world', href: '/download/creator-hub', label: 'Land/World' }
const publicTabs: readonly TabsBarItem[] = [overview, collections, landWorld]
const curatorTabs: readonly TabsBarItem[] = [overview, collections, curation, landWorld]

function RouteLink({ href, ...props }: TabsBarLinkProps) {
  return <Link to={href} {...props} />
}

type Props = {
  /** On a page whose top bar overlays its content (the landing): the tabs take their place under it, on the section's violet field. */
  landing?: boolean
}

/** The Create section's tabs: its own pages, and the ones Sites serves beside them. */
export function SubNav({ landing = false }: Props) {
  const { pathname } = useLocation()
  const [isCurator, setIsCurator] = useState(false)
  const items = isCurator ? curatorTabs : publicTabs
  const activeId = items.find(({ href }) =>
    pathname === href || (href !== overview.href && pathname.startsWith(`${href}/`))
  )?.id ?? null

  const [idle, setIdle] = useState(idleReached)
  useEffect(() => {
    if (idleReached) return
    return afterLoadIdle(() => {
      idleReached = true
      setIdle(true)
    })
  }, [])

  const tabs = (
    <div className={styles.subnav} data-testid="subnav">
      {idle && (
        <Suspense fallback={null}>
          <CurationAvailability onChange={setIsCurator} />
        </Suspense>
      )}
      <TabsBar
        mode="navigation"
        page="create"
        label="Creator Hub sections"
        items={items}
        activeId={activeId}
        LinkComponent={RouteLink}
      />
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
