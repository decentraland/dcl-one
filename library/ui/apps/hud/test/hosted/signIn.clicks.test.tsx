// The sign-in screens after LOGIN OR SIGNUP: the methods, the email code, and what each answer does.
import { StrictMode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { PopupHost, resetPopups } from '../../design'
import { EmailLoginError } from '@explorer-core/features/auth/thirdweb'
import { SessionProvider } from '../../features/session/SessionContext'
import { StartupFlow } from '../../features/startup/StartupFlow'
import type { LoginFlow } from '../../features/session/useEngineSession'
import { renderSettled, fakeSession } from '../harness'

const EMAIL = 'player@example.com'

function flow(over: Partial<LoginFlow> = {}): LoginFlow {
  return { ...fakeSession().login, ...over }
}

async function methods(f: LoginFlow): Promise<void> {
  // StrictMode, as the dev server runs it: a screen mounted twice still takes its answers
  await renderSettled(
    <StrictMode>
      <SessionProvider value={fakeSession()}>
        <StartupFlow flow={f} music={{ muted: false, toggleMute: vi.fn() }} />
        <PopupHost />
      </SessionProvider>
    </StrictMode>
  )
  await userEvent.click(screen.getByRole('button', { name: /LOGIN OR SIGNUP/i }))
}

async function toCode(f: LoginFlow): Promise<void> {
  await methods(f)
  await userEvent.type(screen.getByRole('textbox', { name: 'Email' }), EMAIL)
  await userEvent.click(screen.getByRole('button', { name: /NEXT/i }))
  await screen.findByRole('heading', { name: 'Enter verification code' })
}

describe('sign-in methods', () => {
  afterEach(() => {
    resetPopups()
    vi.useRealTimers()
  })

  it('Google, MetaMask and the More options methods sign in with that method; Fortmatic is not offered', async () => {
    const f = flow()
    await methods(f)
    await userEvent.click(screen.getByRole('button', { name: /GOOGLE/i }))
    expect(f.startWithAccount).toHaveBeenLastCalledWith('google')
    await userEvent.click(screen.getByRole('button', { name: /METAMASK/i }))
    expect(f.startWithAccount).toHaveBeenLastCalledWith('metamask')

    expect(screen.queryByRole('button', { name: 'Discord' })).toBeNull()
    const more = screen.getByRole('button', { name: /MORE OPTIONS/i })
    await userEvent.click(more)
    expect(more).toHaveAttribute('aria-expanded', 'true')
    for (const [name, method] of [['Discord', 'discord'], ['Apple', 'apple'], ['X', 'x'], ['Coinbase', 'coinbase'], ['WalletConnect', 'wallet-connect']]) {
      await userEvent.click(screen.getByRole('button', { name }))
      expect(f.startWithAccount).toHaveBeenLastCalledWith(method)
    }
    expect(screen.queryByRole('button', { name: /Fortmatic/i })).toBeNull()
  })

  it("NEXT waits for an address; the code is sent and its screen names the address", async () => {
    const f = flow()
    await methods(f)
    const next = screen.getByRole('button', { name: /NEXT/i })
    expect(next).toBeDisabled()
    await userEvent.type(screen.getByRole('textbox', { name: 'Email' }), 'player@')
    expect(next).toBeDisabled()
    await userEvent.type(screen.getByRole('textbox', { name: 'Email' }), 'example.com{Enter}')
    expect(f.sendEmailCode).toHaveBeenCalledWith(EMAIL)
    await screen.findByRole('heading', { name: 'Enter verification code' })
    expect(screen.getByText(EMAIL).tagName).toBe('B')
  })

  it('an address thirdweb refuses is marked Invalid email; any other failure shows Connection Error', async () => {
    const f = flow({ sendEmailCode: vi.fn(async () => Promise.reject(new EmailLoginError('invalid-email', 'thirdweb 400'))) })
    await methods(f)
    await userEvent.type(screen.getByRole('textbox', { name: 'Email' }), `${EMAIL}{Enter}`)
    expect(await screen.findByText('Invalid email')).toBeInTheDocument()
    expect(screen.getByRole('textbox', { name: 'Email' })).toHaveAttribute('aria-invalid', 'true')

    vi.mocked(f.sendEmailCode).mockRejectedValueOnce(new Error('thirdweb 500'))
    await userEvent.type(screen.getByRole('textbox', { name: 'Email' }), '{Enter}')
    const dialog = await screen.findByRole('dialog')
    expect(within(dialog).getByText('Connection Error')).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Log in or Sign up' })).toBeInTheDocument()
  })
})

describe('verification code', () => {
  afterEach(() => resetPopups())

  it('a full code is checked at once and a right one shows Success', async () => {
    const f = flow()
    await toCode(f)
    await userEvent.type(screen.getByRole('textbox', { name: 'Verification code' }), '123456')
    expect(f.verifyEmailCode).toHaveBeenCalledWith(EMAIL, '123456', expect.any(AbortSignal))
    expect(await screen.findByText('Success')).toBeInTheDocument()
  })

  it("a right code whose wallet won't sign here goes on to the sign-in page; an accepted code can't be left", async () => {
    const f = flow({ verifyEmailCode: vi.fn(async () => Promise.reject(new EmailLoginError('sign-refused', 'thirdweb 403'))) })
    await toCode(f)
    await userEvent.type(screen.getByRole('textbox', { name: 'Verification code' }), '123456')
    await waitFor(() => expect(f.startWithAccount).toHaveBeenCalledWith())

    vi.mocked(f.verifyEmailCode).mockResolvedValueOnce(undefined)
    await userEvent.type(screen.getByRole('textbox', { name: 'Verification code' }), '123456')
    expect(await screen.findByText('Success')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /BACK/i })).toBeNull()
  })

  it('a wrong code shows Incorrect code, then clears for another try', async () => {
    const f = flow({ verifyEmailCode: vi.fn(async () => Promise.reject(new Error('EmailLoginError: [incorrect-code] thirdweb 400'))) })
    await toCode(f)
    const field = screen.getByRole('textbox', { name: 'Verification code' })
    await userEvent.type(field, '000000')
    expect(await screen.findByText('Incorrect code')).toBeInTheDocument()
    await waitFor(() => expect(screen.queryByText('Incorrect code')).toBeNull(), { timeout: 2500 })
    expect(screen.getByRole('textbox', { name: 'Verification code' })).toHaveValue('')
  })

  it('an address refused at the code goes back to the methods, marked; a failure shows Connection Error', async () => {
    const f = flow({ verifyEmailCode: vi.fn(async () => Promise.reject(new EmailLoginError('invalid-email', 'thirdweb 400'))) })
    await toCode(f)
    await userEvent.type(screen.getByRole('textbox', { name: 'Verification code' }), '123456')
    await screen.findByRole('heading', { name: 'Log in or Sign up' })
    expect(screen.getByRole('textbox', { name: 'Email' })).toHaveValue(EMAIL)
    expect(screen.getByText('Invalid email')).toBeInTheDocument()

    vi.mocked(f.verifyEmailCode).mockRejectedValueOnce(new Error('thirdweb 500'))
    await userEvent.type(screen.getByRole('textbox', { name: 'Email' }), 'x')
    await userEvent.clear(screen.getByRole('textbox', { name: 'Email' }))
    await userEvent.type(screen.getByRole('textbox', { name: 'Email' }), `${EMAIL}{Enter}`)
    await screen.findByRole('heading', { name: 'Enter verification code' })
    await userEvent.type(screen.getByRole('textbox', { name: 'Verification code' }), '123456')
    expect(within(await screen.findByRole('dialog')).getByText('Connection Error')).toBeInTheDocument()
  })

  it('Back stops a check under way and returns to the methods with the address; Resend Code sends again', async () => {
    let signal: AbortSignal | undefined
    const f = flow({
      verifyEmailCode: vi.fn((_e: string, _c: string, s?: AbortSignal) => {
        signal = s
        return new Promise<void>(() => {})
      })
    })
    await toCode(f)
    await userEvent.click(screen.getByRole('button', { name: /Resend Code/i }))
    expect(f.sendEmailCode).toHaveBeenCalledTimes(2)
    await userEvent.type(screen.getByRole('textbox', { name: 'Verification code' }), '123456')
    await userEvent.click(screen.getByRole('button', { name: /BACK/i }))
    expect(signal?.aborted).toBe(true)
    expect(screen.getByRole('heading', { name: 'Log in or Sign up' })).toBeInTheDocument()
    expect(screen.getByRole('textbox', { name: 'Email' })).toHaveValue(EMAIL)
  })
})
