import { afterEach, describe, expect, it, vi } from 'vitest'
import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { PopupHost, resetPopups } from '../../design'
import { BootProgress } from '../../features/engine/BootProgress'
import { SessionProvider } from '../../features/session/SessionContext'
import { StartupFlow } from '../../features/startup/StartupFlow'
import type { LoginMusic } from '../../features/startup/useLoginMusic'
import { VERIFY_TIMED_OUT, type EngineSession, type LoginFlow } from '../../features/session/useEngineSession'
import { renderSettled, fakeSession } from '../harness'

vi.mock('../../features/gate/exitApplication', async (importOriginal) => ({ ...(await importOriginal<object>()), exitApplication: vi.fn() }))
// a build the host recorded; the desktop client where a test says so
let isNative = false
vi.mock('@hud-host', async (importOriginal) => ({ ...(await importOriginal<object>()), BUILD_VERSION: 'a1b2c3d', isNativeHud: () => isNative }))
const { exitApplication } = await import('../../features/gate/exitApplication')

function flow(over: Partial<LoginFlow>): LoginFlow {
  return { ...fakeSession().login, ...over }
}

const music = (over: Partial<LoginMusic> = {}): LoginMusic => ({ muted: false, toggleMute: vi.fn(), ...over })

// the footer reads the session (Help on the desktop client); the popup layer shows Connection Error
function startup(f: LoginFlow, m: LoginMusic = music(), session: EngineSession = fakeSession()): React.JSX.Element {
  return (
    <SessionProvider value={session}>
      <StartupFlow flow={f} music={m} />
      <PopupHost />
    </SessionProvider>
  )
}

describe('startup screens', () => {
  afterEach(() => {
    resetPopups()
    isNative = false
    vi.unstubAllGlobals()
    vi.mocked(exitApplication).mockClear()
  })

  it('no session: PLAY AS GUEST plays as a guest with no pick (the name and terms page follows), LOGIN OR SIGNUP opens the methods and BACK returns', async () => {
    const f = flow({ status: 'sign-in-or-guest' })
    await renderSettled(startup(f))
    expect(screen.getByRole('heading', { name: 'Welcome to Decentraland!' })).toBeInTheDocument()
    expect(screen.getByText('Discover a world shaped by its community!')).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: /PLAY AS GUEST/i }))
    expect(vi.mocked(f.exploreAsGuest)).toHaveBeenCalledWith()
    await userEvent.click(screen.getByRole('button', { name: /LOGIN OR SIGNUP/i }))
    expect(screen.getByRole('heading', { name: 'Log in or Sign up' })).toBeInTheDocument()
    expect(f.startWithAccount).not.toHaveBeenCalled()
    await userEvent.click(screen.getByRole('button', { name: /BACK/i }))
    expect(screen.getByRole('heading', { name: 'Welcome to Decentraland!' })).toBeInTheDocument()
  })

  it('the splash covers loading and a stored sign-in on its way to the lobby; a failed one is back on the first screen with Connection Error', async () => {
    const { unmount } = await renderSettled(startup(flow({ status: 'loading' })))
    expect(screen.getByRole('status', { name: 'Loading' })).toBeInTheDocument()
    expect(screen.queryByRole('button')).toBeNull()
    unmount()
    const stored = await renderSettled(startup(flow({ status: 'reuse-login-or-new', account: '0xabc' })))
    expect(screen.getByRole('status', { name: 'Loading' })).toBeInTheDocument()
    stored.unmount()
    await renderSettled(startup(flow({ status: 'sign-in-or-guest', account: '0xabc', error: 'Login failed' })))
    expect(screen.getByRole('heading', { name: 'Welcome to Decentraland!' })).toBeInTheDocument()
    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByText(/We couldn't connect to the server/)).toBeInTheDocument()
  })

  it('the desktop browser sign-in waits on the methods screen, with a spinner and Cancel', async () => {
    const f = flow({ status: 'sign-in-or-guest', authPending: true })
    await renderSettled(startup(f))
    expect(screen.getByRole('heading', { name: 'Log in or Sign up' })).toBeInTheDocument()
    expect(screen.getByRole('status', { name: 'Loading' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /GOOGLE/i })).toBeNull()
    await userEvent.click(screen.getByRole('button', { name: /CANCEL/i }))
    expect(vi.mocked(f.cancelLogin)).toHaveBeenCalledTimes(1)
  })

  it("shows the browser sign-in's code once the engine has it, to match against the browser's", async () => {
    const f = flow({ status: 'sign-in-or-guest', authPending: true })
    const { rerender } = await renderSettled(startup(f))
    expect(screen.queryByText(/Verify this code matches/)).toBeNull()
    rerender(startup({ ...f, authCode: '42' }))
    expect(screen.getByText(/Verify this code matches the one in your browser/)).toBeInTheDocument()
    expect(screen.getByLabelText('Sign-in code')).toHaveTextContent('42')
  })

  it('a checked email code waiting for the engine to boot shows Verifying, not a frozen code screen', async () => {
    await renderSettled(startup(flow({ status: 'sign-in-or-guest', engineReady: false, emailQueued: true })))
    expect(screen.getByText('Verifying your account...')).toBeInTheDocument()
  })

  it('CTAs are disabled while busy', async () => {
    await renderSettled(startup(flow({ status: 'sign-in-or-guest', busy: true })))
    expect(screen.getByRole('button', { name: /PLAY AS GUEST/i })).toBeDisabled()
    expect(screen.getByRole('button', { name: /LOGIN OR SIGNUP/i })).toBeDisabled()
  })

  it('shows the engine boot progress while loading; PLAY AS GUEST takes the click and shows the progress until the engine runs it', async () => {
    const { rerender: rerenderBar, unmount } = await renderSettled(<BootProgress flow={flow({ status: 'loading', engineReady: false, avatarLoaded: false, loadProgress: 42, loadStep: 'download' })} />)
    const bar = screen.getByRole('progressbar')
    expect(bar).toHaveTextContent(/Downloading engine · 42%/i)
    expect(bar).toHaveAttribute('aria-valuenow', '42')
    // the engine can take commands but the lobby stage's avatar hasn't loaded yet: held short of done
    rerenderBar(<BootProgress flow={flow({ status: 'reuse-login-or-new', engineReady: true, avatarLoaded: false, loadProgress: 100, loadStep: null })} />)
    expect(screen.getByRole('progressbar')).toHaveTextContent(/Starting… · 95%/i)
    // the first avatar loaded: full for a moment, then gone
    rerenderBar(<BootProgress flow={flow({ status: 'reuse-login-or-new', engineReady: true, avatarLoaded: true, loadProgress: 100, loadStep: null })} />)
    expect(screen.getByRole('progressbar')).toHaveTextContent(/Ready · 100%/i)
    await waitFor(() => expect(screen.queryByRole('progressbar')).toBeNull(), { timeout: 2000 })
    unmount()

    const f = flow({ status: 'sign-in-or-guest', engineReady: false, loadProgress: 42, loadStep: 'download' })
    const { rerender } = await renderSettled(startup(f))
    const play = screen.getByRole('button', { name: /PLAY AS GUEST/i })
    expect(play).toBeEnabled()
    await userEvent.click(play)
    expect(vi.mocked(f.exploreAsGuest)).toHaveBeenCalledTimes(1)
    // the click is kept: the CTA dims and shows the live download percent instead of "STARTING…"
    rerender(startup({ ...f, acceptQueued: true }))
    const gated = screen.getByRole('button', { name: /DOWNLOADING/i })
    expect(gated).toHaveTextContent(/DOWNLOADING…?\s*42%/i)
    expect(gated).toBeDisabled()
    expect(screen.getByRole('button', { name: /LOGIN OR SIGNUP/i })).toBeDisabled()
    rerender(startup(flow({ status: 'sign-in-or-guest', engineReady: true })))
    expect(screen.getByRole('button', { name: /PLAY AS GUEST/i })).toBeEnabled()
  })

  it('the first screen with no session shows no stage: the bar is done once the engine can take commands', async () => {
    await renderSettled(<BootProgress flow={flow({ status: 'sign-in-or-guest', engineReady: true, avatarLoaded: false, loadProgress: 100, loadStep: null })} />)
    expect(screen.getByRole('progressbar')).toHaveTextContent(/Ready · 100%/i)
    await waitFor(() => expect(screen.queryByRole('progressbar')).toBeNull(), { timeout: 2000 })
  })

  it('an account sign-in that fails keeps what went wrong', async () => {
    const f = flow({ status: 'sign-in-or-guest' })
    const { rerender } = await renderSettled(startup(f))
    await userEvent.click(screen.getByRole('button', { name: /LOGIN OR SIGNUP/i }))
    await userEvent.click(screen.getByRole('button', { name: /GOOGLE/i }))
    rerender(startup({ ...f, error: 'nothing on this mac handles decentraland:// links' }))
    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByText('nothing on this mac handles decentraland:// links')).toBeInTheDocument()
    expect(within(dialog).queryByText(/We couldn't connect to the server/)).toBeNull()
  })

  it('an account sign-in whose Verifying gave up shows the plain Connection Error, not its internal reason', async () => {
    const f = flow({ status: 'sign-in-or-guest' })
    const { rerender } = await renderSettled(startup(f))
    await userEvent.click(screen.getByRole('button', { name: /LOGIN OR SIGNUP/i }))
    await userEvent.click(screen.getByRole('button', { name: /GOOGLE/i }))
    rerender(startup({ ...f, error: VERIFY_TIMED_OUT }))
    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).queryByText(VERIFY_TIMED_OUT)).toBeNull()
    expect(within(dialog).getByText(/We couldn't connect to the server/)).toBeInTheDocument()
  })

  it('the desktop client offers no Exit: not in the footer, not in Connection Error', async () => {
    isNative = true
    await renderSettled(startup(flow({ status: 'sign-in-or-guest', error: 'thirdweb 500' })))
    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).queryByRole('button', { name: 'Exit' })).toBeNull()
    expect(within(dialog).getByRole('button', { name: 'Retry' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /^EXIT$/i })).toBeNull()
  })

  it('a guest sign-in that fails shows Connection Error: Retry closes it, Exit leaves', async () => {
    const { rerender } = await renderSettled(startup(flow({ status: 'sign-in-or-guest', error: 'thirdweb 500' })))
    let dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByText('Connection Error')).toBeInTheDocument()
    expect(within(dialog).getByText(/We couldn't connect to the server/)).toBeInTheDocument()
    await userEvent.click(within(dialog).getByRole('button', { name: 'Retry' }))
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
    expect(exitApplication).not.toHaveBeenCalled()
    expect(screen.getByRole('button', { name: /PLAY AS GUEST/i })).toBeEnabled()

    // the next failure shows it again
    rerender(startup(flow({ status: 'sign-in-or-guest', error: null })))
    rerender(startup(flow({ status: 'sign-in-or-guest', error: 'thirdweb 500' })))
    dialog = await screen.findByRole('dialog')
    await userEvent.click(within(dialog).getByRole('button', { name: 'Exit' }))
    expect(exitApplication).toHaveBeenCalledTimes(1)
  })

  it('the footer: Exit leaves, the music mutes, Help opens the help page, the build is named', async () => {
    const open = vi.fn()
    vi.stubGlobal('open', open)
    const m = music()
    const { rerender } = await renderSettled(startup(flow({ status: 'sign-in-or-guest' }), m))
    await userEvent.click(screen.getByRole('button', { name: /EXIT/i }))
    expect(exitApplication).toHaveBeenCalledTimes(1)
    const mute = screen.getByRole('button', { name: 'Mute music' })
    expect(mute).toHaveAttribute('aria-pressed', 'false')
    await userEvent.click(mute)
    expect(m.toggleMute).toHaveBeenCalledTimes(1)
    rerender(startup(flow({ status: 'sign-in-or-guest' }), music({ muted: true })))
    expect(screen.getByRole('button', { name: 'Unmute music' })).toHaveAttribute('aria-pressed', 'true')
    await userEvent.click(screen.getByRole('button', { name: 'Help' }))
    expect(open).toHaveBeenCalledWith('https://decentraland.org/help/', '_blank', 'noopener')
    expect(screen.getByText('a1b2c3d')).toBeInTheDocument()
  })
})
