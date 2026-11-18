import { describe, it, expect, vi } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { NAME_ALREADY_IN_USE_ERROR } from '~/lib/collections'
import { CollectionNameModal } from './CollectionNameModal'

const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>

function renderModal(props: Partial<React.ComponentProps<typeof CollectionNameModal>> = {}) {
  const onSubmit = vi.fn()
  const onClose = vi.fn()
  render(
    <CollectionNameModal
      variant="create"
      isPending={false}
      error={null}
      onSubmit={onSubmit}
      onClose={onClose}
      {...props}
    />,
    { wrapper }
  )
  return { onSubmit, onClose }
}

describe('CollectionNameModal', () => {
  it('disables submit until a non-blank name is typed, and shows the live counter', async () => {
    const { onSubmit } = renderModal()
    const submit = screen.getByTestId('collection-name-submit')
    expect(submit).toBeDisabled()
    expect(screen.getByTestId('collection-name-count')).toHaveTextContent('0/32')

    await userEvent.type(screen.getByTestId('collection-name-input'), 'Pirate Hats')
    expect(screen.getByTestId('collection-name-count')).toHaveTextContent('11/32')
    expect(submit).toBeEnabled()

    await userEvent.click(submit)
    expect(onSubmit).toHaveBeenCalledWith('Pirate Hats')
  })

  it('caps the name at 32 characters', async () => {
    renderModal()
    await userEvent.type(screen.getByTestId('collection-name-input'), 'x'.repeat(40))
    expect(screen.getByTestId('collection-name-count')).toHaveTextContent('32/32')
  })

  it('blocks names with a colon and explains why', async () => {
    const { onSubmit } = renderModal()
    await userEvent.type(screen.getByTestId('collection-name-input'), 'urn:like')
    await userEvent.click(screen.getByTestId('collection-name-submit'))
    expect(onSubmit).not.toHaveBeenCalled()
    expect(screen.getByTestId('collection-name-error')).toHaveTextContent(/":"/)
  })

  it('shows the uniqueness hint until an error replaces it', () => {
    renderModal({ error: NAME_ALREADY_IN_USE_ERROR })
    expect(screen.getByTestId('collection-name-error')).toBeInTheDocument()
    expect(screen.queryByTestId('collection-name-hint')).not.toBeInTheDocument()
  })

  it('maps the server name-taken error to friendly copy', () => {
    renderModal({ error: NAME_ALREADY_IN_USE_ERROR })
    expect(screen.getByTestId('collection-name-error')).toHaveTextContent(/already in use/i)
  })

  it('hides a stale server error once the user edits the name', async () => {
    renderModal({ error: NAME_ALREADY_IN_USE_ERROR })
    expect(screen.getByTestId('collection-name-error')).toBeInTheDocument()
    await userEvent.type(screen.getByTestId('collection-name-input'), 'Another Name')
    expect(screen.queryByTestId('collection-name-error')).not.toBeInTheDocument()
  })

  it('shows generic copy for any other server error, never the raw message', () => {
    renderModal({ error: 'ECONNREFUSED 10.0.0.1' })
    const error = screen.getByTestId('collection-name-error')
    expect(error).not.toHaveTextContent('ECONNREFUSED')
    expect(error).toHaveTextContent(/try again/i)
  })

  it('closes from cancel but not while saving', async () => {
    const { onClose } = renderModal()
    await userEvent.click(screen.getByTestId('collection-name-cancel'))
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('blocks every close affordance while saving', () => {
    const { onClose } = renderModal({ isPending: true })
    expect(screen.getByTestId('collection-name-cancel')).toBeDisabled()
    expect(screen.getByTestId('collection-name-modal-close')).toBeDisabled()
    expect(onClose).not.toHaveBeenCalled()
  })

  it('shows a spinner instead of the submit label while saving', () => {
    renderModal({ isPending: true })
    const submit = screen.getByTestId('collection-name-submit')
    expect(screen.getByTestId('button-spinner')).toBeInTheDocument()
    expect(submit).not.toHaveTextContent(/continue/i)
    expect(submit).toBeDisabled()
  })

  it('seeds the field for renames and submits the new name', async () => {
    const { onSubmit } = renderModal({ variant: 'rename', initialName: 'Old Name' })
    const input = screen.getByTestId('collection-name-input')
    expect(input).toHaveValue('Old Name')
    await userEvent.clear(input)
    await userEvent.type(input, 'New Name')
    await userEvent.click(screen.getByTestId('collection-name-submit'))
    expect(onSubmit).toHaveBeenCalledWith('New Name')
  })
})
