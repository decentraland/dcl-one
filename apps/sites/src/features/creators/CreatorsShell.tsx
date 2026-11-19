import { lazy, Suspense, useEffect, useLayoutEffect, useSyncExternalStore, type ReactNode } from 'react'
import SitesChrome from '@ui/web/frames/SitesChrome'
import { ErrorBoundary } from './components/ErrorBoundary'
import { MaintenancePage } from './components/MaintenancePage'
import { SubNav } from './components/SubNav'
import { Toasts } from './components/Toasts'
import { useFailureReports } from './hooks/useFailureReports'
import { useFeatureFlag } from './hooks/useFeatureFlag'
import { useWallet } from './store/wallet'
import { setTrackContext } from '@/data/lib/creators/analytics'
import { FeatureFlag } from '@/data/lib/creators/featureFlags'
import NotFoundPage from '../not-found/NotFoundPage'
import './styles/index.css'
import styles from './CreatorsShell.module.css'

const PAGES = {
  collections: lazy(() => import('./components/CollectionsPage').then(m => ({ default: m.CollectionsPage }))),
  collection: lazy(() => import('./components/CollectionDetailPage').then(m => ({ default: m.CollectionDetailPage }))),
  editor: lazy(() => import('./components/ItemEditorPage').then(m => ({ default: m.ItemEditorPage }))),
  curation: lazy(() => import('./components/CurationPage').then(m => ({ default: m.CurationPage }))),
  livePreview: lazy(() => import('./components/LivePreviewPage').then(m => ({ default: m.LivePreviewPage })))
}

export type CreatorsPage = keyof typeof PAGES

const STORIES: Record<CreatorsPage, string> = {
  collections: 'creator-hub/collections',
  collection: 'creator-hub/collection',
  editor: 'creator-hub/item-editor',
  curation: 'creator-hub/curation',
  livePreview: 'creator-hub/live-preview'
}

// Pages that take the whole viewport, without the site's top bar and footer.
const FULLSCREEN: readonly CreatorsPage[] = ['editor', 'livePreview']

const Loading = () => (
  <div className={styles.loading} aria-busy="true">
    <span className="spinner" aria-hidden />
  </div>
)

const noop = () => () => undefined

/** One of the creators pages inside the site: its chrome, the section tabs, and what every page shares. */
export default function CreatorsShell({ page, sid }: { page: CreatorsPage; sid: string }) {
  // The pages read the visitor's session and wallet, which only the browser has: the server renders the frame.
  const inBrowser = useSyncExternalStore(
    noop,
    () => true,
    () => false
  )
  const maintenance = useFeatureFlag(FeatureFlag.MAINTENANCE)
  const livePreview = useFeatureFlag(FeatureFlag.BLENDER_LIVE_PREVIEW, { enabled: page === 'livePreview' })
  useFailureReports()
  const restore = useWallet(state => state.restore)
  useEffect(() => restore(), [restore])
  // A layout effect, so the page's own effects already report under its story.
  useLayoutEffect(() => {
    setTrackContext({ sid, story: STORIES[page] })
    return () => setTrackContext(null)
  }, [page, sid])

  const gated = page === 'livePreview' && !livePreview.enabled
  const fullscreen = FULLSCREEN.includes(page) && !maintenance.enabled && !gated
  const Page = PAGES[page]

  let content: ReactNode
  if (!inBrowser || (gated && livePreview.isLoading)) content = <Loading />
  else if (maintenance.enabled) content = <MaintenancePage />
  else if (gated) content = <NotFoundPage />
  else content = <Page />

  const body = (
    <div className={styles.root} data-creators data-fullscreen={fullscreen || undefined}>
      {!fullscreen && <SubNav />}
      <main className={styles.page}>
        <ErrorBoundary>
          <Suspense fallback={<Loading />}>{content}</Suspense>
        </ErrorBoundary>
      </main>
      <Toasts />
    </div>
  )

  return fullscreen ? body : <SitesChrome active="create">{body}</SitesChrome>
}
