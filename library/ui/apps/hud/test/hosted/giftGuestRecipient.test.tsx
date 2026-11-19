// A guest recipient is offered Gift, as in unity-explorer: only the sender's session decides.
import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest'
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import type { HostGifting } from '@explorer-core/hostContract'
import { ProfileCard } from '../../features/profileCard/ProfileCard'
import { Passport } from '../../features/profile/Passport'
import { SessionProvider } from '../../features/session/SessionContext'
import { receiveProfile, resetProfileStore, setProfileRequester } from '../../features/session/profileStore'
import { resetPopups } from '../../design'
import { fakeSession } from '../harness'

let canGift = true
const host: HostGifting = { available: async () => canGift, pending: async () => [], open: vi.fn(async () => undefined), onSent: () => () => undefined }
vi.mock('@hud-host', async (importOriginal) => ({
  ...(await importOriginal<object>()),
  get gifting() {
    return host
  }
}))

beforeEach(() => {
  canGift = true
  setProfileRequester(vi.fn())
  receiveProfile(GUEST, { address: GUEST, name: 'Guest', hasClaimedName: false, isGuest: true })
})

afterEach(() => {
  resetPopups()
  resetProfileStore()
})

const GUEST = '0x90e5700000000000000000000000000000000001'

describe('Gift for a guest recipient', () => {
  it("is in a guest's profile card", async () => {
    render(
      <SessionProvider value={fakeSession()}>
        <ProfileCard userId={GUEST} x={0} y={0} onClose={vi.fn()} />
      </SessionProvider>
    )
    expect(await screen.findByRole('button', { name: 'Gift' })).toBeTruthy()
  })

  it("is in a guest's passport options", async () => {
    render(
      <SessionProvider value={fakeSession()}>
        <Passport userId={GUEST} onClose={vi.fn()} />
      </SessionProvider>
    )
    // the host's answer lands after the first render: reopen the menu until it offers Gift
    const options = await screen.findByRole('button', { name: /options|more/i })
    await waitFor(() => {
      if (screen.queryByRole('menuitem', { name: 'Gift' }) == null) fireEvent.click(options)
      expect(screen.getByRole('menuitem', { name: 'Gift' })).toBeTruthy()
    })
  })

  it('is not offered when this session cannot gift (a guest sender)', async () => {
    canGift = false
    render(
      <SessionProvider value={fakeSession()}>
        <ProfileCard userId={GUEST} x={0} y={0} onClose={vi.fn()} />
      </SessionProvider>
    )
    await screen.findByRole('dialog')
    // let the host's answer land before checking
    await act(async () => {
      await Promise.resolve()
    })
    expect(screen.queryByRole('button', { name: 'Gift' })).toBeNull()
  })
})
