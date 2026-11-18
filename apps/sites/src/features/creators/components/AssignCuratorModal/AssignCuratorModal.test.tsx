import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { TranslationProvider } from '~/intl'
import { type Collection } from '~/lib/collections'
import { type CollectionCuration } from '~/lib/curation'
import { AssignCuratorModal } from './AssignCuratorModal'

const ME = '0xme'
const OTHER = '0xother'
const mutate = vi.fn()
vi.mock('~/lib/builder', () => ({ fetchCommittee: async () => [OTHER, ME] }))
vi.mock('~/hooks/useCuration', async importOriginal => ({
  ...(await importOriginal<typeof import('~/hooks/useCuration')>()),
  useAssignCurator: () => ({ mutate, isPending: false, isError: false })
}))
vi.mock('~/hooks/useProfile', () => ({ useProfiles: (list: string[]) => list.map(() => undefined) }))

const collection = { id: 'c1', name: 'Hats' } as Collection
const curation: CollectionCuration = {
  id: 'r1',
  collectionId: 'c1',
  status: 'pending',
  assignee: OTHER,
  reviewedBy: null,
  rejectionReasons: null,
  rejectionMessage: null,
  createdAt: 1,
  updatedAt: 1
}

function renderModal(mode: 'self' | 'edit', onClose = vi.fn()) {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <TranslationProvider>
        <AssignCuratorModal collection={collection} curation={curation} address={ME} mode={mode} onClose={onClose} />
      </TranslationProvider>
    </QueryClientProvider>
  )
  return onClose
}

beforeEach(() => mutate.mockReset())

describe('AssignCuratorModal', () => {
  it('assigns the signed-in curator', () => {
    renderModal('self')
    fireEvent.click(screen.getByTestId('assign-curator-submit'))
    expect(mutate).toHaveBeenCalledWith({ collection, curation, assignee: ME }, expect.anything())
  })

  it('unassigns the collection', async () => {
    renderModal('edit')
    fireEvent.click(screen.getByTestId('assign-curator-select'))
    fireEvent.click(await screen.findByRole('option', { name: 'Nobody (unassign)' }))
    fireEvent.click(screen.getByTestId('assign-curator-submit'))
    expect(mutate).toHaveBeenCalledWith({ collection, curation, assignee: null }, expect.anything())
  })

  it('closes without a request when the curator did not change', () => {
    const onClose = renderModal('edit')
    fireEvent.click(screen.getByTestId('assign-curator-submit'))
    expect(mutate).not.toHaveBeenCalled()
    expect(onClose).toHaveBeenCalled()
  })

  it('takes over another curator’s collection and continues into the decision', async () => {
    const onAssigned = vi.fn()
    mutate.mockImplementation((_vars: unknown, options?: { onSuccess?: () => void }) => options?.onSuccess?.())
    render(
      <QueryClientProvider client={new QueryClient()}>
        <TranslationProvider>
          <AssignCuratorModal
            collection={collection}
            curation={curation}
            address={ME}
            mode="self"
            onClose={vi.fn()}
            onAssigned={onAssigned}
          />
        </TranslationProvider>
      </QueryClientProvider>
    )
    expect(await screen.findByTestId('assign-curator-body')).toHaveTextContent(`assigned to ${OTHER}`)
    expect(screen.getByTestId('assign-curator-submit')).toHaveTextContent(/continue/i)
    fireEvent.click(screen.getByTestId('assign-curator-submit'))
    expect(onAssigned).toHaveBeenCalled()
  })
})
