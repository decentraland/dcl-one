import { beforeEach, describe, expect, it, vi } from 'vitest'
import { type ReactNode } from 'react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen, waitFor } from '@testing-library/react'
import { TranslationProvider } from '~/intl'
import { type Session } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { type Item } from '~/lib/items'
import { type ThirdParty } from '~/lib/thirdParty'
import { LinkedApprovalFlowModal } from './LinkedApprovalFlowModal'

const withVideo = [
  { id: 'i1', name: 'Gold Hat' },
  { id: 'i2', name: 'Silver Hat' }
] as Item[]

const pending = [
  {
    id: 'i1',
    name: 'Gold Hat',
    urn: 'urn:decentraland:amoy:collections-thirdparty:brand:hats:gold',
    data: { category: 'hat', representations: [] },
    contents: { 'a.glb': 'Qma' }
  },
  {
    id: 'i2',
    name: 'Silver Hat',
    urn: 'urn:decentraland:amoy:collections-thirdparty:brand:hats:silver',
    data: { category: 'hat', representations: [] },
    contents: { 'b.glb': 'Qmb' }
  }
] as unknown as Item[]

const initialFlow = () => ({
  view: { kind: 'error', step: 'has_video', failed: [], items: withVideo } as Record<string, unknown>,
  plan: null as Record<string, unknown> | null,
  steps: [] as string[],
  running: false,
  start: vi.fn(),
  runRoot: vi.fn(),
  runDeploy: vi.fn(),
  stop: vi.fn()
})
const flow = initialFlow()

beforeEach(() => Object.assign(flow, initialFlow()))

vi.mock('~/hooks/useLinkedReview', () => ({ useLinkedApprovalFlow: () => flow }))
vi.mock('~/lib/builder', async importOriginal => ({
  ...(await importOriginal<typeof import('~/lib/builder')>()),
  fetchContentSize: vi.fn(async (hash: string) => (hash === 'Qma' ? 2.5 * 1024 * 1024 : 512 * 1024))
}))

const wrapper = ({ children }: { children: ReactNode }) => (
  <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
    <TranslationProvider>{children}</TranslationProvider>
  </QueryClientProvider>
)

function renderModal() {
  return render(
    <LinkedApprovalFlowModal
      session={{ address: '0xcurator' } as unknown as Session}
      collection={{ id: 'c1', name: 'Brand Hats' } as Collection}
      thirdParty={{ id: 'urn:tp', isApproved: true } as ThirdParty}
      review={{ pendingIds: new Set(), reviewedHashes: new Map() }}
      onClose={vi.fn()}
    />,
    { wrapper }
  )
}

describe('LinkedApprovalFlowModal', () => {
  it('names the items whose preview video blocks the approval, before anything goes on chain', () => {
    renderModal()
    const dialog = screen.getByTestId('approval-has-video')
    expect(dialog).toHaveTextContent('Gold Hat, Silver Hat')
    expect(dialog).toHaveTextContent('Nothing was put on chain')
  })

  it('asks the curator to switch the wallet back when it sits on another account', () => {
    flow.view = { kind: 'error', step: 'root', failed: [], wallet: true }
    renderModal()
    expect(screen.getByTestId('approval-error-description')).toHaveTextContent(/switch it back to the account/i)
    expect(screen.getByTestId('approval-retry')).toBeInTheDocument()
  })

  it('lists the item identifiers on the root step and the file sizes on the upload step', async () => {
    flow.plan = { pendingItems: pending, slots: [], approvalData: { cheque: { qty: 0 } } }
    flow.steps = ['root', 'deploy']
    flow.view = { kind: 'step', step: 'root', phase: { kind: 'idle' } }
    const { unmount } = renderModal()
    expect(screen.getByTestId('approval-step-root')).toHaveTextContent('gold')
    expect(screen.queryByTestId('approval-item-size')).toBeNull()
    unmount()

    flow.view = { kind: 'step', step: 'deploy', phase: { kind: 'idle' } }
    renderModal()
    await waitFor(() =>
      expect(screen.getAllByTestId('approval-item-size').map(size => size.textContent)).toEqual(['2.5MB', '512.0KB'])
    )
    expect(screen.getByTestId('approval-step-deploy')).not.toHaveTextContent('gold')
  })
})
