// The gift's transfer panel (features/gifting/GiftTransferPanel), as the /play shell draws it.
import { describe, it, expect, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import { GiftTransferPanel, type GiftTransferHandlers } from '../../features/gifting/GiftTransferPanel'

const RECIPIENT = { address: '0x' + 'b'.repeat(40), name: 'Bob', nameColor: '#ff0000' }
const handlers = (): GiftTransferHandlers => ({ onClose: vi.fn(), onConfirm: vi.fn(), onRetry: vi.fn(), onActivity: vi.fn(), onSupport: vi.fn() })

describe('the gift transfer panel', () => {
  it('asks the player to confirm the item and the recipient, with their address, before anything is signed', () => {
    const h = handlers()
    render(<GiftTransferPanel view={{ step: 'confirm', recipient: RECIPIENT, item: { name: 'Cool Hat', rarity: 'epic' } }} handlers={h} />)
    expect(screen.getByText('Send Gift to')).toBeTruthy()
    expect(screen.getByText(RECIPIENT.address)).toBeTruthy()
    expect(screen.getByText('Gifting an item cannot be undone.')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'SEND GIFT' }))
    expect(h.onConfirm).toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: 'CANCEL' }))
    expect(h.onClose).toHaveBeenCalled()
  })

  it('prepares the gift for the recipient with the item, and after a while links to the delivery status', () => {
    const h = handlers()
    const view = { step: 'preparing' as const, recipient: RECIPIENT, item: { name: 'Cool Hat', rarity: 'epic' }, status: 'A browser window should open for you to confirm the transaction.', slow: false }
    const { rerender } = render(<GiftTransferPanel view={view} handlers={h} />)
    expect(screen.getByText('Preparing Gift for')).toBeTruthy()
    expect(screen.getByText('Bob')).toBeTruthy()
    expect(screen.getByText('Cool Hat')).toBeTruthy()
    expect(screen.queryByText(/taking longer than expected/)).toBeNull()
    rerender(<GiftTransferPanel view={{ ...view, slow: true }} handlers={h} />)
    fireEvent.click(screen.getByRole('button', { name: 'here' }))
    expect(h.onActivity).toHaveBeenCalled()
  })

  it('says the gift was sent, and OK closes it', () => {
    const h = handlers()
    render(<GiftTransferPanel view={{ step: 'sent', recipient: RECIPIENT }} handlers={h} />)
    expect(screen.getByRole('heading').textContent).toBe('Gift Sent to Bob!')
    fireEvent.click(screen.getByRole('button', { name: 'OK' }))
    expect(h.onClose).toHaveBeenCalled()
  })

  it('offers TRY AGAIN and Contact Support when it went wrong', () => {
    const h = handlers()
    render(<GiftTransferPanel view={{ step: 'failed' }} handlers={h} />)
    expect(screen.getByText('Something went wrong')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'TRY AGAIN' }))
    fireEvent.click(screen.getByRole('button', { name: 'Contact Support' }))
    expect(h.onRetry).toHaveBeenCalled()
    expect(h.onSupport).toHaveBeenCalled()
  })
})
