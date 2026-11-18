import { lazy, Suspense, useEffect, useSyncExternalStore, type ReactNode } from 'react'
import SitesChrome from '@ui/web/frames/SitesChrome'
import { ErrorBoundary } from '@/features/creators/components/ErrorBoundary'
import { MaintenancePage } from '@/features/creators/components/MaintenancePage'
import { SubNav } from '@/features/creators/components/SubNav'
import { Toasts } from '@/features/creators/components/Toasts'
import { useFeatureFlag } from '@/features/creators/hooks/useFeatureFlag'
import { useWallet } from '@/features/creators/store/wallet'
import { FeatureFlag } from '@/data/lib/creators/featureFlags'
import NotFoundPage from '@/features/not-found/NotFoundPage'
import '@/features/creators/styles/index.css'
import styles from './CreatorsShell.module.css'

const PAGES = {
  collections: lazy(() => import('@/features/creators/components/CollectionsPage').then(m => ({ default: m.CollectionsPage }))),
  collection: lazy(() =>
    import('@/features/creators/components/CollectionDetailPage').then(m => ({ default: m.CollectionDetailPage }))
  ),
  editor: lazy(() => import('@/features/creators/components/ItemEditorPage').then(m => ({ default: m.ItemEditorPage }))),
  curation: lazy(() => import('@/features/creators/components/CurationPage').then(m => ({ default: m.CurationPage }))),
  livePreview: lazy(() => import('@/features/creators/components/LivePreviewPage').then(m => ({ default: m.LivePreviewPage })))
}

export type CreatorsPage = keyof typeof PAGES

// Pages that take the whole viewport, without the site's top bar and footer.
const FULLSCREEN: readonly CreatorsPage[] = ['editor', 'livePreview']

const Loading = () => (
  <div className={styles.loading} aria-busy="true">
    <span className="spinner" aria-hidden />
  </div>
)

const noop = () => () => undefined

/** One of the creators pages inside the site: its chrome, the section tabs, and what every page shares. */
export default function CreatorsShell({ page }: { page: CreatorsPage }) {
  // The pages read the visitor's session and wallet, which only the browser has: the server renders the frame.
  const inBrowser = useSyncExternalStore(
    noop,
    () => true,
    () => false
  )
  const maintenance = useFeatureFlag(FeatureFlag.MAINTENANCE)
  const livePreview = useFeatureFlag(FeatureFlag.BLENDER_LIVE_PREVIEW, { enabled: page === 'livePreview' })
  const restore = useWallet(state => state.restore)
  useEffect(() => restore(), [restore])

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
