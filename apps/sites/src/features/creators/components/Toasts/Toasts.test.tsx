import { beforeEach, describe, expect, it } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { useNotifications } from '~/lib/notifications'
import { Toasts } from './Toasts'

beforeEach(() => {
  useNotifications.setState({ toasts: [] })
})

function renderToasts() {
  return render(
    <TranslationProvider>
      <Toasts />
    </TranslationProvider>
  )
}

describe('Toasts', () => {
  it('renders each toast with the icon of its type and none for a plain toast', () => {
    const { showToast } = useNotifications.getState()
    showToast('Saved')
    showToast('Failed', { type: 'error' })
    showToast('Heads up', { type: 'warn' })
    showToast('FYI', { type: 'info' })
    showToast('Plain', { type: null })
    renderToasts()

    const cards = screen.getAllByTestId('toast')
    expect(cards.map(card => card.getAttribute('data-type'))).toEqual(['success', 'error', 'warn', 'info', null])
    expect(screen.getAllByTestId('toast-icon')).toHaveLength(4)
    expect(screen.getByText('Failed')).toHaveAttribute('role', 'alert')
  })

  it('dismisses a toast from its close button', async () => {
    useNotifications.getState().showToast('Saved')
    renderToasts()
    await userEvent.click(screen.getByRole('button'))
    expect(screen.queryByTestId('toast')).not.toBeInTheDocument()
  })
})
