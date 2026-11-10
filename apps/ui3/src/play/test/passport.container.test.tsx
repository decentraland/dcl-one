import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest'
import { render, screen, act, fireEvent } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Passport, openPassport } from '@hud/features/profile/Passport'
import { SessionProvider } from '@hud/features/session/SessionContext'
import { fakeProfileState, fakeSession } from '@hud/test/harness'
import { receiveProfile, seedProfiles, setProfileRequester } from '@hud/features/session/profileStore'
import { PopupHost, closeTopPopup, resetPopups } from '@hud/design'
import type { EngineSession } from '@hud/features/session/useEngineSession'
import type { Profile } from '@hud/engine/protocol'

// COMPONENT: the smart <Passport userId> fetches the rich profile on open, renders identity-only from
// the profile store until it lands, and shows the presentational ProfilePassport; openPassport mounts it.
afterEach(resetPopups)

const request = vi.fn()
beforeEach(() => {
  request.mockClear()
  setProfileRequester(request)
})

function renderWithSession(node: React.ReactNode, mutate?: (s: EngineSession) => void): EngineSession {
  const s = fakeSession()
  mutate?.(s)
  render(<SessionProvider value={s}>{node}</SessionProvider>)
  return s
}

const RICH: Profile = {
  address: '0xabc',
  name: 'Alice',
  picture: 'a.png',
  hasClaimedName: true,
  isGuest: false,
  description: 'gm from the plaza',
  mutuals: 5
}

describe('Passport container', () => {
  it('fetches the rich profile on open and renders it', () => {
    receiveProfile('0xabc', RICH)
    renderWithSession(<Passport userId="0xabc" onClose={vi.fn()} />)
    expect(request).toHaveBeenCalledWith('0xabc', true)
    expect(screen.getByText('gm from the plaza')).toBeTruthy() // rich field from the fetched profile
    expect(screen.getByText('5 Mutual')).toBeTruthy()
  })

  it('renders identity-only from a roster seed while the fetch is in flight', () => {
    seedProfiles([{ address: '0xabc', name: 'Alice', picture: 'a.png' }]) // no engine profile yet
    renderWithSession(<Passport userId="0xabc" onClose={vi.fn()} />)
    expect(request).toHaveBeenCalledWith('0xabc', true)
    expect(screen.getByText('Alice')).toBeTruthy() // resolved from the roster
  })

  it('openPassport mounts the passport via the popup layer', () => {
    receiveProfile('0xabc', RICH)
    renderWithSession(<PopupHost />)
    act(() => {
      openPassport('0xabc')
    })
    expect(screen.getByText('gm from the plaza')).toBeTruthy()
  })
})

describe('Passport block & report', () => {
  it('blocks after the confirmation, through the friends action', async () => {
    receiveProfile('0xabc', RICH)
    const s = renderWithSession(<PopupHost />)
    act(() => {
      openPassport('0xabc')
    })
    await userEvent.click(screen.getByRole('button', { name: 'More options' }))
    await userEvent.click(screen.getByRole('menuitem', { name: 'Block' }))
    await userEvent.click(screen.getByRole('button', { name: 'BLOCK' }))
    expect(s.friends.act).toHaveBeenCalledWith('block', '0xabc')
  })

  it('reads blocked-by from the session: no CHAT for a user who blocked you', () => {
    receiveProfile('0xabc', RICH)
    renderWithSession(<Passport userId="0xabc" onClose={vi.fn()} />, (s) => {
      s.friends.blockedBy = ['0xabc']
    })
    expect(screen.queryByRole('button', { name: 'CHAT' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'ADD FRIEND' })).toBeNull()
  })

  it('shows CHAT and ADD FRIEND for anyone else', () => {
    receiveProfile('0xabc', RICH)
    renderWithSession(<Passport userId="0xabc" onClose={vi.fn()} />)
    expect(screen.getByRole('button', { name: 'CHAT' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'ADD FRIEND' })).toBeTruthy()
  })
})

// The passport is the HUD's one popup that can hold unsaved work, so it is where the popup layer's
// close guard is exercised: a stray backdrop click is refused outright, while the deliberate paths
// (its ×, and the engine-resolved Cancel action) ask the same question.
describe('closing a passport with unsaved edits', () => {
  const openSelfPassport = async (): Promise<void> => {
    receiveProfile('0xme', { ...RICH, address: '0xme', name: 'Me' })
    renderWithSession(<PopupHost />, (s) => {
      s.profile = fakeProfileState({ data: { ...RICH, address: '0xme', name: 'Me' } })
    })
    act(() => {
      openPassport('0xme')
    })
    await userEvent.click(screen.getByRole('button', { name: 'Edit profile' }))
    await userEvent.type(screen.getByLabelText('About me'), '!')
  }

  const backdrop = (): HTMLElement => document.querySelector('[class*="backdrop"]') as HTMLElement

  it('refuses a stray backdrop click outright — no dialog, nothing lost', async () => {
    await openSelfPassport()
    fireEvent.click(backdrop())
    expect(screen.queryByText('Discard changes?')).toBeNull()
    expect(screen.getByLabelText('About me')).toBeTruthy()
  })

  it('has no × to press while editing — that corner is CANCEL', async () => {
    await openSelfPassport()
    expect(screen.queryByRole('button', { name: 'Close' })).toBeNull()
    expect(screen.getByRole('button', { name: 'CANCEL' })).toBeTruthy()
  })

  it('asks on the Cancel action too — Escape must not be the one path that bins an edit', async () => {
    await openSelfPassport()
    act(() => closeTopPopup())
    expect(await screen.findByText('Discard changes?')).toBeTruthy()

    // Escape ON the confirm resolves it as dismissed, which is the non-destructive answer.
    act(() => closeTopPopup())
    expect(screen.queryByText('Discard changes?')).toBeNull()
    expect(screen.getByLabelText('About me')).toBeTruthy()
  })

  it('closes the whole passport on Discard', async () => {
    await openSelfPassport()
    act(() => closeTopPopup())
    await userEvent.click(await screen.findByRole('button', { name: 'Discard' }))
    expect(screen.queryByLabelText('About me')).toBeNull()
  })

  it('closes without a word when nothing has been edited', async () => {
    receiveProfile('0xabc', RICH)
    renderWithSession(<PopupHost />)
    act(() => {
      openPassport('0xabc')
    })
    act(() => closeTopPopup())
    expect(screen.queryByText('gm from the plaza')).toBeNull()
  })
})

// The name colour: own passport, claimed name only; previewed while open, deployed as it closes
// (unity-explorer PassportController), and dropped with the form on Discard.
describe('the name colour picker', () => {
  const ME: Profile = { ...RICH, address: '0xme', name: 'Me' }
  const openOwn = (me: Profile = ME): EngineSession => {
    receiveProfile('0xme', me)
    const s = renderWithSession(<PopupHost />, (s) => {
      s.profile = fakeProfileState({ data: me })
    })
    act(() => {
      openPassport('0xme')
    })
    return s
  }
  const pickPreset = async (hex: string): Promise<void> => {
    // The popup stays open after a pick, so the swatch is pressed only to open it.
    if (screen.queryByRole('dialog', { name: 'Name color' }) == null) await userEvent.click(screen.getByRole('button', { name: 'Name color' }))
    await userEvent.click(screen.getByRole('radio', { name: hex }))
  }

  it('is offered on your own passport with a claimed name only', () => {
    openOwn({ ...ME, name: 'Me#abcd', hasClaimedName: false })
    expect(screen.queryByRole('button', { name: 'Name color' })).toBeNull()
    act(() => closeTopPopup())

    receiveProfile('0xabc', RICH)
    act(() => {
      openPassport('0xabc')
    })
    expect(screen.queryByRole('button', { name: 'Name color' })).toBeNull()
  })

  it('previews the pick on the name and deploys it once, as the passport closes', async () => {
    const s = openOwn()
    await pickPreset('#ff4046')
    expect(screen.getByText('Me').style.getPropertyValue('--name-color')).toBe('#ff4046')
    await pickPreset('#40eaff')
    expect(s.profile.saveNameColor).not.toHaveBeenCalled()

    // Cancel closes the passport even with the picker open: popups are dispatched before leaf
    // cancel layers (lib/cancelLayers). The pick is kept, not lost.
    act(() => closeTopPopup())
    expect(screen.queryByRole('button', { name: 'Name color' })).toBeNull()
    expect(s.profile.saveNameColor).toHaveBeenCalledTimes(1)
    expect(s.profile.saveNameColor).toHaveBeenCalledWith({ r: 0x40 / 255, g: 0xea / 255, b: 1 })
  })

  it('deploys nothing when the colour picked is the one already saved', async () => {
    const s = openOwn({ ...ME, nameColor: { r: 1, g: 0x40 / 255, b: 0x46 / 255 } })
    await pickPreset('#40eaff')
    await pickPreset('#ff4046')
    act(() => closeTopPopup())
    expect(s.profile.saveNameColor).not.toHaveBeenCalled()
  })

  it('deploys nothing when no colour was picked', () => {
    const s = openOwn()
    act(() => closeTopPopup())
    expect(s.profile.saveNameColor).not.toHaveBeenCalled()
  })

  it('drops the picked colour with the form edits on Discard', async () => {
    const s = openOwn()
    await pickPreset('#ff4046')
    await userEvent.click(screen.getByRole('button', { name: 'Edit profile' }))
    await userEvent.type(screen.getByLabelText('About me'), '!')
    act(() => closeTopPopup())
    await userEvent.click(await screen.findByRole('button', { name: 'Discard' }))
    expect(screen.queryByLabelText('About me')).toBeNull()
    expect(s.profile.saveNameColor).not.toHaveBeenCalled()
  })
})
