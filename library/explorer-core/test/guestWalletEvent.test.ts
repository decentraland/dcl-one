import { describe, expect, it, vi, beforeEach } from 'vitest'
import { track } from '../lib/analytics'
import { trackGuestWalletCreation } from '../features/auth/guestWalletEvent'

vi.mock('../lib/analytics', () => ({ track: vi.fn() }))

// PLAY AS GUEST's "Guest Wallet Creation": a new user only for a new wallet, nothing without a wallet call
describe('Guest Wallet Creation', () => {
  beforeEach(() => vi.mocked(track).mockClear())

  it.each([
    ['a new wallet', 'new', true],
    ['the kept wallet session', 'existing', false],
    ['a shell that does not say', undefined, true]
  ] as const)('%s reports a success, a new user -> %s', async (_case, wallet, isNew) => {
    await trackGuestWalletCreation(async () => ({ wallet }))
    expect(track).toHaveBeenCalledWith('Guest Wallet Creation', expect.objectContaining({ outcome: 'success', is_new_user: isNew }))
  })

  it('a kept guest signed in again with no wallet call reports nothing', async () => {
    await trackGuestWalletCreation(async () => ({ wallet: null }))
    expect(track).not.toHaveBeenCalled()
  })
})
