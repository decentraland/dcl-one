import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { TranslationProvider } from '~/intl'
import { type ApprovalPlan, type ApprovalView } from '~/hooks/useApprovalFlow'
import { type Session } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { type CollectionCuration } from '~/lib/curation'
import { type Item } from '~/lib/items'
import { ApprovalFlowModal } from './ApprovalFlowModal'

// Read at render time, after the mock factory ran.
let view: ApprovalView
let plan: ApprovalPlan
let running = false
const state = vi.hoisted(() => ({
  start: vi.fn(),
  stop: vi.fn(),
  runRescue: vi.fn(),
  cancelSigning: vi.fn(),
  reloadItems: vi.fn()
}))
vi.mock('~/hooks/useApprovalFlow', () => ({
  useApprovalFlow: () => ({
    view,
    plan,
    running,
    start: state.start,
    stop: state.stop,
    runRescue: state.runRescue,
    runDeploy: vi.fn(),
    runApprove: vi.fn(),
    cancelSigning: state.cancelSigning,
    reloadItems: state.reloadItems
  })
}))
const storedSizes = vi.hoisted(() => new Map<string, number>([['Qmmodel', 819_100]]))
vi.mock('~/lib/builder', async importOriginal => ({
  ...(await importOriginal<typeof import('~/lib/builder')>()),
  fetchContentSize: vi.fn(async (hash: string) => {
    const size = storedSizes.get(hash)
    if (size === undefined) throw new Error('storage down')
    return size
  })
}))
vi.mock('~/lib/monitoring', () => ({ captureError: vi.fn() }))
vi.mock('~/components/CollectionMosaic', () => ({ CollectionMosaic: () => null }))
vi.mock('~/lib/analytics', () => ({ track: vi.fn() }))
const clipboard = vi.hoisted(() => ({ copyToClipboard: vi.fn() }))
vi.mock('~/lib/clipboard', () => clipboard)

const web3 = { address: '0xMe', providerType: 'injected' } as Session
const collection = { id: 'c1', name: 'Hats', itemCount: 6 } as Collection
const item = {
  id: 'i1',
  name: 'Pirate Hat',
  rarity: 'legendary',
  thumbnail: 'thumbnail.png',
  contents: {},
  data: { category: 'hat', representations: [] }
} as unknown as Item
const request = (assignee: string | null) =>
  ({ id: 'r1', collectionId: 'c1', status: 'pending', assignee, createdAt: 1, updatedAt: 1 }) as CollectionCuration

function renderModal(
  curation: CollectionCuration | null = request('0xme'),
  session: Session = web3,
  onClose = vi.fn()
) {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <TranslationProvider>
        <ApprovalFlowModal
          session={session}
          collection={collection}
          curation={curation}
          mode="approve"
          items={[]}
          onClose={onClose}
        />
      </TranslationProvider>
    </QueryClientProvider>
  )
}

beforeEach(() => {
  vi.clearAllMocks()
  view = { kind: 'loading' }
  running = false
  plan = { steps: [], rescue: [], deploy: [] }
})

describe('ApprovalFlowModal', () => {
  it('starts checking the collection right away', () => {
    renderModal()
    expect(state.start).toHaveBeenCalledTimes(1)
    expect(screen.getByTestId('approval-loading')).toBeInTheDocument()
  })

  it('lists the items to approve on chain, with a step per remaining transaction', () => {
    plan = { steps: ['rescue', 'approve'], rescue: [{ item, contentHash: 'bafkreiabcdefxyz123456' }], deploy: [] }
    view = { kind: 'step', step: 'rescue', phase: { kind: 'idle' } }
    renderModal()
    expect(screen.getByTestId('approval-steps-1')).toHaveAttribute('data-state', 'current')
    expect(screen.getByTestId('approval-steps-2')).toHaveAttribute('data-state', 'todo')
    expect(screen.getByTestId('approval-item-row')).toHaveTextContent('Pirate Hat')
    expect(screen.getByTestId('approval-item-row')).toHaveTextContent('bafkre...123456')

    fireEvent.click(screen.getByTestId('approval-rescue'))
    expect(state.runRescue).toHaveBeenCalled()
  })

  it('hides the stepper when the run has a single step, and shows each upload’s size', async () => {
    plan = { steps: ['deploy'], rescue: [], deploy: [{ ...item, contents: { 'm.glb': 'Qmmodel' } }] }
    view = { kind: 'step', step: 'deploy', phase: { kind: 'idle' } }
    renderModal()
    expect(screen.queryByTestId('approval-steps')).not.toBeInTheDocument()
    expect(screen.getByRole('dialog')).toHaveAccessibleName('Upload files')
    expect(await screen.findByText('799.9KB')).toBeInTheDocument()
  })

  it('says a size is unknown when it could not be measured, never a made-up one', async () => {
    plan = { steps: ['deploy'], rescue: [], deploy: [{ ...item, contents: { 'm.glb': 'Qmmissing' } }] }
    view = { kind: 'step', step: 'deploy', phase: { kind: 'idle' } }
    renderModal()
    expect(screen.getByRole('status', { name: 'Measuring size' })).toBeInTheDocument()
    await waitFor(() => expect(screen.getByTestId('approval-item-size')).toHaveTextContent('Unknown'))
  })

  it('shows the upload progress with no way to close it, and guards the page while it runs', () => {
    plan = { steps: ['deploy'], rescue: [], deploy: [item] }
    view = { kind: 'step', step: 'deploy', phase: { kind: 'uploading', done: 1, total: 3 } }
    renderModal()
    expect(screen.getByTestId('approval-uploading-bar')).toHaveAttribute('value', '1')
    expect(screen.getByTestId('approval-uploading-count')).toHaveTextContent('1/3')
    expect(screen.queryByTestId('approval-uploading-close')).not.toBeInTheDocument()

    const leaving = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(leaving)
    expect(leaving.defaultPrevented).toBe(true)
  })

  it('waits on the wallet prompt with a way back to the step', () => {
    plan = { steps: ['approve'], rescue: [], deploy: [] }
    view = { kind: 'step', step: 'approve', phase: { kind: 'signing', tx: 1, txs: 1 } }
    renderModal()
    fireEvent.click(screen.getByTestId('approval-signing-cancel'))
    expect(state.cancelSigning).toHaveBeenCalled()
  })

  it('skips the wallet prompt screen for social logins, which sign without one', () => {
    plan = { steps: ['approve'], rescue: [], deploy: [] }
    view = { kind: 'step', step: 'approve', phase: { kind: 'signing', tx: 1, txs: 1 } }
    renderModal(undefined, { address: '0xMe', providerType: 'magic' } as Session)
    expect(screen.queryByTestId('approval-signing')).not.toBeInTheDocument()
    expect(screen.getByTestId('approval-step-approve')).toBeInTheDocument()
  })

  it('shows the raw failure so the curator can copy it for support', async () => {
    view = { kind: 'error', step: 'deploy', detail: null, failed: [{ item, message: 'Catalyst said no' }] }
    renderModal()
    expect(screen.getByTestId('approval-error-detail')).toHaveTextContent('Pirate Hat: Catalyst said no')

    clipboard.copyToClipboard.mockResolvedValue(true)
    fireEvent.click(screen.getByTestId('approval-error-copy'))
    expect(clipboard.copyToClipboard).toHaveBeenCalledWith('Pirate Hat: Catalyst said no')
    await waitFor(() => expect(screen.getByTestId('approval-error-copy')).toHaveAttribute('data-copied'))

    fireEvent.click(screen.getByTestId('approval-retry'))
    expect(state.start).toHaveBeenCalledTimes(2)
  })

  it('asks the curator to switch the wallet back, with nothing to copy, when it sits on another account', () => {
    view = { kind: 'error', step: 'approve', detail: null, failed: [], wallet: true }
    renderModal()
    expect(screen.getByTestId('approval-error-description')).toHaveTextContent(/switch it back to the account/i)
    expect(screen.queryByTestId('approval-error-detail')).not.toBeInTheDocument()
    fireEvent.click(screen.getByTestId('approval-retry'))
    expect(state.start).toHaveBeenCalledTimes(2)
  })

  it('sends the curator back to the latest items when the creator changed them mid-review', () => {
    view = { kind: 'error', step: 'changed', detail: null, failed: [] }
    const onClose = vi.fn()
    renderModal(undefined, undefined, onClose)
    fireEvent.click(screen.getByTestId('approval-changed-review'))
    expect(state.reloadItems).toHaveBeenCalled()
    expect(onClose).toHaveBeenCalled()
  })

  it('can’t be closed while a request the curator backed out of is still open in the wallet', () => {
    plan = { steps: ['approve'], rescue: [], deploy: [] }
    view = { kind: 'step', step: 'approve', phase: { kind: 'idle' } }
    running = true
    renderModal()
    expect(screen.getByTestId('approval-flow-modal-close')).toBeDisabled()
    expect(screen.getByTestId('approval-approve')).toBeEnabled()
  })
})
