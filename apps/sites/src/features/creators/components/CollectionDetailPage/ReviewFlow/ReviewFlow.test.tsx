import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { TranslationProvider } from '@/data/lib/creators/intl'
import { BuilderServerError, ValidationLimitError } from '@/data/lib/creators/builder'
import { type Collection } from '@/data/lib/creators/collections'
import { type CollectionCuration } from '@/data/lib/creators/curation'
import { type CollectionEvent } from '@/data/lib/creators/events'
import { type Item } from '@/data/lib/creators/items'
import { ReviewFlow, type ReviewFlowRequest } from './ReviewFlow'

const mocks = vi.hoisted(() => ({
  validate: vi.fn(),
  appeal: vi.fn(),
  appealReset: vi.fn(),
  toast: vi.fn(),
  track: vi.fn(),
  appealError: null as Error | null
}))
vi.mock('@/features/creators/hooks/useCollectionEvents', () => ({
  useRequestValidation: () => ({ mutate: mocks.validate, isPending: false }),
  useAppealCuration: () => ({
    mutate: mocks.appeal,
    reset: mocks.appealReset,
    isPending: false,
    isError: mocks.appealError !== null,
    error: mocks.appealError
  })
}))
vi.mock('@/features/creators/components/CollectionActivityModal', () => ({
  CollectionActivityModal: ({ commentAs }: { commentAs?: string }) => (
    <div data-testid="activity-modal-stub" data-comment-as={commentAs} />
  )
}))
vi.mock('@/data/lib/creators/notifications', () => ({
  useNotifications: (selector: (state: { showToast: typeof mocks.toast }) => unknown) =>
    selector({ showToast: mocks.toast })
}))
vi.mock('@/data/lib/creators/analytics', () => ({ track: mocks.track, errorCode: () => 'error' }))

const collection = { id: 'c1', name: 'Hats', isPublished: true, isApproved: false } as Collection
const items = [
  {
    id: 'i1',
    name: 'Pirate Hat',
    type: 'wearable',
    data: { category: 'hat' },
    thumbnail: 'thumbnail.png',
    contents: {}
  },
  { id: 'i2', name: 'Clean Cap', type: 'wearable', data: { category: 'hat' }, thumbnail: 'thumbnail.png', contents: {} }
] as unknown as Item[]
const pending: CollectionCuration = {
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
const rejectedByValidator = { ...pending, status: 'rejected' as const, reviewedBy: 'validator' }
const rejectedByCurator = { ...pending, status: 'rejected' as const, reviewedBy: '0xcurator' }

let counter = 0
function event(type: string, payload: CollectionEvent['payload'] = {}, ageMs = 0): CollectionEvent {
  return {
    id: `e${++counter}`,
    collectionId: 'c1',
    type,
    actor: 'validator',
    actorAddress: null,
    payload,
    createdAt: Date.now() - ageMs
  }
}

const verdict = () =>
  event('review.automated_rejected', {
    verdict: 'rejected',
    items: [
      {
        itemId: 'i1',
        contentHash: 'h',
        passed: false,
        findings: [{ rule: 'M-01', check: 'Triangle count', severity: 'error', message: 'Too many triangles' }]
      },
      { itemId: 'i2', contentHash: 'h', passed: true, findings: [] }
    ]
  })

type Options = {
  canManage?: boolean
  request?: ReviewFlowRequest | null
  onRequestHandled?: () => void
  items?: Item[]
}

/** The failed item was saved again after the verdict: its findings are gone and there is something new to judge. */
const editedItems = () => items.map(item => (item.id === 'i1' ? { ...item, currentContentHash: 'edited' } : item))

function renderFlow(curation: CollectionCuration | null, events: CollectionEvent[] | null, options: Options = {}) {
  return render(
    <TranslationProvider>
      <ReviewFlow
        collection={collection}
        address="0xme"
        curation={curation}
        events={events}
        items={options.items ?? items}
        canManage={options.canManage ?? true}
        request={options.request ?? null}
        onRequestHandled={options.onRequestHandled ?? (() => undefined)}
      />
    </TranslationProvider>
  )
}

const handled = () => undefined

/** The page's "Review again", on a rejection whose failed item was saved since. */
function reviewAgain(events: CollectionEvent[], onRequestHandled = handled) {
  return renderFlow(rejectedByValidator, events, { items: editedItems(), request: 'validate', onRequestHandled })
}

describe('ReviewFlow', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    localStorage.clear()
    mocks.appealError = null
  })

  it('tells the creator the automated review is running, with nothing to do', () => {
    renderFlow(pending, [event('review.automated_started', { trigger: 'publish' })])
    expect(screen.getByTestId('review-notice')).toHaveAttribute('data-stage', 'automated_reviewing')
    expect(screen.queryByTestId('request-curator-review')).toBeNull()
  })

  it('keeps the review running through a failed send the server re-sends', () => {
    renderFlow(pending, [
      event('review.automated_error', { reason: 'publish_failed' }),
      event('review.automated_started')
    ])
    expect(screen.getByTestId('review-notice')).toHaveAttribute('data-stage', 'automated_reviewing')
  })

  it("opens the validator's findings once per verdict and lets the creator reopen them", () => {
    const sameVerdict = verdict()
    const { unmount } = renderFlow(rejectedByValidator, [sameVerdict])
    expect(screen.getByTestId('automated-issues-title')).toHaveTextContent('Some items need your attention')
    expect(screen.getByTestId('automated-issues-i1-issues')).toHaveTextContent('Too many triangles')
    expect(screen.queryByTestId('automated-issues-i2-card')).toBeNull()
    expect(mocks.track).toHaveBeenCalledWith('Review Modal Opened', {
      collectionId: 'c1',
      stage: 'rejected_by_validator',
      modal: 'issues'
    })

    fireEvent.click(screen.getByTestId('automated-issues-back'))
    expect(screen.queryByTestId('automated-issues-modal')).toBeNull()
    unmount()

    renderFlow(rejectedByValidator, [sameVerdict])
    expect(screen.queryByTestId('automated-issues-modal')).toBeNull()
    fireEvent.click(screen.getByTestId('review-see-issues'))
    expect(screen.getByTestId('automated-issues-modal')).toBeInTheDocument()
  })

  it('leaves only the way into the findings on the notice of an automated rejection', () => {
    renderFlow(rejectedByValidator, [verdict()])
    fireEvent.click(screen.getByTestId('automated-issues-back'))
    expect(screen.getByTestId('review-see-issues')).toBeInTheDocument()
    expect(screen.queryByTestId('request-curator-review')).toBeNull()
  })

  it('starts another automated review when the page asks for one', () => {
    const onRequestHandled = vi.fn()
    reviewAgain([verdict(), event('review.automated_started', { trigger: 'publish' }, 1000)], onRequestHandled)
    expect(mocks.validate).toHaveBeenCalledWith(expect.objectContaining({ collection }), expect.anything())
    expect(onRequestHandled).toHaveBeenCalled()
  })

  it('opens the daily limit modal at the limit, and sends the creator to a curator from it', () => {
    const events = [
      verdict(),
      ...[1000, 2000, 3000, 4000].map(at => event('review.automated_started', { trigger: 'retry' }, at)),
      event('review.automated_started', { trigger: 'publish' }, 5000)
    ]
    reviewAgain(events)
    expect(mocks.validate).not.toHaveBeenCalled()
    expect(screen.getByTestId('daily-limit-attempts')).toHaveTextContent('5 of 5 used today')

    mocks.appeal.mockImplementationOnce((_, { onSuccess }) => onSuccess())
    fireEvent.click(screen.getByTestId('daily-limit-request'))
    expect(screen.queryByTestId('request-review-modal')).toBeNull()
    expect(mocks.appeal).toHaveBeenCalledWith({ collection }, expect.anything())
    expect(screen.getByTestId('sent-for-review-modal-title')).toBeInTheDocument()
  })

  it('says a review is running instead of opening the limit modal while one is in progress', () => {
    reviewAgain([event('review.automated_started', { trigger: 'retry' }), verdict()])
    expect(mocks.validate).not.toHaveBeenCalled()
    expect(screen.queryByTestId('daily-limit-modal')).toBeNull()
    expect(mocks.toast).toHaveBeenCalledWith(expect.any(String), { type: 'warn' })
  })

  it('only asks the creator to wait when submitted changes hit the limit outside a rejection', () => {
    renderFlow(pending, [event('review.automated_passed', { verdict: 'passed', items: [] })], { request: 'limit' })
    expect(screen.getByTestId('daily-limit-modal')).toBeInTheDocument()
    expect(screen.queryByTestId('daily-limit-request')).toBeNull()
  })

  it('explains a 429 and each 409 answered by the server', () => {
    mocks.validate.mockImplementationOnce((_, { onError }) =>
      onError(new ValidationLimitError(new BuilderServerError('Limit', 429, { retryAt: '2026-09-30T00:00:00Z' })))
    )
    reviewAgain([verdict()]).unmount()
    expect(mocks.validate).toHaveBeenCalledTimes(1)

    mocks.validate.mockImplementationOnce((_, { onError }) =>
      onError(new BuilderServerError('A validation is already in progress for this collection', 409))
    )
    reviewAgain([verdict()]).unmount()
    expect(mocks.toast).toHaveBeenCalledWith('A review is already running. Try again when it finishes.', {
      type: 'warn'
    })

    mocks.validate.mockImplementationOnce((_, { onError }) =>
      onError(
        new BuilderServerError('Nothing changed since the last validation: edit an item or appeal the decision', 409)
      )
    )
    reviewAgain([verdict()])
    expect(mocks.toast).toHaveBeenLastCalledWith(
      "There's nothing new for the automated review. Edit an item and publish your changes, or request a curator review.",
      { type: 'warn' }
    )
  })

  it('asks the curators again when the page requests it after their fixes, then says the collection was sent', () => {
    renderFlow(rejectedByCurator, [event('review.rejected', { rejectionReasons: ['clipping'] })], {
      request: 'appeal',
      onRequestHandled: handled
    })
    expect(screen.getByTestId('request-review-modal-title')).toHaveTextContent('Request review again')
    mocks.appeal.mockImplementationOnce((_, { onSuccess }) => onSuccess())
    fireEvent.click(screen.getByTestId('request-review-confirm'))
    expect(mocks.appeal).toHaveBeenCalledWith({ collection }, expect.anything())
    expect(screen.getByTestId('sent-for-review-modal-title')).toHaveTextContent('Collection sent for review!')
  })

  it('tells an appeal already open from one the server refuses for another reason', () => {
    mocks.appealError = new BuilderServerError('There is already an open appeal for this collection', 409)
    const { unmount } = renderFlow(rejectedByCurator, [event('review.rejected')], {
      request: 'appeal',
      onRequestHandled: handled
    })
    expect(mocks.toast).toHaveBeenLastCalledWith(expect.stringContaining('already requested'), { type: 'error' })
    unmount()

    mocks.appealError = new BuilderServerError('Only rejected collections can be appealed', 409)
    renderFlow(rejectedByCurator, [event('review.rejected')], { request: 'appeal', onRequestHandled: handled })
    expect(mocks.toast).toHaveBeenLastCalledWith(expect.not.stringContaining('already requested'), { type: 'error' })
  })

  it('warns instead of asking twice while an appeal is open', () => {
    renderFlow(rejectedByValidator, [event('review.appeal_requested'), verdict()])
    fireEvent.click(screen.getByTestId('automated-issues-request-review'))
    expect(screen.queryByTestId('request-review-modal')).toBeNull()
    expect(mocks.toast).toHaveBeenCalledWith(expect.stringContaining('already requested'), { type: 'warn' })
  })

  it('announces a hand-off to a curator once', () => {
    const handoff = event('review.human_required', { reason: 'unsupported_items' })
    const { unmount } = renderFlow(pending, [handoff])
    expect(screen.getByTestId('additional-review-modal-title')).toHaveTextContent('additional review')
    unmount()
    renderFlow(pending, [handoff])
    expect(screen.queryByTestId('additional-review-modal-title')).toBeNull()
    expect(screen.getByTestId('review-notice')).toHaveAttribute('data-stage', 'needs_curator')
  })

  it('shows the reviewing and the limit modals the page asks for, once', async () => {
    const onRequestHandled = vi.fn()
    const events = [event('review.automated_started', { trigger: 'publish' })]
    const { rerender } = renderFlow(pending, events, { request: 'reviewing', onRequestHandled })
    expect(screen.getByTestId('reviewing-attempts')).toHaveTextContent('1 of 5 used today')
    await waitFor(() => expect(onRequestHandled).toHaveBeenCalled())

    rerender(
      <TranslationProvider>
        <ReviewFlow
          collection={collection}
          address="0xme"
          curation={pending}
          events={events}
          items={items}
          canManage
          request="limit"
          onRequestHandled={onRequestHandled}
        />
      </TranslationProvider>
    )
    expect(screen.getByTestId('daily-limit-modal')).toBeInTheDocument()
  })

  it('offers viewers without rights neither actions nor the findings modal', () => {
    renderFlow(rejectedByValidator, [verdict()], { canManage: false })
    expect(screen.queryByTestId('automated-issues-modal')).toBeNull()
    expect(screen.getByTestId('review-notice')).toHaveAttribute('data-stage', 'rejected_by_validator')
  })

  it('shows no notice without a timeline, as without auto curation', () => {
    renderFlow({ ...pending, assignee: '0xcurator' }, null)
    expect(screen.queryByTestId('review-notice')).toBeNull()
  })

  it("points the creator to the activity for a curator's reasons and message, without quoting them", () => {
    renderFlow(
      {
        ...rejectedByCurator,
        rejectionReasons: ['clipping', 'thumbnail'],
        rejectionMessage: 'The straps clip the arm'
      },
      [event('review.rejected')]
    )
    expect(screen.getByTestId('review-notice')).toHaveAttribute('data-stage', 'rejected_by_curator')
    expect(screen.getByTestId('review-notice')).not.toHaveTextContent('The straps clip the arm')
    expect(screen.queryByTestId('request-curator-review')).toBeNull()
    fireEvent.click(screen.getByTestId('review-view-feedback'))
    expect(screen.getByTestId('activity-modal-stub')).toHaveAttribute('data-comment-as', 'creator')
  })

  it('flags a curator comment waiting for an answer and opens the timeline to read it', () => {
    const comment = {
      ...event('review.comment', { message: 'Is the tail meant to be a tiara?' }),
      actor: 'curator' as const,
      actorAddress: '0xcurator'
    }
    renderFlow(pending, [comment, event('review.automated_passed', { verdict: 'passed', items: [] }, 1000)])
    expect(screen.getByTestId('review-comment-count')).toHaveTextContent('1')
    expect(screen.getByTestId('review-notice')).not.toHaveTextContent('Is the tail meant to be a tiara?')
    const reply = screen.getByTestId('review-reply')
    expect(reply).toHaveTextContent('View comment')
    fireEvent.click(reply)
    expect(screen.getByTestId('activity-modal-stub')).toHaveAttribute('data-comment-as', 'creator')
  })

  it('offers no way to write from the notice until a curator comment waits for an answer', () => {
    renderFlow(pending, [event('review.automated_passed', { verdict: 'passed', items: [] })])
    expect(screen.queryByTestId('review-reply')).not.toBeInTheDocument()
  })

  it('sends the note the creator writes along with the request for a curator', () => {
    renderFlow(rejectedByValidator, [verdict()])
    fireEvent.click(screen.getByTestId('automated-issues-request-review'))
    fireEvent.change(screen.getByTestId('request-review-note'), { target: { value: ' The cape is meant to clip ' } })
    fireEvent.click(screen.getByTestId('request-review-confirm'))
    expect(mocks.appeal.mock.calls[0][0]).toMatchObject({ collection, note: 'The cape is meant to clip' })
  })
})
