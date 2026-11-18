// @vitest-environment jsdom
import '@/data/lib/creators/test/setup'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { TranslationProvider } from '@/data/lib/creators/intl'
import { type Collection } from '@/data/lib/creators/collections'
import { type CollectionEvent, type ReviewParty } from '@/data/lib/creators/events'
import { CollectionActivityModal } from './CollectionActivityModal'

const state = vi.hoisted(() => ({
  events: [] as CollectionEvent[],
  isLoading: false,
  isError: false,
  hasNextPage: false,
  isFetchNextPageError: false,
  isAvailable: true,
  fetchNextPage: vi.fn(),
  isCurator: false,
  post: vi.fn(),
  postError: false
}))
vi.mock('../../hooks/useCollectionEvents', () => ({
  useCollectionEvents: () => ({
    events: state.events,
    isLoading: state.isLoading,
    isError: state.isError || state.isFetchNextPageError,
    error: null,
    hasNextPage: state.hasNextPage,
    isFetchNextPageError: state.isFetchNextPageError,
    isAvailable: state.isAvailable,
    isFetchingNextPage: false,
    fetchNextPage: state.fetchNextPage,
    refetch: vi.fn()
  }),
  usePostComment: () => ({ mutate: state.post, isPending: false, isError: state.postError })
}))
vi.mock('../../hooks/useCommittee', () => ({ useCommittee: () => ({ isCurator: state.isCurator }) }))
vi.mock('../../hooks/useProfile', () => ({ useProfile: () => ({ data: { name: 'Curator Jane' } }) }))
vi.mock('@/data/lib/creators/analytics', () => ({ track: vi.fn() }))

const collection = { id: 'c1', name: 'Hats', isPublished: true } as Collection

let counter = 0
function event(
  type: string,
  actor: CollectionEvent['actor'],
  payload: CollectionEvent['payload'] = {}
): CollectionEvent {
  return {
    id: `e${++counter}`,
    collectionId: 'c1',
    type,
    actor,
    actorAddress: actor === 'creator' || actor === 'curator' ? '0xabc' : null,
    payload,
    createdAt: Date.now() - counter * 60_000
  }
}

function renderModal(commentAs?: ReviewParty) {
  render(
    <TranslationProvider>
      <CollectionActivityModal collection={collection} address="0xme" commentAs={commentAs} onClose={vi.fn()} />
    </TranslationProvider>
  )
}

beforeEach(() => {
  state.events = []
  state.isLoading = false
  state.isError = false
  state.hasNextPage = false
  state.isFetchNextPageError = false
  state.isAvailable = true
  state.isCurator = false
  state.fetchNextPage.mockReset()
  state.post.mockReset()
  state.postError = false
})

describe('CollectionActivityModal', () => {
  it('renders one readable row per event with its details', () => {
    state.events = [
      event('review.rejected', 'curator', { rejectionReasons: ['clipping'], rejectionMessage: 'Fix the sleeves' }),
      event('review.appeal_requested', 'creator', { note: 'I fixed it' }),
      event('review.automated_rejected', 'validator', {
        verdict: 'rejected',
        items: [
          {
            itemId: 'i1',
            contentHash: 'h',
            passed: false,
            findings: [{ rule: 'M-01', severity: 'error', message: 'Too many', docs: 'javascript:alert(1)' }]
          },
          { itemId: 'i2', contentHash: 'h2', passed: null, findings: [] }
        ]
      }),
      event('review.assigned', 'curator', { assignee: '0xabc' }),
      event('review.automated_started', 'validator', { trigger: 'retry' }),
      event('collection.published', 'creator')
    ]
    renderModal()
    const lines = screen.getAllByTestId('activity-line').map(line => line.textContent)
    expect(lines).toEqual([
      'requested fixes',
      'requested a human review',
      'Automated review found issues in 1 item',
      'assigned the review to Curator Jane',
      'Automated review started (requested by the creator)',
      'published the collection'
    ])
    expect(screen.getByTestId('activity-reasons')).toHaveTextContent('Clipping')
    expect(screen.getAllByTestId('activity-quote').map(quote => quote.textContent)).toEqual([
      'Fix the sleeves',
      'I fixed it'
    ])
    fireEvent.click(screen.getByTestId('activity-toggle-findings'))
    expect(screen.getByTestId('activity-findings-i1-finding')).toHaveTextContent('M-01')
    expect(screen.getByTestId('activity-findings-i1-finding').querySelector('a')).toBeNull()
  })

  it('names an event type it does not know instead of an i18n key', () => {
    state.events = [event('review.something_new', 'system')]
    renderModal()
    expect(screen.getByTestId('activity-line')).toHaveTextContent('review.something_new')
  })

  it('keeps the loaded rows when a later page fails', () => {
    state.events = [event('collection.published', 'creator')]
    state.hasNextPage = true
    state.isFetchNextPageError = true
    renderModal()
    expect(screen.getByTestId('activity-timeline')).toBeInTheDocument()
    expect(screen.getByTestId('activity-load-more-error')).toBeInTheDocument()
    expect(screen.queryByTestId('activity-error')).toBeNull()
  })

  it('softens validator errors for creators', () => {
    state.events = [
      event('review.automated_error', 'validator', { validationId: 'val-9', verdict: 'error', items: [] })
    ]
    renderModal()
    expect(screen.getByTestId('activity-line')).toHaveTextContent('Automated review restarted')
  })

  it("keeps the validator's own failure text to curators; creators read the item's status", () => {
    state.events = [
      event('review.automated_rejected', 'validator', {
        verdict: 'rejected',
        items: [{ itemId: 'i1', contentHash: 'h', passed: null, findings: [], error: 'render server timed out' }]
      })
    ]
    renderModal()
    fireEvent.click(screen.getByTestId('activity-toggle-findings'))
    expect(screen.queryByTestId('activity-findings-item-error')).toBeNull()
    cleanup()

    state.isCurator = true
    renderModal()
    fireEvent.click(screen.getByTestId('activity-toggle-findings'))
    expect(screen.getByTestId('activity-findings-item-error')).toHaveTextContent('render server timed out')
  })

  it('explains items the validator could not check, with their findings', () => {
    state.isCurator = true
    state.events = [
      event('review.human_required', 'system', {
        reason: 'unsupported_items',
        validationId: 'val-3',
        verdict: 'error',
        items: [
          {
            itemId: 'i1',
            contentHash: 'h',
            passed: null,
            findings: [{ rule: 'S-01', severity: 'warning', message: 'PNG facial features need a curator' }]
          }
        ]
      }),
      event('review.human_required', 'system', { reason: 'third_party' })
    ]
    renderModal()
    const lines = screen.getAllByTestId('activity-line').map(line => line.textContent)
    expect(lines[0]).toBe("Some items can't go through the automated review, a curator will review them")
    expect(lines[1]).toBe('Sent to the curation committee (third-party collection)')
    fireEvent.click(screen.getByTestId('activity-toggle-findings'))
    expect(screen.getByTestId('activity-findings-i1-finding')).toHaveTextContent('S-01')
  })

  it('says when the automated review could not run, and labels items it cannot judge', () => {
    state.events = [
      event('review.automated_error', 'system', { reason: 'sweep_exhausted', validationId: 'val-4' }),
      event('review.human_required', 'system', {
        reason: 'unsupported_items',
        verdict: 'error',
        rulesVersion: '0.4.0',
        items: [{ itemId: 'i1', contentHash: 'h', passed: null, unsupported: true, findings: [] }]
      })
    ]
    renderModal()
    const lines = screen.getAllByTestId('activity-line').map(line => line.textContent)
    expect(lines[0]).toBe("The automated review couldn't run on this collection, a curator will review it")
    fireEvent.click(screen.getByTestId('activity-toggle-findings'))
    expect(screen.getByTestId('activity-findings-item')).toHaveTextContent('Not covered by the automated review')
  })

  it('keeps run metadata out of the timeline, even for curators', () => {
    state.isCurator = true
    state.events = [
      event('review.automated_passed', 'validator', {
        verdict: 'passed',
        validationId: 'val-7',
        rulesVersion: '0.8.0',
        items: []
      })
    ]
    renderModal()
    const row = screen.getByTestId('activity-event')
    expect(row).not.toHaveTextContent('val-7')
    expect(row).not.toHaveTextContent('0.8.0')
  })

  it("marks every decision, automated or a curator's, with its outcome icon", () => {
    state.events = [
      event('review.approved', 'curator'),
      event('review.rejected', 'curator', { rejectionReasons: ['clipping'] }),
      event('review.automated_rejected', 'validator', { verdict: 'rejected', items: [] }),
      event('review.automated_passed', 'validator', { verdict: 'passed', items: [] }),
      event('collection.published', 'creator')
    ]
    renderModal()
    const rows = screen.getAllByTestId('activity-event')
    expect(rows.map(row => row.querySelector('[data-status]')?.getAttribute('data-status') ?? null)).toEqual([
      'passed',
      'rejected',
      'rejected',
      'passed',
      null
    ])
  })

  it('names every final automated failure the same way', () => {
    state.events = [
      event('review.automated_error', 'system', { reason: 'too_large' }),
      event('review.human_required', 'system', { reason: 'validator_error' })
    ]
    renderModal()
    const lines = screen.getAllByTestId('activity-line').map(line => line.textContent)
    expect(new Set(lines)).toEqual(
      new Set(["The automated review couldn't run on this collection, a curator will review it"])
    )
  })

  it('tells curators about validator failures', () => {
    state.isCurator = true
    state.events = [event('review.automated_error', 'validator', { verdict: 'error', items: [] })]
    renderModal()
    expect(screen.getByTestId('activity-line')).toHaveTextContent('failed and will be retried')
  })

  it('loads the next page on demand and shows the empty and error states', () => {
    state.events = [event('collection.published', 'creator')]
    state.hasNextPage = true
    const { unmount } = render(
      <TranslationProvider>
        <CollectionActivityModal collection={collection} address="0xme" onClose={vi.fn()} />
      </TranslationProvider>
    )
    fireEvent.click(screen.getByTestId('activity-load-more'))
    expect(state.fetchNextPage).toHaveBeenCalled()
    unmount()

    state.events = []
    state.hasNextPage = false
    renderModal()
    expect(screen.getByTestId('activity-empty')).toBeInTheDocument()
  })

  it('tells a server without the timeline apart from an empty one', () => {
    state.isAvailable = false
    renderModal()
    expect(screen.getByTestId('activity-unavailable')).toBeInTheDocument()
    expect(screen.queryByTestId('activity-empty')).toBeNull()
  })

  it('shows a comment with its author and text', () => {
    state.events = [event('review.comment', 'curator', { message: 'Is the tail meant to be a tiara?' })]
    renderModal()
    expect(screen.getByTestId('activity-line')).toHaveTextContent('commented')
    expect(screen.getByTestId('activity-actor')).toBeInTheDocument()
    expect(screen.getByTestId('activity-quote')).toHaveTextContent('Is the tail meant to be a tiara?')
  })

  it('sends what a creator writes, trimmed, and clears the box once it is stored', () => {
    state.post.mockImplementation((_vars: unknown, options: { onSuccess: () => void }) => options.onSuccess())
    renderModal('creator')
    const input = screen.getByTestId('activity-comment-input')
    fireEvent.change(input, { target: { value: '  Only added a tag  ' } })
    fireEvent.click(screen.getByTestId('activity-comment-send'))
    expect(state.post.mock.calls[0][0]).toMatchObject({ collection, message: 'Only added a tag', role: 'creator' })
    expect(input).toHaveValue('')
  })

  it('lets a committee member write as the creator of their own collection', () => {
    state.isCurator = true
    renderModal('creator')
    expect(screen.getByTestId('activity-composer')).toHaveTextContent('Write to the curators')
    fireEvent.change(screen.getByTestId('activity-comment-input'), { target: { value: 'Fixed the hat' } })
    fireEvent.click(screen.getByTestId('activity-comment-send'))
    expect(state.post.mock.calls[0][0]).toMatchObject({ role: 'creator' })
  })

  it('writes as a curator from the review and never sends an empty message', () => {
    state.isCurator = true
    renderModal('curator')
    expect(screen.getByTestId('activity-comment-send')).toBeDisabled()
    fireEvent.change(screen.getByTestId('activity-comment-input'), { target: { value: 'Please reweight the legs' } })
    fireEvent.click(screen.getByTestId('activity-comment-send'))
    expect(state.post.mock.calls[0][0]).toMatchObject({ role: 'curator' })
  })

  it('shows the length of a message only once it is close to the limit', () => {
    renderModal('creator')
    const input = screen.getByTestId('activity-comment-input')
    fireEvent.change(input, { target: { value: 'Short' } })
    expect(screen.queryByTestId('activity-comment-count')).not.toBeInTheDocument()
    fireEvent.change(input, { target: { value: 'a'.repeat(1600) } })
    expect(screen.getByTestId('activity-comment-count')).toHaveTextContent('1600/2000')
  })

  it('keeps the text and says so when the message could not be sent', () => {
    state.postError = true
    renderModal('creator')
    fireEvent.change(screen.getByTestId('activity-comment-input'), { target: { value: 'Hello' } })
    fireEvent.click(screen.getByTestId('activity-comment-send'))
    expect(screen.getByTestId('activity-comment-error')).toBeInTheDocument()
    expect(screen.getByTestId('activity-comment-input')).toHaveValue('Hello')
  })

  it('offers no box to readers, nor where the collection has no timeline', () => {
    renderModal()
    expect(screen.queryByTestId('activity-composer')).not.toBeInTheDocument()
    cleanup()
    state.isAvailable = false
    renderModal('creator')
    expect(screen.queryByTestId('activity-composer')).not.toBeInTheDocument()
  })
})
