// The screens after a sign-in: Verifying, then a new account's starting look and name.
import { afterEach, describe, expect, it, vi } from 'vitest'
import { act, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { PopupHost, resetPopups } from '../../design'
import { SessionProvider } from '../../features/session/SessionContext'
import { StartupFlow } from '../../features/startup/StartupFlow'
import { PRESETS } from '../../features/startup/presets'
import { VERIFY_TIMED_OUT, type EngineSession, type WelcomeFlow } from '../../features/session/useEngineSession'
import { FakeDriver, fakeSession, renderSession, renderSettled } from '../harness'

function welcomeSession(welcome: Partial<WelcomeFlow>, over: Partial<EngineSession> = {}): EngineSession {
  const session = fakeSession()
  return { ...session, phase: 'welcome', logout: vi.fn(), ...over, welcome: { ...session.welcome, ...welcome } }
}

async function show(session: EngineSession): Promise<void> {
  await renderSettled(
    <SessionProvider value={session}>
      <StartupFlow flow={session.login} music={{ muted: false, toggleMute: vi.fn() }} />
      <PopupHost />
    </SessionProvider>
  )
}

describe('after sign-in', () => {
  afterEach(() => resetPopups())

  it('shows Verifying while the account loads, and an existing account too (its terms answered for it); Cancel stops it', async () => {
    const session = welcomeSession({ pending: true })
    await show(session)
    expect(screen.getByText('Verifying your account...')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(session.login.cancelSignIn).toHaveBeenCalled()
  })

  it('a new account picks a starting look: body type A, first look first; a look and a body type are worn at once', async () => {
    const session = welcomeSession({ pending: false, newProfile: true })
    await show(session)
    expect(screen.getByRole('heading', { name: 'Choose a Starting Look' })).toBeInTheDocument()
    expect(session.welcome.showLook).toHaveBeenLastCalledWith(1, PRESETS.a[0].look)
    expect(screen.getByRole('radio', { name: 'Look 1' })).toHaveAttribute('aria-checked', 'true')

    await userEvent.click(screen.getByRole('radio', { name: 'Look 3' }))
    expect(session.welcome.showLook).toHaveBeenLastCalledWith(3, PRESETS.a[2].look)
    // body type B keeps the slot
    await userEvent.click(screen.getByRole('button', { name: /BODY TYPE A/i }))
    await userEvent.click(within(screen.getByRole('menu', { name: 'Body type' })).getByRole('menuitemradio', { name: /BODY TYPE B/i }))
    expect(session.welcome.showLook).toHaveBeenLastCalledWith(7, PRESETS.b[2].look)
    expect(screen.queryByRole('menu')).toBeNull()

    await userEvent.click(screen.getByRole('button', { name: /BACK/i }))
    expect(session.logout).toHaveBeenCalled()
  })

  it("opens the engine's sign-in stage for the avatar, keeping the page's background until the engine has drawn its own", async () => {
    const session = welcomeSession({ pending: false, newProfile: true })
    await show(session)
    expect(session.setEngineViewport).toHaveBeenCalledWith('signin', expect.objectContaining({ width: expect.any(Number) }), expect.any(Number))
    const backdrop = document.querySelector('[class*="backdrop"]')
    expect(backdrop?.className).not.toMatch(/backdropAway/)
  })

  it('fades the background out once the sign-in stage is drawn', async () => {
    await show(welcomeSession({ pending: false, newProfile: true }, { signinStageReady: true }))
    expect(document.querySelector('[class*="backdrop"]')?.className).toMatch(/backdropAway/)
  })

  it('the name screen: JUMP IN needs a valid name and the terms; the newsletter is only for an email account', async () => {
    const session = welcomeSession({ pending: false, newProfile: true, email: 'player@example.com' })
    await show(session)
    await userEvent.click(screen.getByRole('button', { name: /CONTINUE/i }))
    expect(screen.getByRole('heading', { name: 'Welcome to Decentraland!' })).toBeInTheDocument()
    const jump = screen.getByRole('button', { name: /JUMP IN/i })
    expect(jump).toBeDisabled()

    const name = screen.getByRole('textbox', { name: 'Username' })
    await userEvent.type(name, 'Nova River')
    // letters and numbers only
    expect(name).toHaveValue('NovaRiver')
    expect(screen.getByText('9/15')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('checkbox', { name: /Terms of Use/i }))
    expect(jump).toBeEnabled()
    await userEvent.click(screen.getByRole('checkbox', { name: 'Subscribe to newsletter' }))
    await userEvent.click(jump)
    expect(session.welcome.accept).toHaveBeenCalledWith('NovaRiver', true)

    await userEvent.clear(name)
    expect(screen.getByText("Name can't be empty.")).toBeInTheDocument()
    expect(jump).toBeDisabled()
    await userEvent.type(name, 'A'.repeat(16))
    expect(screen.getByText('Character limit reached')).toBeInTheDocument()
  })

  it('signing out from the name screen and in again starts the new account on its look', async () => {
    const session = welcomeSession({ pending: false, newProfile: true })
    const view = (s: EngineSession): React.JSX.Element => (
      <SessionProvider value={s}>
        <StartupFlow flow={s.login} music={{ muted: false, toggleMute: vi.fn() }} />
        <PopupHost />
      </SessionProvider>
    )
    const { rerender } = await renderSettled(view(session))
    await userEvent.click(screen.getByRole('radio', { name: 'Look 3' }))
    await userEvent.click(screen.getByRole('button', { name: /CONTINUE/i }))
    // BACK on the name screen signs out: the first screen, then a new guest's welcome
    await act(async () => rerender(view({ ...session, phase: 'login' })))
    await act(async () => rerender(view(session)))
    expect(screen.getByRole('heading', { name: 'Choose a Starting Look' })).toBeInTheDocument()
    expect(screen.getByRole('radio', { name: 'Look 1' })).toHaveAttribute('aria-checked', 'true')
    expect(session.welcome.showLook).toHaveBeenLastCalledWith(1, PRESETS.a[0].look)
  })

  it("a guest's name screen has no newsletter", async () => {
    await show(welcomeSession({ pending: false, newProfile: true, email: null }))
    await userEvent.click(screen.getByRole('button', { name: /CONTINUE/i }))
    expect(screen.queryByRole('checkbox', { name: 'Subscribe to newsletter' })).toBeNull()
  })

  it("a new account's save that fails shows Connection Error on its name screen", async () => {
    await show(welcomeSession({ pending: false, newProfile: true, error: 'failed to deploy to server.' }))
    expect(within(await screen.findByRole('dialog')).getByText('Connection Error')).toBeInTheDocument()
  })
})

describe('the session after sign-in', () => {
  afterEach(() => {
    vi.useRealTimers()
    vi.unstubAllGlobals()
  })

  it('gives up Verifying after 15 seconds: back to the first screen with the error, the stored sign-in kept', async () => {
    const driver = new FakeDriver()
    driver.welcome = null
    const h = renderSession({ userId: null }, driver)
    await waitFor(() => expect(h.session().login.status).toBe('sign-in-or-guest'))
    vi.useFakeTimers({ shouldAdvanceTime: true })
    act(() => h.session().login.exploreAsGuest())
    await waitFor(() => expect(h.session().phase).toBe('welcome'))
    await act(async () => vi.advanceTimersByTime(15_500))
    await waitFor(() => expect(h.session().phase).toBe('login'))
    expect(h.session().login.status).toBe('sign-in-or-guest')
    expect(h.session().login.error).toBe(VERIFY_TIMED_OUT)
    expect(driver.calls).toContain('logout')
  })

  it('the sign-in stage is ready once the engine says so, and again only after it is closed and redrawn', async () => {
    const h = renderSession({ userId: null })
    await waitFor(() => expect(h.session().login.status).toBe('sign-in-or-guest'))
    act(() => h.session().setEngineViewport('signin', { x: 0, y: 0, width: 10, height: 10 }, 1))
    expect(h.session().signinStageReady).toBe(false)
    act(() => h.driver.emit({ kind: 'signinStageReady' }))
    await waitFor(() => expect(h.session().signinStageReady).toBe(true))
    act(() => h.session().setEngineViewport('signin', null))
    expect(h.session().signinStageReady).toBe(false)
  })

  it("an email account's newsletter opt-in goes to the builder service with the address", async () => {
    const posts: { url: string; body: string }[] = []
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string, init?: RequestInit) => {
        if (init?.method === 'POST') posts.push({ url, body: String(init.body) })
        return new Response('{}', { status: 404 })
      })
    )
    class EmailDriver extends FakeDriver {
      async emailSendCode(): Promise<void> {}
      async emailVerify(): Promise<never> {
        return {
          ephemeralIdentity: { address: '0x00000000000000000000000000000000000000e1', publicKey: '0x04' },
          expiration: new Date(Date.now() + 3_600_000).toISOString(),
          authChain: [{ type: 'SIGNER', payload: '0x00000000000000000000000000000000000e4a11', signature: '' }]
        } as never
      }
      async loginEmail(): Promise<void> {}
    }
    const driver = new EmailDriver()
    driver.welcome = { terms: true, newProfile: true }
    const h = renderSession({ userId: null }, driver)
    await waitFor(() => expect(h.session().login.status).toBe('sign-in-or-guest'))
    await act(() => h.session().login.verifyEmailCode('player@example.com', '123456'))
    await waitFor(() => expect(h.session().welcome.newProfile).toBe(true))
    expect(h.session().welcome.email).toBe('player@example.com')
    act(() => h.session().welcome.accept('NovaRiver', true))
    expect(posts).toEqual([{ url: 'https://builder-api.decentraland.org/v1/newsletter', body: JSON.stringify({ email: 'player@example.com', source: 'auth' }) }])
  })
})
