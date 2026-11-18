import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { TranslationProvider } from '~/intl'
import { type Session } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { type Item } from '~/lib/items'
import { openExternal } from '~/lib/navigation'
import { ValidationSeverity, type ValidationIssue } from '~/lib/validation'
import { type CollectionCuration } from '~/lib/curation'
import { type CollectionEvent } from '~/lib/events'
import { ItemSyncStatus } from '~/lib/itemSync'
import { ReviewBar } from './ReviewBar'

const state = vi.hoisted(() => ({
  curation: null as CollectionCuration | null,
  curationError: false,
  curationLoading: false,
  refetch: vi.fn(),
  events: null as CollectionEvent[] | null,
  eventsStatus: 'success',
  autoCuration: true,
  flagLoading: false,
  syncs: new Map<string, { status: string; entity?: object; lookupFailed?: true }>(),
  reject: vi.fn(),
  disable: vi.fn(),
  issues: new Map<string, ValidationIssue[]>(),
  isValidating: false,
  selectItem: vi.fn(),
  track: vi.fn()
}))
vi.mock('~/lib/navigation', () => ({
  openExternal: vi.fn(),
  isWebUrl: (url?: string) => /^https?:\/\//.test(url ?? '')
}))
vi.mock('~/lib/analytics', () => ({ track: state.track }))
vi.mock('~/hooks/useCuration', () => ({
  useCollectionCuration: () => ({
    data: state.curation,
    isLoading: state.curationLoading,
    isError: state.curationError,
    refetch: state.refetch
  }),
  useRejectCuration: () => ({ mutate: state.reject, isPending: false, isError: false, reset: vi.fn() }),
  useDisableCollection: () => ({ mutate: state.disable, isPending: false, isError: false, reset: vi.fn() })
}))
vi.mock('~/hooks/useCollectionEvents', () => ({
  useCollectionEvents: () => ({
    events: state.events,
    isAvailable: state.events !== null,
    isSuccess: state.eventsStatus === 'success',
    isPending: state.eventsStatus === 'pending'
  })
}))
vi.mock('~/hooks/useFeatureFlag', () => ({
  useFeatureFlag: () => ({ enabled: state.autoCuration, isLoading: state.flagLoading })
}))
vi.mock('~/hooks/useItemSync', () => ({ useItemSyncs: () => state.syncs }))
// Checks exactly the items it is handed, like the real hook, with canned results.
vi.mock('~/hooks/useCollectionValidation', () => ({
  useCollectionValidation: (items: Item[], enabled: boolean) => ({
    results: new Map(
      (enabled ? items : []).map(item => [item.id, { status: 'pass', issues: state.issues.get(item.id) ?? [] }])
    ),
    isValidating: enabled && state.isValidating
  })
}))
vi.mock('~/hooks/useProfile', () => ({ useProfile: () => ({ data: undefined }) }))
vi.mock('~/components/CollectionActivityModal', () => ({
  CollectionActivityModal: () => <div data-testid="activity-modal-stub" />
}))
vi.mock('./ApprovalFlowModal', () => ({
  ApprovalFlowModal: ({ mode }: { mode: string }) => <div data-testid="approval-flow" data-mode={mode} />
}))
vi.mock('~/components/AssignCuratorModal', () => ({
  AssignCuratorModal: ({ mode, onAssigned }: { mode: string; onAssigned?: () => void }) => (
    <div data-testid="assign-modal" data-mode={mode}>
      {onAssigned && <button type="button" data-testid="assign-continue" onClick={onAssigned} />}
    </div>
  )
}))

const session = { address: '0xme' } as Session
const base = {
  id: 'c1',
  name: 'Hats',
  isPublished: true,
  isApproved: false,
  reviewedAt: 1,
  createdAt: 1
} as Collection

const item = (id: string) =>
  ({
    id,
    name: `Item ${id}`,
    type: 'wearable',
    contents: {},
    thumbnail: '',
    data: { category: 'hat' }
  }) as unknown as Item
const items = [item('i1'), item('i2'), item('i3')]
const error = (message: string): ValidationIssue => ({ code: 'c', severity: ValidationSeverity.ERROR, message })
const warning = (message: string): ValidationIssue => ({ code: 'c', severity: ValidationSeverity.WARNING, message })

function renderBar(collection: Collection = base, barItems: Item[] = items) {
  render(
    <TranslationProvider>
      <MemoryRouter>
        <ReviewBar session={session} collection={collection} items={barItems} onSelectItem={state.selectItem} />
      </MemoryRouter>
    </TranslationProvider>
  )
}

const actions = () => screen.queryAllByTestId(/^review-action-/).map(button => button.dataset.testid)

beforeEach(() => {
  state.curation = null
  state.events = null
  state.eventsStatus = 'success'
  state.autoCuration = true
  state.flagLoading = false
  state.curationError = false
  state.curationLoading = false
  state.syncs = new Map()
  state.reject.mockReset()
  state.disable.mockReset()
  state.issues = new Map()
  state.isValidating = false
  state.selectItem.mockReset()
  state.track.mockReset()
})

describe('ReviewBar', () => {
  it('offers approve and reject on a new collection, assigning it to the curator before the approval flow', () => {
    renderBar()
    expect(actions()).toEqual(['review-action-approve', 'review-action-reject'])
    fireEvent.click(screen.getByTestId('review-action-approve'))
    expect(screen.getByTestId('assign-modal')).toHaveAttribute('data-mode', 'self')
    expect(screen.queryByTestId('approval-flow')).not.toBeInTheDocument()

    fireEvent.click(screen.getByTestId('assign-continue'))
    expect(screen.getByTestId('approval-flow')).toHaveAttribute('data-mode', 'approve')
  })

  it('takes over another curator’s collection before deciding on it', () => {
    state.curation = { status: 'pending', assignee: '0xother', createdAt: 1, updatedAt: 1 } as CollectionCuration
    renderBar()
    fireEvent.click(screen.getByTestId('review-action-reject'))
    fireEvent.click(screen.getByTestId('assign-continue'))
    expect(screen.getByTestId('review-reject')).toBeInTheDocument()
  })

  it('goes straight to the decision when the collection is already the curator’s', () => {
    state.curation = { status: 'pending', assignee: '0xme', createdAt: 1, updatedAt: 1 } as CollectionCuration
    renderBar()
    fireEvent.click(screen.getByTestId('review-action-approve'))
    expect(screen.queryByTestId('assign-modal')).not.toBeInTheDocument()
    expect(screen.getByTestId('approval-flow')).toBeInTheDocument()
  })

  it('enables a disabled collection without opening a request up front', () => {
    renderBar({ ...base, reviewedAt: 5 })
    fireEvent.click(screen.getByTestId('review-action-enable'))
    expect(screen.queryByTestId('assign-modal')).not.toBeInTheDocument()
    expect(screen.getByTestId('approval-flow')).toHaveAttribute('data-mode', 'enable')
  })

  it('asks to take over the request of a disabled collection before enabling it', () => {
    state.curation = { status: 'approved', assignee: '0xother', createdAt: 1, updatedAt: 1 } as CollectionCuration
    renderBar({ ...base, reviewedAt: 5 })
    fireEvent.click(screen.getByTestId('review-action-enable'))
    fireEvent.click(screen.getByTestId('assign-continue'))
    expect(screen.getByTestId('approval-flow')).toHaveAttribute('data-mode', 'enable')
  })

  it('rejects with a plain confirmation while auto-curation is off, sending no reasons', () => {
    state.autoCuration = false
    renderBar()
    fireEvent.click(screen.getByTestId('review-action-reject'))
    fireEvent.click(screen.getByTestId('assign-continue'))
    fireEvent.click(screen.getByTestId('review-reject-confirm'))
    expect(state.reject).toHaveBeenCalledWith({ collection: base, curation: null }, expect.anything())
  })

  it('points the curator to the forum post to explain a rejection', () => {
    state.reject.mockImplementation((_vars: unknown, options: { onSuccess: () => void }) => options.onSuccess())
    renderBar({ ...base, forumLink: 'https://forum.decentraland.org/t/hats/77' })
    fireEvent.click(screen.getByTestId('review-action-reject'))
    fireEvent.click(screen.getByTestId('assign-continue'))
    fireEvent.click(screen.getByTestId('review-reject-confirm'))
    fireEvent.click(screen.getByTestId('review-verdict-forum-link'))
    expect(openExternal).toHaveBeenCalledWith('https://forum.decentraland.org/t/hats/77')
  })

  it('just confirms a rejection when the collection has no forum post', () => {
    state.reject.mockImplementation((_vars: unknown, options: { onSuccess: () => void }) => options.onSuccess())
    renderBar()
    fireEvent.click(screen.getByTestId('review-action-reject'))
    fireEvent.click(screen.getByTestId('assign-continue'))
    fireEvent.click(screen.getByTestId('review-reject-confirm'))
    expect(screen.queryByTestId('review-verdict')).not.toBeInTheDocument()
  })

  it('also keeps the plain confirmation when the server has no timeline, whatever the flag says', () => {
    state.events = null
    renderBar()
    fireEvent.click(screen.getByTestId('review-action-reject'))
    fireEvent.click(screen.getByTestId('assign-continue'))
    expect(screen.getByTestId('review-reject-confirm')).toBeInTheDocument()
    expect(screen.queryByTestId('reject-curation-confirm')).toBeNull()
  })

  it('waits for the timeline before letting the curator reject, so a rejection never skips its reasons', () => {
    state.eventsStatus = 'pending'
    renderBar()
    expect(screen.getByTestId('review-action-reject')).toBeDisabled()
  })

  it('waits for the flag too before letting the curator reject', () => {
    state.autoCuration = false
    state.flagLoading = true
    renderBar()
    expect(screen.getByTestId('review-action-reject')).toBeDisabled()
  })

  it('still asks for reasons when the timeline failed to load: only a server without one refuses them', () => {
    state.eventsStatus = 'error'
    renderBar()
    fireEvent.click(screen.getByTestId('review-action-reject'))
    fireEvent.click(screen.getByTestId('assign-continue'))
    expect(screen.getByTestId('reject-curation-confirm')).toBeInTheDocument()
  })

  function pickReason(code: string) {
    // The list stays open between picks.
    if (!screen.queryByTestId('reject-reasons-listbox')) fireEvent.click(screen.getByTestId('reject-reasons'))
    fireEvent.click(screen.getByTestId(`reject-reasons-option-${code}`))
  }

  it('rejects only with at least one reason, and sends the details when given', () => {
    state.events = []
    renderBar()
    fireEvent.click(screen.getByTestId('review-action-reject'))
    fireEvent.click(screen.getByTestId('assign-continue'))
    const confirm = screen.getByTestId('reject-curation-confirm')
    expect(confirm).toBeDisabled()
    pickReason('clipping')
    expect(confirm).toBeEnabled()
    fireEvent.change(screen.getByTestId('rejection-message'), { target: { value: '  Clips through the torso ' } })
    fireEvent.click(confirm)
    expect(state.reject).toHaveBeenCalledWith(
      {
        collection: base,
        curation: null,
        decision: { rejectionReasons: ['clipping'], rejectionMessage: 'Clips through the torso' }
      },
      expect.anything()
    )
  })

  it('lists the picked reasons as pills that drop them, and leaves the details out when empty', () => {
    state.events = []
    renderBar()
    fireEvent.click(screen.getByTestId('review-action-reject'))
    fireEvent.click(screen.getByTestId('assign-continue'))
    pickReason('clipping')
    pickReason('thumbnail')
    expect(screen.getByTestId('reject-reasons')).toHaveTextContent('2 Selected')
    expect(screen.getByTestId('reject-reason-pills')).toHaveTextContent('Clipping issues')
    fireEvent.click(screen.getByTestId('reject-reason-pill-clipping-remove'))
    expect(screen.queryByTestId('reject-reason-pill-clipping')).toBeNull()
    fireEvent.click(screen.getByTestId('reject-curation-confirm'))
    expect(state.reject).toHaveBeenCalledWith(
      { collection: base, curation: null, decision: { rejectionReasons: ['thumbnail'] } },
      expect.anything()
    )
  })

  it('keeps the curator in the app after requesting fixes with reasons, without pointing to the forum', () => {
    state.events = []
    state.reject.mockImplementation((_vars: unknown, options: { onSuccess: () => void }) => options.onSuccess())
    renderBar({ ...base, forumLink: 'https://forum.decentraland.org/t/hats/77' })
    fireEvent.click(screen.getByTestId('review-action-reject'))
    fireEvent.click(screen.getByTestId('assign-continue'))
    pickReason('clipping')
    fireEvent.click(screen.getByTestId('reject-curation-confirm'))
    expect(screen.queryByTestId('review-verdict-forum-link')).not.toBeInTheDocument()
  })

  it('tells the curator when the creator answered and opens the timeline to reply', () => {
    state.events = [
      {
        id: 'c2',
        collectionId: 'c1',
        type: 'review.comment',
        actor: 'creator',
        actorAddress: '0xowner',
        createdAt: 2,
        payload: { message: 'Fixed the clipping' }
      }
    ]
    renderBar()
    expect(screen.getByTestId('review-activity')).toHaveTextContent('Creator replied')
    fireEvent.click(screen.getByTestId('review-activity'))
    expect(screen.getByTestId('activity-modal-stub')).toBeInTheDocument()
  })

  it('shows the derived stage and opens the automated review verdict with its findings', () => {
    state.curation = {
      id: 'r1',
      collectionId: 'c1',
      status: 'pending',
      assignee: null,
      reviewedBy: null,
      rejectionReasons: null,
      rejectionMessage: null,
      createdAt: 1,
      updatedAt: 1
    }
    state.events = [
      {
        id: 'e1',
        collectionId: 'c1',
        type: 'review.automated_passed',
        actor: 'validator',
        actorAddress: null,
        createdAt: Date.now(),
        payload: {
          validationId: 'val-1',
          verdict: 'passed',
          items: [
            {
              itemId: 'i1',
              contentHash: 'h',
              passed: true,
              findings: [{ rule: 'M-04', severity: 'warning', message: 'Large texture', measured: 2048, limit: 1024 }],
              visualSummary: 'Looks fine on both shapes.'
            },
            { itemId: 'i2', contentHash: 'h2', passed: null, findings: [] }
          ]
        }
      }
    ]
    renderBar()
    expect(screen.getByTestId('curation-state')).toHaveAttribute('data-state', 'to_review')
    expect(screen.getByTestId('review-automated-verdict')).toHaveTextContent('Automated review')
    expect(screen.getByTestId('review-automated-verdict')).toHaveAttribute('data-status', 'passed')
    fireEvent.click(screen.getByTestId('review-automated-verdict'))
    expect(screen.getByTestId('automated-verdict-headline')).toHaveAttribute('data-outcome', 'passed')
    expect(screen.getByTestId('automated-verdict-modal')).not.toHaveTextContent('val-1')
    expect(screen.getByTestId('automated-verdict-items-i1-issues')).toHaveTextContent('Large texture')
    expect(screen.getByTestId('automated-verdict-items-i1-issues')).not.toHaveTextContent('M-04')
    expect(screen.getByTestId('automated-verdict-items-i1')).not.toHaveAttribute('open')
    expect(screen.getByTestId('automated-verdict-unchecked')).not.toHaveAttribute('open')
    expect(screen.getByTestId('automated-verdict-items-visual-summary')).toHaveTextContent('Looks fine')
  })

  it('shows the automated review as running, with nothing to open yet', () => {
    state.curation = {
      id: 'r1',
      collectionId: 'c1',
      status: 'pending',
      assignee: null,
      reviewedBy: null,
      rejectionReasons: null,
      rejectionMessage: null,
      createdAt: 1,
      updatedAt: 1
    }
    state.events = [
      {
        id: 'e1',
        collectionId: 'c1',
        type: 'review.automated_started',
        actor: 'validator',
        actorAddress: null,
        createdAt: Date.now(),
        payload: { trigger: 'publish' }
      }
    ]
    renderBar()
    expect(screen.getByTestId('curation-state')).toHaveAttribute('data-state', 'auto_reviewing')
    const chip = screen.getByTestId('review-automated-verdict')
    expect(chip).toHaveAttribute('data-status', 'running')
    fireEvent.click(chip)
    expect(screen.queryByTestId('automated-verdict-modal')).toBeNull()
  })

  it('tells the curator when the creator asked for them after the automated review', () => {
    state.curation = {
      id: 'r1',
      collectionId: 'c1',
      status: 'pending',
      assignee: null,
      reviewedBy: null,
      rejectionReasons: null,
      rejectionMessage: null,
      createdAt: 1,
      updatedAt: 1
    }
    state.events = [
      {
        id: 'e2',
        collectionId: 'c1',
        type: 'review.appeal_requested',
        actor: 'creator',
        actorAddress: '0xcreator',
        createdAt: Date.now(),
        payload: { note: '' }
      },
      {
        id: 'e1',
        collectionId: 'c1',
        type: 'review.automated_rejected',
        actor: 'validator',
        actorAddress: null,
        createdAt: Date.now() - 1000,
        payload: {
          verdict: 'rejected',
          items: [{ itemId: 'i1', contentHash: 'h1', passed: false, findings: [] }]
        }
      }
    ]
    renderBar()
    expect(screen.getByTestId('curation-state')).toHaveAttribute('data-state', 'to_review')
    fireEvent.click(screen.getByTestId('review-automated-verdict'))
    expect(screen.getByTestId('automated-verdict-reason')).toHaveTextContent('The creator requested a curator review')
  })

  it('leads the verdict with the failed items and folds the passed ones', () => {
    // A run always belongs to a review request.
    state.curation = {
      id: 'r1',
      collectionId: 'c1',
      status: 'pending',
      assignee: null,
      reviewedBy: null,
      rejectionReasons: null,
      rejectionMessage: null,
      createdAt: 1,
      updatedAt: 1
    }
    state.events = [
      {
        id: 'e1',
        collectionId: 'c1',
        type: 'review.automated_rejected',
        actor: 'validator',
        actorAddress: null,
        createdAt: Date.now(),
        payload: {
          validationId: 'val-2',
          verdict: 'rejected',
          items: [
            { itemId: 'i1', contentHash: 'h1', passed: true, findings: [] },
            {
              itemId: 'i2',
              contentHash: 'h2',
              passed: false,
              findings: [
                { rule: 'M-01', severity: 'error', message: 'Too many triangles', measured: 12400, limit: 10000 }
              ]
            }
          ]
        }
      }
    ]
    renderBar()
    fireEvent.click(screen.getByTestId('review-automated-verdict'))
    expect(screen.getByTestId('automated-verdict-headline')).toHaveTextContent('Issues in 1 of 2 items')
    const rows = screen.getAllByTestId(/^automated-verdict-items-i\d$/)
    expect(rows.map(row => row.getAttribute('data-testid'))).toEqual([
      'automated-verdict-items-i2',
      'automated-verdict-items-i1'
    ])
    expect(screen.getByTestId('automated-verdict-items-i2')).toHaveAttribute('open')
    expect(screen.getByTestId('automated-verdict-items-i2-status')).toContainHTML('aria-label="1 error"')
    expect(screen.getByTestId('automated-verdict-passed')).not.toHaveAttribute('open')
  })

  it('lists the items without sections when they all share one outcome', () => {
    // A run always belongs to a review request.
    state.curation = {
      id: 'r1',
      collectionId: 'c1',
      status: 'pending',
      assignee: null,
      reviewedBy: null,
      rejectionReasons: null,
      rejectionMessage: null,
      createdAt: 1,
      updatedAt: 1
    }
    state.events = [
      {
        id: 'e1',
        collectionId: 'c1',
        type: 'review.automated_passed',
        actor: 'validator',
        actorAddress: null,
        createdAt: Date.now(),
        payload: {
          validationId: 'val-3',
          verdict: 'passed',
          items: [
            { itemId: 'i1', contentHash: 'h1', passed: true, findings: [] },
            { itemId: 'i2', contentHash: 'h2', passed: true, findings: [] }
          ]
        }
      }
    ]
    renderBar()
    fireEvent.click(screen.getByTestId('review-automated-verdict'))
    expect(screen.queryByTestId('automated-verdict-passed')).toBeNull()
    expect(screen.getByTestId('automated-verdict-items-i1')).toBeInTheDocument()
    expect(screen.getByTestId('automated-verdict-items-i2')).toBeInTheDocument()
  })

  it('opens a lone item, whatever its outcome', () => {
    // A run always belongs to a review request.
    state.curation = {
      id: 'r1',
      collectionId: 'c1',
      status: 'pending',
      assignee: null,
      reviewedBy: null,
      rejectionReasons: null,
      rejectionMessage: null,
      createdAt: 1,
      updatedAt: 1
    }
    state.events = [
      {
        id: 'e1',
        collectionId: 'c1',
        type: 'review.automated_passed',
        actor: 'validator',
        actorAddress: null,
        createdAt: Date.now(),
        payload: {
          validationId: 'val-4',
          verdict: 'passed',
          items: [{ itemId: 'i1', contentHash: 'h1', passed: true, findings: [] }]
        }
      }
    ]
    renderBar()
    fireEvent.click(screen.getByTestId('review-automated-verdict'))
    expect(screen.getByTestId('automated-verdict-items-i1')).toHaveAttribute('open')
  })

  it('disables an approved collection after confirming, and offers the missing entities deploy', () => {
    state.syncs = new Map([['i1', { status: ItemSyncStatus.UNSYNCED }]])
    const approved = { ...base, isApproved: true }
    renderBar(approved)
    expect(actions()).toEqual(['review-action-disable', 'review-action-deploy_missing'])
    fireEvent.click(screen.getByTestId('review-action-disable'))
    fireEvent.click(screen.getByTestId('review-disable-confirm'))
    expect(state.disable).toHaveBeenCalledWith(expect.objectContaining({ collection: approved }), expect.anything())
  })

  it('offers only enable on a disabled collection', () => {
    renderBar({ ...base, reviewedAt: 5 })
    expect(actions()).toEqual(['review-action-enable'])
  })

  it('has no actions on an unpublished collection', () => {
    renderBar({ ...base, isPublished: false })
    expect(actions()).toEqual([])
    expect(screen.getByTestId('review-unpublished')).toBeInTheDocument()
  })

  it('assigns the collection from the bar', () => {
    renderBar()
    fireEvent.click(screen.getByTestId('review-assign-me'))
    expect(screen.getByTestId('assign-modal')).toHaveAttribute('data-mode', 'self')
  })

  it('dates a rejection by when it happened, not by the request it opened', () => {
    state.curation = { status: 'rejected', assignee: null, createdAt: 1, updatedAt: Date.now() } as CollectionCuration
    renderBar()
    expect(screen.getByTestId('review-requested')).toHaveTextContent(/^Fixes requested/)
  })

  it('stops offering the assignment once the collection and its request are approved', () => {
    state.curation = { status: 'approved', assignee: null, createdAt: 1, updatedAt: 2 } as CollectionCuration
    renderBar({ ...base, isApproved: true })
    expect(screen.queryByTestId('review-assign-me')).not.toBeInTheDocument()
  })

  it('holds back every action until the review request loads', () => {
    state.curationError = true
    renderBar()
    expect(actions()).toEqual([])
    expect(screen.queryByTestId('review-assign-me')).toBeNull()
    fireEvent.click(screen.getByTestId('review-curation-retry'))
    expect(state.refetch).toHaveBeenCalled()
  })

  it('offers nothing to act on while the review request loads', () => {
    state.curationLoading = true
    renderBar()
    expect(actions()).toEqual([])
    expect(screen.queryByTestId('review-assign-me')).toBeNull()
    expect(screen.queryByTestId('review-assignee')).toBeNull()
  })

  describe('collection validation', () => {
    const pending = (...ids: string[]) =>
      new Map(ids.map(id => [id, { status: ItemSyncStatus.UNDER_REVIEW }] as [string, { status: string }]))

    it('lists every item with issues in one modal and opens the one picked', () => {
      state.syncs = pending('i1', 'i2', 'i3')
      state.issues = new Map([
        ['i1', [error('Too many triangles')]],
        ['i3', [warning('Big texture'), error('No skeleton')]]
      ])
      renderBar()
      const badge = screen.getByTestId('review-validation')
      expect(badge).toHaveAttribute('data-status', 'errors')
      expect(badge).toHaveTextContent('3 issues')

      fireEvent.click(badge)
      expect(screen.getByTestId('review-validation-i1-card')).toHaveTextContent('Too many triangles')
      expect(screen.getByTestId('review-validation-i3-card')).toHaveTextContent('No skeleton')
      expect(screen.queryByTestId('review-validation-i2-card')).toBeNull()

      fireEvent.click(screen.getByTestId('review-validation-i3-select'))
      expect(state.selectItem).toHaveBeenCalledWith(items[2])
      expect(screen.queryByTestId('review-validation-modal')).toBeNull()
    })

    it('keeps checking until it knows which approved items changed', () => {
      state.curation = { status: 'pending', assignee: null, createdAt: 1, updatedAt: 1 } as CollectionCuration
      state.syncs = new Map([
        ['i1', { status: ItemSyncStatus.LOADING }],
        ['i2', { status: ItemSyncStatus.UNDER_REVIEW }]
      ])
      renderBar({ ...base, isApproved: true })
      expect(screen.getByTestId('review-validation')).toHaveAttribute('data-status', 'loading')
    })

    it('checks the items the Catalyst could not compare instead of spinning forever', () => {
      state.curation = { status: 'pending', assignee: null, createdAt: 1, updatedAt: 1 } as CollectionCuration
      state.syncs = new Map([
        ['i1', { status: ItemSyncStatus.LOADING, lookupFailed: true }],
        ['i2', { status: ItemSyncStatus.UNDER_REVIEW }]
      ])
      state.issues = new Map([['i1', [error('Too many triangles')]]])
      renderBar({ ...base, isApproved: true })
      expect(screen.getByTestId('review-validation')).toHaveAttribute('data-status', 'errors')
    })

    it('reports the result once per collection, even when every check was cached', () => {
      state.syncs = pending('i1', 'i2')
      state.issues = new Map([['i1', [error('Too many triangles'), warning('Big texture')]]])
      renderBar()
      const results = state.track.mock.calls.filter(([event]) => event === 'Review Validation Result')
      expect(results).toEqual([
        [
          'Review Validation Result',
          expect.objectContaining({ collectionId: 'c1', itemCount: 2, errors: 1, warnings: 1, itemsWithIssues: 1 })
        ]
      ])
    })

    it('shows a spinner while any item is being checked', () => {
      state.syncs = pending('i1')
      state.isValidating = true
      renderBar()
      expect(screen.getByTestId('review-validation')).toHaveAttribute('data-status', 'loading')
    })

    it('turns green with no modal when every item passes', () => {
      state.syncs = pending('i1', 'i2')
      renderBar()
      const badge = screen.getByTestId('review-validation')
      expect(badge).toHaveAttribute('data-status', 'pass')
      fireEvent.click(badge)
      expect(screen.queryByTestId('review-validation-modal')).toBeNull()
    })

    it('opens warnings-only results too', () => {
      state.syncs = pending('i1')
      state.issues = new Map([['i1', [warning('Big texture')]]])
      renderBar()
      expect(screen.getByTestId('review-validation')).toHaveAttribute('data-status', 'warnings')
      fireEvent.click(screen.getByTestId('review-validation'))
      expect(screen.getByTestId('review-validation-i1-card')).toBeInTheDocument()
    })

    it('checks only the items with changes waiting for approval', () => {
      state.curation = { status: 'pending', assignee: null, createdAt: 1, updatedAt: 1 } as CollectionCuration
      state.syncs = new Map([
        ['i1', { status: ItemSyncStatus.SYNCED }],
        ['i2', { status: ItemSyncStatus.UNDER_REVIEW }]
      ])
      state.issues = new Map([['i1', [error('Old problem')]]])
      renderBar({ ...base, isApproved: true })
      expect(screen.getByTestId('review-validation')).toHaveAttribute('data-status', 'pass')
    })

    it.each([
      ['approved', { ...base, isApproved: true }],
      ['disabled', { ...base, reviewedAt: 5 }],
      ['unpublished', { ...base, isPublished: false }]
    ])('checks nothing on an %s collection', (_, collection) => {
      state.syncs = pending('i1')
      state.issues = new Map([['i1', [error('Too many triangles')]]])
      renderBar(collection)
      expect(screen.queryByTestId('review-validation')).toBeNull()
    })

    it('checks nothing on a rejected collection', () => {
      state.curation = { status: 'rejected', assignee: null, createdAt: 1, updatedAt: 1 } as CollectionCuration
      state.syncs = pending('i1')
      renderBar()
      expect(screen.queryByTestId('review-validation')).toBeNull()
    })
  })
})
