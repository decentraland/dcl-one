import { lazy, Suspense, useEffect, useRef } from 'react'
import { Navigate, Route, Routes, useLocation } from 'react-router-dom'
import { ErrorBoundary } from '@/features/creators/components/ErrorBoundary'
import { Footer } from '@/features/creators/components/Footer'
import { Intercom } from '@/features/creators/components/Intercom'
import { MaintenancePage } from '@/features/creators/components/MaintenancePage'
import { NavBar } from '@/features/creators/components/NavBar'
import { OverviewPage } from '@/features/creators/components/OverviewPage'
import { Toasts } from '@/features/creators/components/Toasts'
import { TranslationProvider } from '@/data/lib/creators/intl'
import { FeatureFlag } from '@/data/lib/creators/featureFlags'
import { trackPageView } from '@/data/lib/creators/pageViews'
import { setCanonical } from '@/data/lib/creators/seo'
import { useAccountWatcher } from '@/features/creators/hooks/useAccountWatcher'
import { useFeatureFlag } from '@/features/creators/hooks/useFeatureFlag'
import { useWallet } from '@/features/creators/store/wallet'

// The overview (the landing route) stays eager for the fastest first paint; every other route is code-split.
const CollectionsPage = lazy(() => import('@/features/creators/components/CollectionsPage').then(m => ({ default: m.CollectionsPage })))
const CollectionDetailPage = lazy(() =>
  import('@/features/creators/components/CollectionDetailPage').then(m => ({ default: m.CollectionDetailPage }))
)
const ItemEditorPage = lazy(() => import('@/features/creators/components/ItemEditorPage').then(m => ({ default: m.ItemEditorPage })))
const CurationPage = lazy(() => import('@/features/creators/components/CurationPage').then(m => ({ default: m.CurationPage })))
const LivePreviewPage = lazy(() => import('@/features/creators/components/LivePreviewPage').then(m => ({ default: m.LivePreviewPage })))
const NotFoundPage = lazy(() => import('@/features/creators/components/NotFoundPage').then(m => ({ default: m.NotFoundPage })))

const PageFallback = () => {
  return (
    <div className="page-loading" aria-busy="true">
      <span className="spinner" aria-hidden />
    </div>
  )
}

// Fullscreen workspaces: no navbar, no footer, no page scroll.
const FULLSCREEN_PATHS = ['/collections/editor', '/live-preview']

const App = () => {
  const location = useLocation()
  const maintenance = useFeatureFlag(FeatureFlag.MAINTENANCE)
  const livePreview = useFeatureFlag(FeatureFlag.BLENDER_LIVE_PREVIEW)
  const path = location.pathname.replace(/\/+$/, '')
  // With its flag off the live preview route is a not-found page, which keeps the shell.
  const isFullscreen =
    !maintenance.enabled && FULLSCREEN_PATHS.includes(path) && (path !== '/live-preview' || livePreview.enabled)

  // Auth bootstrap lives at the app root so the silent session restore (and the return from /auth)
  // doesn't depend on any layout component staying mounted.
  const restore = useWallet(state => state.restore)
  useEffect(() => {
    void restore()
  }, [restore])
  useAccountWatcher()

  // Pagination and in-page filters update the query string only; a new pathname is a new page.
  useEffect(() => {
    window.scrollTo({ top: 0 })
  }, [location.pathname])

  // One view per pathname, so StrictMode's effect replay doesn't count a page twice.
  const seenPathname = useRef<string>()
  useEffect(() => {
    if (seenPathname.current === location.pathname) return
    seenPathname.current = location.pathname
    trackPageView(location.pathname)
  }, [location.pathname])

  useEffect(() => {
    if (isFullscreen) document.body.dataset.fullscreen = ''
    else delete document.body.dataset.fullscreen
    // The flag belongs to this mount: an unmount (StrictMode, HMR) must not leave the shell hidden.
    return () => {
      delete document.body.dataset.fullscreen
    }
  }, [isFullscreen])

  useEffect(() => {
    setCanonical(path)
  }, [path])

  // Mirrored on <body> so shell-level CSS (the page field behind the fixed navbar) can vary per route.
  useEffect(() => {
    document.body.dataset.route = path || '/'
    return () => {
      delete document.body.dataset.route
    }
  }, [path])

  return (
    <TranslationProvider>
      {!isFullscreen && <NavBar />}
      {/* The route is exposed so a page can opt out of shell-level CSS by path if it ever needs to. */}
      <main className="page" data-route={location.pathname} data-fullscreen={isFullscreen || undefined}>
        <ErrorBoundary>
          {maintenance.enabled ? (
            <MaintenancePage />
          ) : (
            <Suspense fallback={<PageFallback />}>
              <Routes>
                <Route path="/" element={<OverviewPage />} />
                <Route path="/overview" element={<Navigate to="/" replace />} />
                <Route path="/collections" element={<CollectionsPage />} />
                <Route path="/collections/editor" element={<ItemEditorPage />} />
                <Route path="/collections/:collectionId" element={<CollectionDetailPage />} />
                <Route path="/curation" element={<CurationPage />} />
                <Route
                  path="/live-preview"
                  element={
                    livePreview.enabled ? (
                      <LivePreviewPage />
                    ) : livePreview.isLoading ? (
                      <PageFallback />
                    ) : (
                      <NotFoundPage />
                    )
                  }
                />
                <Route path="*" element={<NotFoundPage />} />
              </Routes>
            </Suspense>
          )}
        </ErrorBoundary>
      </main>
      {!isFullscreen && <Footer />}
      <Toasts />
      <Intercom />
    </TranslationProvider>
  )
}

export { App }
