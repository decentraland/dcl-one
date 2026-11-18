import { describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { TranslationProvider } from '~/intl'
import { type Collection } from '~/lib/collections'
import { openExternal } from '~/lib/navigation'
import { type CollectionCuration } from '~/lib/curation'
import { CurationRow } from './CurationRow'

vi.mock('~/lib/navigation', () => ({ openExternal: vi.fn() }))
vi.mock('~/components/CollectionMosaic', () => ({ CollectionMosaic: () => null }))
vi.mock('~/hooks/useProfile', () => ({ useProfile: () => ({ data: undefined }) }))
const latest = vi.hoisted(() => ({
  event: null as { type: string; actor?: string } | null,
  timeline: true,
  fetched: [] as boolean[]
}))
vi.mock('~/hooks/useCollectionEvents', () => ({
  useRecentCollectionEvents: (_address: string, _id: string, enabled: boolean) => {
    latest.fetched.push(enabled)
    return { data: latest.timeline ? (latest.event ? [latest.event] : []) : null, flagEnabled: latest.timeline }
  }
}))

const ME = '0xme00000000000000000000000000000000000001'
const collection = {
  id: 'c1',
  name: 'Hats',
  owner: '0xowner0000000000000000000000000000000001',
  isPublished: true,
  isApproved: false,
  itemCount: 3,
  reviewedAt: 1,
  createdAt: 1,
  updatedAt: 1
} as Collection
const pending: CollectionCuration = {
  id: 'r1',
  collectionId: 'c1',
  status: 'pending',
  assignee: null,
  reviewedBy: null,
  rejectionReasons: null,
  rejectionMessage: null,
  createdAt: 1,
  updatedAt: 2
}

function renderRow(curation: CollectionCuration | null, subject: Collection = collection) {
  const onAssign = vi.fn()
  render(
    <TranslationProvider>
      <MemoryRouter>
        <CurationRow collection={subject} curation={curation} address={ME} onAssign={onAssign} />
      </MemoryRouter>
    </TranslationProvider>
  )
  return onAssign
}

describe('CurationRow', () => {
  it('opens the collection in the editor in review mode', () => {
    renderRow(null)
    expect(screen.getByTestId('curation-row-link')).toHaveAttribute(
      'href',
      '/collections/editor?collection=c1&reviewing=true'
    )
  })

  it('tells a first review from an update to an approved collection', () => {
    renderRow(pending)
    expect(screen.getByTestId('review-type')).toHaveAttribute('data-type', 'new')
    cleanup()
    renderRow(pending, { ...collection, isApproved: true })
    expect(screen.getByTestId('review-type')).toHaveAttribute('data-type', 'updates')
  })

  it('links the collection forum post', () => {
    renderRow(pending, { ...collection, forumLink: 'https://forum.decentraland.org/t/hats/77' })
    fireEvent.click(screen.getByTestId('curation-row-forum-link'))
    expect(openExternal).toHaveBeenCalledWith('https://forum.decentraland.org/t/hats/77')
  })

  it('says a collection without a forum post was not posted', () => {
    renderRow(pending)
    expect(screen.queryByTestId('curation-row-forum-link')).not.toBeInTheDocument()
    expect(screen.getByTestId('curation-row-discussion')).toHaveTextContent('Not posted')
  })

  it('offers "Assign to me" on an unassigned collection', () => {
    const onAssign = renderRow(null)
    expect(screen.getByTestId('curation-state')).toHaveAttribute('data-state', 'to_review')
    fireEvent.click(screen.getByTestId('curation-row-assign-me'))
    expect(onAssign).toHaveBeenCalledWith(collection, null, 'self')
  })

  it('shows the assigned curator, marked when it is the signed-in one, with an edit button', () => {
    const assigned = { ...pending, assignee: ME }
    const onAssign = renderRow(assigned)
    expect(screen.getByTestId('curation-state')).toHaveAttribute('data-state', 'to_review')
    expect(screen.getByTestId('curation-row-curator')).toHaveTextContent('(you)')
    fireEvent.click(screen.getByTestId('curation-row-edit-assignee'))
    expect(onAssign).toHaveBeenCalledWith(collection, assigned, 'edit')
  })

  it('keeps a request out of the curator queue while its automated review runs', () => {
    latest.event = { type: 'review.automated_started' }
    renderRow(pending)
    expect(screen.getByTestId('curation-state')).toHaveAttribute('data-state', 'auto_reviewing')
    latest.event = null
  })

  it('marks a request the validator rejected, without asking the server for its timeline', () => {
    latest.fetched = []
    renderRow({ ...pending, status: 'rejected', reviewedBy: 'validator' })
    expect(screen.getByTestId('curation-state')).toHaveAttribute('data-state', 'rejected')
    expect(screen.getByTestId('curation-row-automated-trigger').querySelector('[data-status]')).toHaveAttribute(
      'data-status',
      'rejected'
    )
    expect(latest.fetched.every(fetched => !fetched)).toBe(true)
  })

  it('shows an approved collection as approved, with no automated mark', () => {
    renderRow({ ...pending, status: 'approved', reviewedBy: '0xcurator' }, { ...collection, isApproved: true })
    expect(screen.getByTestId('curation-state')).toHaveAttribute('data-state', 'approved')
    expect(screen.queryByTestId('curation-row-automated-trigger')).toBeNull()
  })

  it('keeps the legacy state without a timeline, even when the server names the reviewer', () => {
    latest.timeline = false
    renderRow({ ...pending, status: 'rejected', reviewedBy: '0xcurator' })
    expect(screen.getByTestId('curation-state')).toHaveAttribute('data-state', 'rejected')
    expect(screen.queryByTestId('curation-row-automated-trigger')).toBeNull()
    latest.timeline = true
  })

  it('locks the curator once the collection and its request are approved', () => {
    renderRow({ ...pending, status: 'approved', assignee: ME }, { ...collection, isApproved: true })
    expect(screen.queryByTestId('curation-row-edit-assignee')).toBeNull()
  })

  it('stops offering the assignment once the collection and its request are approved', () => {
    renderRow({ ...pending, status: 'approved' }, { ...collection, isApproved: true })
    expect(screen.getByTestId('curation-row-assignee')).toHaveTextContent('Unassigned')
    expect(screen.queryByTestId('curation-row-assign-me')).toBeNull()
  })

  it('marks a collection whose creator answered in writing, also after fixes were requested', () => {
    latest.event = { type: 'review.comment', actor: 'creator' }
    renderRow(pending)
    expect(screen.getByTestId('curation-row-replied')).toHaveTextContent('Creator replied')
    cleanup()
    renderRow({ ...pending, status: 'rejected', reviewedBy: '0xcurator' })
    expect(screen.getByTestId('curation-row-replied')).toBeInTheDocument()
    cleanup()
    latest.event = { type: 'review.comment', actor: 'curator' }
    renderRow(pending)
    expect(screen.queryByTestId('curation-row-replied')).toBeNull()
    latest.event = null
  })
})
