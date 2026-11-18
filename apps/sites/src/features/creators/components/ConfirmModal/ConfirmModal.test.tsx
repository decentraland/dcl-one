import { describe, expect, it, vi } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '@/data/lib/creators/intl'
import { useNotifications } from '@/data/lib/creators/notifications'
import { ConfirmModal } from './ConfirmModal'

const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>

function renderModal(props: Partial<React.ComponentProps<typeof ConfirmModal>> = {}) {
  const onClose = vi.fn()
  const onConfirm = vi.fn()
  render(
    <ConfirmModal
      title="Delete this?"
      description="It cannot be undone."
      onClose={onClose}
      cancel={{ label: 'Cancel', onClick: onClose, testId: 'cancel' }}
      confirm={{ label: 'Delete', onClick: onConfirm, testId: 'confirm' }}
      testId="confirm-modal"
      {...props}
    />,
    { wrapper }
  )
  return { onClose, onConfirm }
}

describe('ConfirmModal', () => {
  it('runs the confirm action and closes from cancel', async () => {
    const { onClose, onConfirm } = renderModal()
    expect(screen.getByTestId('confirm-modal-description')).toHaveTextContent('It cannot be undone.')
    await userEvent.click(screen.getByTestId('confirm'))
    expect(onConfirm).toHaveBeenCalled()
    await userEvent.click(screen.getByTestId('cancel'))
    expect(onClose).toHaveBeenCalled()
  })

  it('locks every way out while busy', async () => {
    const { onClose } = renderModal({ busy: true })
    expect(screen.getByTestId('cancel')).toBeDisabled()
    await userEvent.keyboard('{Escape}')
    expect(onClose).not.toHaveBeenCalled()
  })

  it('reports a failed confirm as an error toast, leaving the dialog as it was', () => {
    renderModal({ error: 'Something went wrong' })
    expect(useNotifications.getState().toasts).toContainEqual(
      expect.objectContaining({ message: 'Something went wrong', type: 'error' })
    )
    expect(screen.getByTestId('confirm-modal')).not.toHaveTextContent('Something went wrong')
  })

  it('renders a single action when only one is given', () => {
    renderModal({ cancel: undefined })
    expect(screen.queryByTestId('cancel')).not.toBeInTheDocument()
    expect(screen.getByTestId('confirm')).toBeInTheDocument()
  })
})
