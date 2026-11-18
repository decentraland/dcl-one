import { describe, it, expect, vi, beforeEach } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { TranslationProvider } from '~/intl'
import { type ReviewedContent } from '~/lib/approveLinkedCollection'
import { type Session } from '~/lib/auth'
import { type Collection } from '~/lib/collections'
import { type Item } from '~/lib/items'
import { type ThirdParty } from '~/lib/thirdParty'
import { LinkedReviewBar } from './LinkedReviewBar'

const rejectMutate = vi.fn()
const approvalProps = vi.fn()

const hat = (id: string) => ({ id, name: `Hat ${id}`, currentContentHash: `hash-${id}` }) as Item
const pending = ['1', '2', '3'].map(hat)
let served = pending

vi.mock('~/hooks/useCuration', () => ({
  useCollectionCuration: () => ({ data: null, isLoading: false })
}))
vi.mock('~/hooks/useLinkedCollection', () => ({
  useItemCurations: () => ({ data: new Map(), isLoading: false })
}))
vi.mock('~/hooks/useLinkedReview', async importOriginal => ({
  ...(await importOriginal<typeof import('~/hooks/useLinkedReview')>()),
  usePendingLinkedItems: () => ({ data: served, isLoading: false }),
  useRejectLinkedItems: () => ({ mutate: rejectMutate, isPending: false, isError: false, reset: vi.fn() })
}))
vi.mock('./LinkedApprovalFlowModal', () => ({
  LinkedApprovalFlowModal: (props: { review: ReviewedContent }) => {
    approvalProps(props)
    return <div data-testid="approval-flow-stub" />
  }
}))

const collection = { id: 'c1', name: 'Brand Hats', urn: 'urn:x', createdAt: 0 } as unknown as Collection
const thirdParty = { id: 'urn:tp', name: 'Brand', published: true, isApproved: true } as unknown as ThirdParty
const wrapper = ({ children }: { children: ReactNode }) => (
  <TranslationProvider>
    <MemoryRouter>{children}</MemoryRouter>
  </TranslationProvider>
)

function renderBar(selectedId: string | null, onReviewedChange?: (ids: ReadonlySet<string>) => void) {
  return (
    <LinkedReviewBar
      session={{ address: '0xcurator' } as unknown as Session}
      collection={collection}
      thirdParty={thirdParty}
      selectedId={selectedId}
      onSelectItem={vi.fn()}
      onReviewedChange={onReviewedChange}
    />
  )
}

describe('LinkedReviewBar', () => {
  beforeEach(() => {
    rejectMutate.mockReset()
    approvalProps.mockReset()
    served = pending
  })

  it('blocks Approve until enough pending items were opened, then approves what was reviewed', async () => {
    const { rerender } = render(renderBar(null), { wrapper })
    expect(screen.getByTestId('review-linked-progress')).toHaveTextContent('Reviewed 0/3 items')
    await userEvent.click(screen.getByTestId('review-action-approve'))
    expect(screen.queryByTestId('approval-flow-stub')).not.toBeInTheDocument()

    for (const id of ['1', '2', '3']) rerender(renderBar(id))
    expect(screen.getByTestId('review-linked-progress')).toHaveTextContent('Reviewed 3/3 items')

    await userEvent.click(screen.getByTestId('review-action-approve'))
    expect(screen.getByTestId('approval-flow-stub')).toBeInTheDocument()
    const { review } = approvalProps.mock.calls[0][0] as { review: ReviewedContent }
    expect([...review.pendingIds]).toEqual(['1', '2', '3'])
    expect(review.reviewedHashes.get('2')).toBe('hash-2')
  })

  it('rejects the pending items', async () => {
    render(renderBar(null), { wrapper })
    await userEvent.click(screen.getByTestId('review-action-reject'))
    await userEvent.click(screen.getByTestId('review-reject-confirm'))
    expect(rejectMutate).toHaveBeenCalledWith({ items: pending }, expect.anything())
  })

  it('rejects only the items pending when the dialog opened, not ones submitted after', async () => {
    const { rerender } = render(renderBar(null), { wrapper })
    await userEvent.click(screen.getByTestId('review-action-reject'))
    served = [...pending, hat('4')]
    rerender(renderBar(null))
    await userEvent.click(screen.getByTestId('review-reject-confirm'))
    expect(rejectMutate).toHaveBeenCalledWith({ items: pending }, expect.anything())
  })

  it('offers no third-party switch while items wait for review', () => {
    render(renderBar(null), { wrapper })
    expect(screen.queryByTestId('review-action-disable')).not.toBeInTheDocument()
    expect(screen.queryByTestId('review-action-enable')).not.toBeInTheDocument()
  })

  it('with nothing to review, offers Disable, or Enable only to a third party approved before', () => {
    served = []
    const { rerender } = render(renderBar(null), { wrapper })
    expect(screen.getByTestId('review-action-disable')).toBeInTheDocument()

    thirdParty.isApproved = false
    thirdParty.root = '0xroot'
    rerender(renderBar(null))
    expect(screen.getByTestId('review-action-enable')).toBeInTheDocument()

    thirdParty.root = ''
    rerender(renderBar(null))
    expect(screen.queryByTestId('review-action-enable')).not.toBeInTheDocument()
    thirdParty.isApproved = true
  })

  it('tells the sidebar which pending items the curator has opened', () => {
    const onReviewedChange = vi.fn()
    const { rerender } = render(renderBar('1', onReviewedChange), { wrapper })
    rerender(renderBar('3', onReviewedChange))
    expect([...(onReviewedChange.mock.lastCall![0] as ReadonlySet<string>)].sort()).toEqual(['1', '3'])
  })
})
