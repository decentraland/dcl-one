import { StrictMode } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
vi.mock('@/features/creators/hooks/useFeatureFlag', () => ({ useFeatureFlag: () => ({ enabled: false, isLoading: false }) }))
vi.mock('@/features/creators/hooks/useAccountWatcher', () => ({ useAccountWatcher: () => undefined }))
vi.mock('@/features/creators/store/wallet', () => ({
  useWallet: (selector: (s: { restore: () => Promise<void> }) => unknown) =>
    selector({ restore: () => Promise.resolve() })
}))
vi.mock('@/features/creators/components/NavBar', () => ({ NavBar: () => <div data-testid="navbar" /> }))
vi.mock('@/features/creators/components/Footer', () => ({ Footer: () => <footer data-testid="footer" /> }))
vi.mock('@/features/creators/components/Intercom', () => ({ Intercom: () => null }))
vi.mock('@/features/creators/components/Toasts', () => ({ Toasts: () => null }))
vi.mock('@/features/creators/components/OverviewPage', () => ({ OverviewPage: () => <div data-testid="overview" /> }))
vi.mock('@/features/creators/components/CollectionsPage', () => ({ CollectionsPage: () => <div data-testid="collections" /> }))
vi.mock('@/features/creators/components/ItemEditorPage', () => ({ ItemEditorPage: () => <div data-testid="editor" /> }))
vi.mock('@/data/lib/creators/analytics', () => ({ track: vi.fn() }))
const trackPageView = vi.hoisted(() => vi.fn())
vi.mock('@/data/lib/creators/pageViews', () => ({ trackPageView }))

const { App } = await import('./App')

function renderApp(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <App />
    </MemoryRouter>
  )
}

beforeEach(() => {
  trackPageView.mockReset()
})

describe('App', () => {
  it('opens a direct link to an inner page in the shell and counts its view', async () => {
    renderApp('/collections')
    await screen.findByTestId('collections')
    expect(screen.getByTestId('navbar')).toBeInTheDocument()
    expect(screen.getByTestId('footer')).toBeInTheDocument()
    await waitFor(() => expect(trackPageView).toHaveBeenCalledWith('/collections'))
    expect(trackPageView).toHaveBeenCalledTimes(1)
  })

  it('renders and counts the overview at once', () => {
    renderApp('/')
    expect(screen.getByTestId('overview')).toBeInTheDocument()
    expect(screen.getByTestId('navbar')).toBeInTheDocument()
    expect(trackPageView).toHaveBeenCalledWith('/')
  })

  it('shows the not-found page for unknown routes, and for the live preview with its flag off', async () => {
    renderApp('/nope')
    expect(await screen.findByTestId('not-found-page')).toBeInTheDocument()

    renderApp('/live-preview')
    await waitFor(() => expect(screen.getAllByTestId('not-found-page')).toHaveLength(2))
  })

  it('opens the item editor as a fullscreen workspace without the shell', async () => {
    renderApp('/collections/editor')
    await screen.findByTestId('editor')
    expect(screen.queryByTestId('navbar')).toBeNull()
    expect(screen.queryByTestId('footer')).toBeNull()
    expect(document.body).toHaveAttribute('data-fullscreen')
  })

  it('counts a page once under StrictMode', () => {
    render(
      <StrictMode>
        <MemoryRouter initialEntries={['/']}>
          <App />
        </MemoryRouter>
      </StrictMode>
    )
    expect(trackPageView).toHaveBeenCalledTimes(1)
  })
})
