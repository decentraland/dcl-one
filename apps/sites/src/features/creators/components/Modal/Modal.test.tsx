import { describe, it, expect, vi } from 'vitest'
import { type ReactNode } from 'react'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { TranslationProvider } from '~/intl'
import { track } from '~/lib/analytics'
import { Modal } from './Modal'

vi.mock('~/lib/analytics', () => ({ track: vi.fn() }))

const wrapper = ({ children }: { children: ReactNode }) => <TranslationProvider>{children}</TranslationProvider>

function renderModal(props: Partial<React.ComponentProps<typeof Modal>> = {}) {
  const onClose = vi.fn()
  const view = render(
    <Modal title="Test Modal" onClose={onClose} {...props}>
      <p>Body</p>
    </Modal>,
    { wrapper }
  )
  return { onClose, view }
}

describe('Modal', () => {
  it('reports the dialog opening and closing under its name', () => {
    const { view } = renderModal({ testId: 'publish-collection-modal' })
    expect(track).toHaveBeenCalledWith('Open modal', { name: 'publish-collection-modal' })

    view.unmount()
    expect(track).toHaveBeenCalledWith('Close modal', { name: 'publish-collection-modal' })
  })

  it('renders the title, body and an accessible dialog', () => {
    renderModal()
    expect(screen.getByRole('dialog', { name: 'Test Modal' })).toBeInTheDocument()
    expect(screen.getByText('Body')).toBeInTheDocument()
  })

  it('closes from the ✕ button, Escape and the scrim, but not from inside the dialog', async () => {
    const { onClose } = renderModal()
    await userEvent.click(screen.getByText('Body'))
    expect(onClose).not.toHaveBeenCalled()

    await userEvent.click(screen.getByTestId('modal-close'))
    await userEvent.keyboard('{Escape}')
    await userEvent.click(screen.getByTestId('modal-scrim'))
    expect(onClose).toHaveBeenCalledTimes(3)
  })

  it('ignores every close affordance while closeDisabled', async () => {
    const { onClose } = renderModal({ closeDisabled: true })
    await userEvent.keyboard('{Escape}')
    await userEvent.click(screen.getByTestId('modal-scrim'))
    expect(screen.getByTestId('modal-close')).toBeDisabled()
    expect(onClose).not.toHaveBeenCalled()
  })

  it('hides the ✕ with hideTitle unless showClose asks for it', async () => {
    const { view } = renderModal({ hideTitle: true })
    expect(screen.queryByTestId('modal-close')).not.toBeInTheDocument()
    view.unmount()

    const { onClose } = renderModal({ hideTitle: true, showClose: true })
    await userEvent.click(screen.getByTestId('modal-close'))
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  it('locks page scroll while open and restores it on unmount', () => {
    const { view } = renderModal()
    expect(document.documentElement.style.overflow).toBe('hidden')
    view.unmount()
    expect(document.documentElement.style.overflow).toBe('')
  })

  it('lets only the topmost stacked modal trap Tab and handle Escape', async () => {
    const parent = renderModal()
    const closeChild = vi.fn()
    const child = render(
      <Modal title="Stacked" onClose={closeChild} testId="child">
        <button type="button">One</button>
        <button type="button">Two</button>
      </Modal>,
      { wrapper }
    )

    await userEvent.tab()
    expect(screen.getByTestId('child-close')).toHaveFocus()
    await userEvent.tab()
    expect(screen.getByRole('button', { name: 'One' })).toHaveFocus()
    await userEvent.tab()
    expect(screen.getByRole('button', { name: 'Two' })).toHaveFocus()
    // Wraps inside the child instead of being pulled back by the parent's trap.
    await userEvent.tab()
    expect(screen.getByTestId('child-close')).toHaveFocus()

    await userEvent.keyboard('{Escape}')
    expect(closeChild).toHaveBeenCalledTimes(1)
    expect(parent.onClose).not.toHaveBeenCalled()

    child.unmount()
    await userEvent.keyboard('{Escape}')
    expect(parent.onClose).toHaveBeenCalledTimes(1)
  })

  it('gives keyboard ownership to a modal nested inside another in the same tree', async () => {
    const closeParent = vi.fn()
    const closeChild = vi.fn()
    render(
      <Modal title="Parent" onClose={closeParent}>
        <button type="button">Parent action</button>
        <Modal title="Child" onClose={closeChild} testId="child">
          <button type="button">One</button>
        </Modal>
      </Modal>,
      { wrapper }
    )

    await userEvent.tab()
    expect(screen.getByTestId('child-close')).toHaveFocus()
    await userEvent.tab()
    expect(screen.getByRole('button', { name: 'One' })).toHaveFocus()
    await userEvent.tab()
    expect(screen.getByTestId('child-close')).toHaveFocus()

    await userEvent.keyboard('{Escape}')
    expect(closeChild).toHaveBeenCalledTimes(1)
    expect(closeParent).not.toHaveBeenCalled()
  })

  it('keeps the page scroll unlocked only after every stacked modal is gone', () => {
    const first = renderModal()
    const second = render(
      <Modal title="Stacked" onClose={vi.fn()}>
        <p>Stacked body</p>
      </Modal>,
      { wrapper }
    )
    expect(document.documentElement.style.overflow).toBe('hidden')

    // Closing in either order must not leave the page locked.
    first.view.unmount()
    expect(document.documentElement.style.overflow).toBe('hidden')
    second.unmount()
    expect(document.documentElement.style.overflow).toBe('')
  })
})
