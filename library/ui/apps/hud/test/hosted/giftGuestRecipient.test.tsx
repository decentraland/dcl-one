// A guest recipient is offered Gift, as in unity-explorer: only the sender's session decides.
import { describe, it, expect, vi, afterEach } from 'vitest'
import { render, screen } from '@testing-library/react'
import { ProfileCard } from '../../features/profileCard/ProfileCard'
import { SessionProvider } from '../../features/session/SessionContext'
import { receiveProfile, resetProfileStore } from '../../features/session/profileStore'
import { resetPopups } from '../../design'
import { fakeSession } from '../harness'

vi.mock('@hud-host', async (importOriginal) => {
  const real = await importOriginal<object>()
  return {
    ...real,
    gifting: { available: () => Promise.resolve(true), pending: () => Promise.resolve([]), onSent: () => () => {} }
  }
})

afterEach(() => {
  resetPopups()
  resetProfileStore()
})

const GUEST = '0x90e5700000000000000000000000000000000001'

describe('Gift for a guest recipient', () => {
  it("is in a guest's profile card", async () => {
    receiveProfile(GUEST, { address: GUEST, name: 'Guest', hasClaimedName: false, isGuest: true })
    render(
      <SessionProvider value={fakeSession()}>
        <ProfileCard userId={GUEST} x={0} y={0} onClose={vi.fn()} />
      </SessionProvider>
    )
    expect(await screen.findByRole('button', { name: 'Gift' })).toBeTruthy()
  })
})
