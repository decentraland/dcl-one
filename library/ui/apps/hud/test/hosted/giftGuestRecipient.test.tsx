// A guest recipient is offered Gift, as in unity-explorer: only the sender's session decides.
import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import type { HostGifting } from '@explorer-core/hostContract'
import { ProfileCard } from '../../features/profileCard/ProfileCard'
import { Passport } from '../../features/profile/Passport'
import { SessionProvider } from '../../features/session/SessionContext'
import { receiveProfile, resetProfileStore, setProfileRequester } from '../../features/session/profileStore'
import { resetPopups } from '../../design'
import { fakeSession } from '../harness'

const host: HostGifting = { available: async () => true, pending: async () => [], open: vi.fn(async () => undefined), onSent: () => () => undefined }
vi.mock('@hud-host', async (importOriginal) => ({
  ...(await importOriginal<object>()),
  get gifting() {
    return host
  }
}))

beforeEach(() => {
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
    // the answer from the host lands after the first render
    await screen.findByRole('button', { name: /options|more/i })
    await new Promise((r) => setTimeout(r, 0))
    fireEvent.click(screen.getByRole('button', { name: /options|more/i }))
    expect(await screen.findByRole('menuitem', { name: 'Gift' })).toBeTruthy()
  })
})
