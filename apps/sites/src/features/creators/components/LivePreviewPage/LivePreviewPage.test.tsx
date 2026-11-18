import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { TranslationProvider } from '~/intl'
import { type Session } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { type ValidationContext, type ValidationSource } from '~/lib/validation'
import { useWallet } from '~/store/wallet'

// ui2 lazy-loads MUI and talks to a real iframe; the page's own behaviour is what's under test.
vi.mock('~/components/WearablePreview', () => ({
  WearablePreview: ({ id }: { id: string }) => (
    <iframe id={id} src="https://wearable-preview.decentraland.zone/?x=1" title="preview" />
  ),
  createWearablePreviewController: vi.fn(() => ({
    emote: {
      events: { on: vi.fn(), off: vi.fn() },
      play: vi.fn(),
      pause: vi.fn(),
      stop: vi.fn(),
      hasSound: vi.fn().mockResolvedValue(false)
    },
    scene: {},
    physics: { setSpringBonesParams: vi.fn().mockResolvedValue(undefined) }
  })),
  EmoteControls: () => <div data-testid="ui2-emote-controls" />
}))
vi.mock('~/lib/featureFlags', async () => {
  const actual = await vi.importActual<typeof import('~/lib/featureFlags')>('~/lib/featureFlags')
  const mock = await import('~/test/featureFlags')
  return { ...actual, getIsFeatureEnabled: mock.getIsFeatureEnabled }
})
vi.mock('~/lib/catalyst', () => ({ fetchBaseWearables: vi.fn().mockResolvedValue([]) }))
vi.mock('~/lib/analytics', () => ({ track: vi.fn(), errorCode: () => 'unknown' }))
vi.mock('~/lib/monitoring', () => ({ captureError: vi.fn(), setMonitoringTags: vi.fn() }))
vi.mock('~/lib/connectionSpeed', () => ({ measureDownlinkMbps: () => Promise.resolve(null) }))

const validate = vi.fn(async (_source: ValidationSource, _ctx: ValidationContext) => ({ issues: [] }))
vi.mock('~/lib/validation', async () => {
  const actual = await vi.importActual<typeof import('~/lib/validation')>('~/lib/validation')
  return { ...actual, getValidator: () => ({ validate }) }
})

const addItemsProps: Array<Record<string, unknown>> = []
vi.mock('~/components/CollectionDetailPage/AddItemsModal', () => ({
  AddItemsModal: (props: Record<string, unknown>) => {
    addItemsProps.push(props)
    return <div data-testid="add-items-modal" />
  }
}))

const draft: Collection = {
  id: 'c1',
  name: 'Pirate Hats',
  owner: '0xabc',
  urn: 'urn:c1',
  isPublished: false,
  isApproved: false,
  itemCount: 0,
  minters: [],
  managers: [],
  createdAt: 1,
  updatedAt: 1
}
vi.mock('~/hooks/useCollections', () => ({ useDraftCollections: () => ({ data: [draft], isLoading: false }) }))

const { LivePreviewPage } = await import('./LivePreviewPage')

type Bridge = { version: number; type: 'wearable' | 'emote'; category: string; down?: boolean }

function serve(bridge: Bridge) {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string | URL) => {
      if (bridge.down) throw new TypeError('Failed to fetch')
      if (String(input).includes('/state')) {
        return new Response(
          JSON.stringify({ version: bridge.version, type: bridge.type, name: 'Hat', category: bridge.category })
        )
      }
      return new Response(new Uint8Array([1, 2, 3]))
    })
  )
}

function stubMatchMedia(matches: boolean) {
  vi.stubGlobal(
    'matchMedia',
    vi.fn(() => ({ matches, addEventListener: vi.fn(), removeEventListener: vi.fn() }))
  )
}

function renderPage(search = '') {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      <TranslationProvider>
        <MemoryRouter initialEntries={[`/live-preview${search}`]}>
          <Routes>
            <Route path="/live-preview" element={children} />
            <Route path="/collections/:id" element={<div data-testid="collection-page" />} />
          </Routes>
        </MemoryRouter>
      </TranslationProvider>
    </QueryClientProvider>
  )
  return render(<LivePreviewPage />, { wrapper })
}

beforeEach(() => {
  addItemsProps.length = 0
  validate.mockClear()
  stubMatchMedia(false)
  useWallet.setState({ session: { address: '0xabc' } as unknown as Session, restored: true })
})
afterEach(() => vi.unstubAllGlobals())

describe('LivePreviewPage', () => {
  it('streams the export onto the avatar and validates it with the bridge category', async () => {
    serve({ version: 1, type: 'wearable', category: 'hat' })
    renderPage('?bridge=8081')
    expect(screen.getByTestId('live-preview-bridge-url')).toHaveValue('http://localhost:8081')

    await screen.findByTestId('avatar-preview')
    expect(screen.getByTestId('live-preview-status')).toHaveAttribute('data-status', 'connected')
    expect(screen.getByTestId('live-preview-updated')).toBeInTheDocument()
    expect(screen.getByTestId('live-preview-updated')).not.toHaveAttribute('data-up-to-date')

    await userEvent.click(screen.getByTestId('live-preview-refresh'))
    await waitFor(() => expect(screen.getByTestId('live-preview-updated')).toHaveAttribute('data-up-to-date'))
    expect(screen.getByTestId('live-preview-updated')).toHaveTextContent(/up to date/i)
    await waitFor(() => expect(validate).toHaveBeenCalled())
    const [source, ctx] = validate.mock.calls[0]
    expect(source).toMatchObject({ kind: 'blob', mainFile: 'model.glb' })
    expect(ctx).toMatchObject({ type: 'wearable', category: 'hat', hides: [] })
    expect(screen.getByTestId('live-preview-category')).toHaveTextContent('Hat')
  })

  it('re-validates when the creator overrides the category', async () => {
    serve({ version: 1, type: 'wearable', category: 'hat' })
    renderPage()
    await screen.findByTestId('avatar-preview')
    await waitFor(() => expect(validate).toHaveBeenCalled())

    const user = userEvent.setup()
    await user.click(screen.getByTestId('live-preview-category'))
    await user.click(await screen.findByTestId('live-preview-category-option-mask'))
    await waitFor(() =>
      expect(validate.mock.calls[validate.mock.calls.length - 1]?.[1]).toMatchObject({ category: 'mask' })
    )
  })

  it('shows the emote controls for a streamed emote', async () => {
    serve({ version: 1, type: 'emote', category: '' })
    renderPage()
    await screen.findByTestId('avatar-preview')
    expect(screen.getByTestId('live-preview-animation')).toBeInTheDocument()
    expect(screen.queryByTestId('live-preview-properties')).not.toBeInTheDocument()
    expect(screen.getByTestId('ui2-emote-controls')).toBeInTheDocument()
  })

  it('explains a blocked local network connection and offers a retry', async () => {
    Object.defineProperty(navigator, 'permissions', {
      value: { query: vi.fn().mockResolvedValue({ state: 'denied', addEventListener() {}, removeEventListener() {} }) },
      configurable: true
    })
    Object.defineProperty(window, 'location', {
      value: { ...window.location, hostname: 'builder.example.com', reload: vi.fn() },
      configurable: true
    })
    serve({ version: 1, type: 'wearable', category: 'hat', down: true })
    renderPage()
    const card = await screen.findByTestId('live-preview-permission')
    expect(card).toHaveAttribute('data-state', 'denied')
    expect(screen.getByTestId('live-preview-permission-retry')).toBeInTheDocument()
    expect(screen.getByTestId('live-preview-error')).toBeInTheDocument()
  })

  it('hands the streamed model and its tuning to the add-items flow, then lands on the collection', async () => {
    serve({ version: 1, type: 'wearable', category: 'hat' })
    renderPage()
    await screen.findByTestId('avatar-preview')

    const user = userEvent.setup()
    await user.click(screen.getByTestId('live-preview-add'))
    await user.click(screen.getByTestId('live-preview-pick-target'))
    await user.click(await screen.findByTestId('live-preview-pick-target-option-c1'))
    await user.click(screen.getByTestId('live-preview-pick-confirm'))

    await screen.findByTestId('add-items-modal')
    const props = addItemsProps[addItemsProps.length - 1]
    expect(props.collection).toBe(draft)
    expect((props.files as File[])[0].name).toBe('model.glb')
    expect(props.prefill).toEqual({ category: 'hat', hides: [], springBoneParams: undefined })

    ;(props.onClose as () => void)()
    await screen.findByTestId('collection-page')
  })

  it('prefills the play mode when adding a streamed emote', async () => {
    serve({ version: 1, type: 'emote', category: '' })
    renderPage()
    await screen.findByTestId('avatar-preview')
    const user = userEvent.setup()
    await user.click(screen.getByTestId('live-preview-loop'))
    await user.click(screen.getByTestId('live-preview-add'))
    await user.click(screen.getByTestId('live-preview-pick-target'))
    await user.click(await screen.findByTestId('live-preview-pick-target-option-c1'))
    await user.click(screen.getByTestId('live-preview-pick-confirm'))
    await screen.findByTestId('add-items-modal')
    expect(addItemsProps[addItemsProps.length - 1].prefill).toEqual({ playMode: 'simple' })
  })

  it('asks to sign in before adding when there is no session', async () => {
    useWallet.setState({ session: null, restored: true })
    serve({ version: 1, type: 'wearable', category: 'hat' })
    renderPage()
    await screen.findByTestId('avatar-preview')
    await userEvent.setup().click(screen.getByTestId('live-preview-add'))
    expect(await screen.findByTestId('live-preview-sign-in')).toBeInTheDocument()
  })

  it('shows only the preview and a desktop hint on a phone', async () => {
    stubMatchMedia(true)
    serve({ version: 1, type: 'wearable', category: 'hat' })
    renderPage()
    await screen.findByTestId('avatar-preview')
    expect(screen.getByTestId('live-preview-mobile')).toBeInTheDocument()
    expect(screen.getByTestId('live-preview-mobile-hint')).toBeInTheDocument()
    expect(screen.queryByTestId('live-preview-page')).not.toBeInTheDocument()
    expect(screen.queryByTestId('live-preview-category')).not.toBeInTheDocument()
  })
})
