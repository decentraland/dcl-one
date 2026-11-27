import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest'
import { screen, act, fireEvent } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Passport, openPassport } from '../../features/profile/Passport'
import { SessionProvider } from '../../features/session/SessionContext'
import { fakeProfileState, fakeSession, renderSettled } from '../harness'
import { receiveProfile, seedProfiles, setProfileRequester } from '../../features/session/profileStore'
import { setPublicReelFetcher } from '../../features/gallery/publicReels'
import { PopupHost, closeTopPopup, resetPopups } from '../../design'
import type { EngineSession } from '../../features/session/useEngineSession'
import type { Profile } from '@explorer-core/engine/protocol'
import { pending } from '@explorer-core/lib/analytics'
import passportStyles from '../../features/profile/ProfilePassport.module.css'

// COMPONENT: the smart <Passport userId> fetches the rich profile on open, renders identity-only from
// the profile store until it lands, and shows the presentational ProfilePassport; openPassport mounts it.
afterEach(resetPopups)

const request = vi.fn()
beforeEach(() => {
  request.mockClear()
  setProfileRequester(request)
})

async function renderWithSession(node: React.ReactNode, mutate?: (s: EngineSession) => void): Promise<EngineSession> {
  const s = fakeSession()
  mutate?.(s)
  await renderSettled(<SessionProvider value={s}>{node}</SessionProvider>)
  return s
}

const RICH: Profile = {
  address: '0xabc',
  name: 'Alice',
  picture: 'a.png',
  hasClaimedName: true,
  isGuest: false,
  description: 'gm from the plaza'
}

describe('Passport container', () => {
  it('fetches the rich profile on open and renders it', async () => {
    receiveProfile('0xabc', RICH)
    await renderWithSession(<Passport userId="0xabc" onClose={vi.fn()} />)
    expect(request).toHaveBeenCalledWith('0xabc', true, expect.any(Number))
    expect(screen.getByText('gm from the plaza')).toBeTruthy() // rich field from the fetched profile
  })

  it("asks for another user's mutual friends and shows them from the session", async () => {
    receiveProfile('0xabc', RICH)
    const s = await renderWithSession(<Passport userId="0xABC" onClose={vi.fn()} />, (s) => {
      s.profile = fakeProfileState({ data: { ...RICH, address: '0xme', name: 'Me' } })
      s.friends.mutuals = { '0xabc': [{ address: '0x1', name: 'Mojito' }, { address: '0x2', name: 'Sharknado' }] }
    })
    expect(s.friends.loadMutuals).toHaveBeenCalledWith('0xABC')
    expect(screen.getByText('2 Mutual')).toBeTruthy()
  })

  it('asks for no mutual friends on your own passport', async () => {
    receiveProfile('0xme', { ...RICH, address: '0xme', name: 'Me' })
    const s = await renderWithSession(<Passport userId="0xme" onClose={vi.fn()} />, (s) => {
      s.profile = fakeProfileState({ data: { ...RICH, address: '0xme', name: 'Me' } })
      s.friends.mutuals = { '0xme': [{ address: '0x1', name: 'Mojito' }] }
    })
    expect(s.friends.loadMutuals).not.toHaveBeenCalled()
    expect(screen.queryByText(/Mutual/)).toBeNull()
  })

  it('renders identity-only from a roster seed while the fetch is in flight', async () => {
    seedProfiles([{ address: '0xabc', name: 'Alice', picture: 'a.png' }]) // no engine profile yet
    await renderWithSession(<Passport userId="0xabc" onClose={vi.fn()} />)
    expect(request).toHaveBeenCalledWith('0xabc', true, expect.any(Number))
    expect(screen.getByText('Alice')).toBeTruthy() // resolved from the roster
  })

  it('openPassport mounts the passport via the popup layer', async () => {
    receiveProfile('0xabc', RICH)
    await renderWithSession(<PopupHost />)
    act(() => {
      openPassport('0xabc')
    })
    expect(screen.getByText('gm from the plaza')).toBeTruthy()
  })

  it('openPassport puts the passport scrim on its backdrop', async () => {
    receiveProfile('0xabc', RICH)
    await renderWithSession(<PopupHost />)
    act(() => {
      openPassport('0xabc')
    })
    expect(document.getElementsByClassName(passportStyles.scrim)).toHaveLength(1)
  })

  it('openPassport opens on the tab it is given', async () => {
    receiveProfile('0xabc', RICH)
    await renderWithSession(<PopupHost />)
    act(() => {
      openPassport('0xabc', 'badges')
    })
    expect(screen.getByRole('tab', { name: 'BADGES' }).getAttribute('aria-selected')).toBe('true')
    expect(screen.getByRole('tab', { name: 'OVERVIEW' }).getAttribute('aria-selected')).toBe('false')
  })
})

describe('Jump In from a passport photo', () => {
  afterEach(() => setPublicReelFetcher(null))

  it.each([
    ['closes the photo and the passport when the travel is sent', 'sent', 1],
    ['leaves the photo and the passport open when the travel is refused', 'refused', 0]
  ] as const)('%s', async (_name, outcome, closes) => {
    receiveProfile('0xabc', RICH)
    setPublicReelFetcher(async () => ({ ok: true, photos: [{ id: 'p0', url: 'https://img/0.jpg', thumbnailUrl: 'https://img/0_t.jpg', dateTime: '1707000000', isPublic: true }], total: 1, max: 500 }))
    const onClose = vi.fn()
    const s = await renderWithSession(
      <>
        <Passport userId="0xabc" initialTab="photos" onClose={onClose} />
        <PopupHost />
      </>,
      (session) => {
        session.gallery.metas = { p0: { sceneName: 'Genesis Plaza', x: -9, y: 14 } }
        session.map.teleportToPlace = vi.fn(() => outcome)
      }
    )
    const tile = await screen.findByRole('button', { name: 'Open photo' })
    fireEvent.load(tile.querySelector('img') as HTMLImageElement)
    await userEvent.click(tile)
    await userEvent.click(screen.getByRole('button', { name: /Jump In/ }))
    expect(s.map.teleportToPlace).toHaveBeenCalledWith(-9, 14)
    expect(onClose).toHaveBeenCalledTimes(closes)
    expect(screen.queryAllByRole('dialog', { name: 'Photo' })).toHaveLength(1 - closes)
  })
})

// The node's social butterfly badge counts the other players' passports opened; one's own is
// reported apart.
describe('Passport analytics', () => {
  const ME: Profile = { ...RICH, address: '0xme', name: 'Me' }
  const opened = (from: number): unknown[] =>
    pending()
      .slice(from)
      .filter((e) => e.event === 'passport_opened' || e.event === 'profile_opened')
      .map((e) => [e.event, e.properties.receiver_id])
  const signedIn = (s: EngineSession): void => {
    s.profile = fakeProfileState({ data: ME })
  }

  it('reports passport_opened with the receiver for another player', async () => {
    receiveProfile('0xabc', RICH)
    const from = pending().length
    await renderWithSession(<Passport userId="0xabc" onClose={vi.fn()} />, signedIn)
    expect(opened(from)).toEqual([['passport_opened', '0xabc']])
  })

  it('reports profile_opened, not passport_opened, for your own', async () => {
    receiveProfile('0xme', ME)
    const from = pending().length
    await renderWithSession(<Passport userId="0xme" onClose={vi.fn()} />, signedIn)
    expect(opened(from)).toEqual([['profile_opened', '0xme']])
  })

  it('waits for your own profile, so your own passport opened early is still profile_opened', async () => {
    receiveProfile('0xme', ME)
    const from = pending().length
    const s = fakeSession()
    const view = await renderSettled(
      <SessionProvider value={s}>
        <Passport userId="0xme" onClose={vi.fn()} />
      </SessionProvider>
    )
    expect(opened(from)).toEqual([])
    const loaded = { ...s, profile: fakeProfileState({ data: ME }) }
    await act(async () => {
      view.rerender(
        <SessionProvider value={loaded}>
          <Passport userId="0xme" onClose={vi.fn()} />
        </SessionProvider>
      )
    })
    expect(opened(from)).toEqual([['profile_opened', '0xme']])
  })

  it('waits for your own profile before reporting another player\'s passport opened early', async () => {
    receiveProfile('0xabc', RICH)
    const from = pending().length
    const s = fakeSession()
    const view = await renderSettled(
      <SessionProvider value={s}>
        <Passport userId="0xabc" onClose={vi.fn()} />
      </SessionProvider>
    )
    expect(opened(from)).toEqual([])
    const loaded = { ...s, profile: fakeProfileState({ data: ME }) }
    await act(async () => {
      view.rerender(
        <SessionProvider value={loaded}>
          <Passport userId="0xabc" onClose={vi.fn()} />
        </SessionProvider>
      )
    })
    expect(opened(from)).toEqual([['passport_opened', '0xabc']])
  })

  it('reports the same player once when the address changes case', async () => {
    receiveProfile('0xabc', RICH)
    const from = pending().length
    const s = fakeSession()
    signedIn(s)
    const view = await renderSettled(
      <SessionProvider value={s}>
        <Passport userId="0xabc" onClose={vi.fn()} />
      </SessionProvider>
    )
    await act(async () => {
      view.rerender(
        <SessionProvider value={s}>
          <Passport userId="0xABC" onClose={vi.fn()} />
        </SessionProvider>
      )
    })
    expect(opened(from)).toEqual([['passport_opened', '0xabc']])
  })
})

describe('Passport block & report', () => {
  it('blocks after the confirmation, through the friends action', async () => {
    receiveProfile('0xabc', RICH)
    const s = await renderWithSession(<PopupHost />)
    act(() => {
      openPassport('0xabc')
    })
    await userEvent.click(screen.getByRole('button', { name: 'More options' }))
    await userEvent.click(screen.getByRole('menuitem', { name: 'Block' }))
    await userEvent.click(screen.getByRole('button', { name: 'BLOCK' }))
    expect(s.friends.act).toHaveBeenCalledWith('block', '0xabc')
  })

  it('reads blocked-by from the session: no Chat or Call for a user who blocked you', async () => {
    receiveProfile('0xabc', RICH)
    await renderWithSession(<Passport userId="0xabc" onClose={vi.fn()} />, (s) => {
      s.friends.blockedBy = ['0xabc']
    })
    expect(screen.queryByRole('button', { name: 'Chat' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'Call' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'ADD FRIEND' })).toBeNull()
  })

  it('shows Chat and ADD FRIEND for anyone else', async () => {
    receiveProfile('0xabc', RICH)
    await renderWithSession(<Passport userId="0xabc" onClose={vi.fn()} />)
    expect(screen.getByRole('button', { name: 'Chat' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'ADD FRIEND' })).toBeTruthy()
  })
})

// The passport is the HUD's one popup that can hold unsaved work, so it is where the popup layer's
// close guard is exercised: a stray backdrop click is refused outright, while the deliberate paths
// (its ×, and the engine-resolved Cancel action) ask the same question.
describe('closing a passport with unsaved edits', () => {
  const openSelfPassport = async (): Promise<void> => {
    receiveProfile('0xme', { ...RICH, address: '0xme', name: 'Me' })
    await renderWithSession(<PopupHost />, (s) => {
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
    await act(async () => closeTopPopup())
    expect(await screen.findByText('Discard changes?')).toBeTruthy()

    // Escape ON the confirm resolves it as dismissed, which is the non-destructive answer.
    await act(async () => closeTopPopup())
    expect(screen.queryByText('Discard changes?')).toBeNull()
    expect(screen.getByLabelText('About me')).toBeTruthy()
  })

  it('closes the whole passport on Discard', async () => {
    await openSelfPassport()
    await act(async () => closeTopPopup())
    await userEvent.click(await screen.findByRole('button', { name: 'Discard' }))
    expect(screen.queryByLabelText('About me')).toBeNull()
  })

  it('closes without a word when nothing has been edited', async () => {
    receiveProfile('0xabc', RICH)
    await renderWithSession(<PopupHost />)
    act(() => {
      openPassport('0xabc')
    })
    await act(async () => closeTopPopup())
    expect(screen.queryByText('gm from the plaza')).toBeNull()
  })
})

// The name colour: own passport, claimed name only; previewed while open, deployed as it closes
// (unity-explorer PassportController), and dropped with the form on Discard.
describe('the name colour picker', () => {
  const ME: Profile = { ...RICH, address: '0xme', name: 'Me' }
  const openOwn = async (me: Profile = ME): Promise<EngineSession> => {
    receiveProfile('0xme', me)
    const s = await renderWithSession(<PopupHost />, (s) => {
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

  it('is offered on your own passport with a claimed name only', async () => {
    await openOwn({ ...ME, name: 'Me#abcd', hasClaimedName: false })
    expect(screen.queryByRole('button', { name: 'Name color' })).toBeNull()
    await act(async () => closeTopPopup())

    receiveProfile('0xabc', RICH)
    act(() => {
      openPassport('0xabc')
    })
    expect(screen.queryByRole('button', { name: 'Name color' })).toBeNull()
  })

  it('previews the pick on the name and deploys it once, as the passport closes', async () => {
    const s = await openOwn()
    await pickPreset('#ff4046')
    expect(screen.getByText('Me').style.getPropertyValue('--name-color')).toBe('#ff4046')
    await pickPreset('#40eaff')
    expect(s.profile.saveNameColor).not.toHaveBeenCalled()

    // Cancel closes the passport even with the picker open: popups are dispatched before leaf
    // cancel layers (lib/cancelLayers). The pick is kept, not lost.
    await act(async () => closeTopPopup())
    expect(screen.queryByRole('button', { name: 'Name color' })).toBeNull()
    expect(s.profile.saveNameColor).toHaveBeenCalledTimes(1)
    expect(s.profile.saveNameColor).toHaveBeenCalledWith({ r: 0x40 / 255, g: 0xea / 255, b: 1 })
  })

  it('deploys nothing when the colour picked is the one already saved', async () => {
    const s = await openOwn({ ...ME, nameColor: { r: 1, g: 0x40 / 255, b: 0x46 / 255 } })
    await pickPreset('#40eaff')
    await pickPreset('#ff4046')
    await act(async () => closeTopPopup())
    expect(s.profile.saveNameColor).not.toHaveBeenCalled()
  })

  it('deploys nothing when no colour was picked', async () => {
    const s = await openOwn()
    await act(async () => closeTopPopup())
    expect(s.profile.saveNameColor).not.toHaveBeenCalled()
  })

  it('drops the picked colour with the form edits on Discard', async () => {
    const s = await openOwn()
    await pickPreset('#ff4046')
    await userEvent.click(screen.getByRole('button', { name: 'Edit profile' }))
    await userEvent.type(screen.getByLabelText('About me'), '!')
    await act(async () => closeTopPopup())
    await userEvent.click(await screen.findByRole('button', { name: 'Discard' }))
    expect(screen.queryByLabelText('About me')).toBeNull()
    expect(s.profile.saveNameColor).not.toHaveBeenCalled()
  })
})
