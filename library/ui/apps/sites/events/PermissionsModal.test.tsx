import { cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { PermissionsModal, type PermissionsModalProps } from './PermissionsModal'

afterEach(cleanup)

const WALLET = '0x1111111111111111111111111111111111111111'

function mount(over: Partial<PermissionsModalProps> = {}) {
  const onClose = vi.fn()
  const onSubmit = vi.fn()
  render(
    <PermissionsModal
      permissions={[]}
      busy={false}
      onClose={onClose}
      onSubmit={onSubmit}
      {...over}
    />
  )
  return { onClose, onSubmit }
}

describe('when adding a user', () => {
  it('should be a dialog named Add User with its close button, Cancel and Save Changes', () => {
    mount()
    const dialog = screen.getByRole('dialog', { name: 'Add User' })
    expect(dialog.tagName).toBe('DIALOG')
    expect(within(dialog).getByRole('button', { name: 'Close' })).toBeTruthy()
    expect(within(dialog).getByRole('button', { name: 'Cancel' })).toBeTruthy()
    expect(
      within(dialog).getByRole<HTMLButtonElement>('button', { name: 'Save Changes' }).disabled
    ).toBe(true)
  })

  it('should close from the close button, Cancel and Escape', async () => {
    const { onClose } = mount()
    await userEvent.click(screen.getByRole('button', { name: 'Close' }))
    await userEvent.click(screen.getByRole('button', { name: 'Cancel' }))
    await userEvent.click(screen.getByRole('textbox', { name: 'Wallet Address' }))
    await userEvent.keyboard('{Escape}')
    expect(onClose).toHaveBeenCalledTimes(3)
  })

  it('should send the typed wallet with the chosen grants', async () => {
    const { onSubmit } = mount()
    await userEvent.type(screen.getByRole('textbox', { name: 'Wallet Address' }), WALLET)
    await userEvent.click(screen.getByRole('switch', { name: 'Approve Events' }))
    await userEvent.click(screen.getByRole('button', { name: 'Save Changes' }))
    expect(onSubmit).toHaveBeenCalledWith({
      address: WALLET,
      permissions: ['approve_any_event'],
      initial: []
    })
  })
})

describe('when the change is being saved', () => {
  it('should stay open: the close button, Cancel and Escape do nothing', async () => {
    const { onClose } = mount({
      user: { address: WALLET, name: 'Selene' },
      permissions: ['approve_any_event'],
      busy: true
    })
    const dialog = screen.getByRole('dialog', { name: 'Edit User' })
    const closeButton = within(dialog).getByRole<HTMLButtonElement>('button', { name: 'Close' })
    expect(closeButton.disabled).toBe(true)
    expect(within(dialog).getByRole<HTMLButtonElement>('button', { name: 'Cancel' }).disabled).toBe(
      true
    )
    await userEvent.click(closeButton)
    fireEvent.keyDown(dialog, { key: 'Escape' })
    expect(onClose).not.toHaveBeenCalled()
  })
})
