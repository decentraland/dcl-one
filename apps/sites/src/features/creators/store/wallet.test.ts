// @vitest-environment jsdom
import '@/data/lib/creators/test/setup'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { generatePrivateKey } from 'viem/accounts'
import { createIdentityFromPrivateKey } from '@/data/lib/auth/identity'
import { clearIdentity, setIdentity } from '@/data/lib/auth/session'
import { currentAddress } from '@/data/lib/creators/currentAddress'
import { useWallet } from './wallet'

const signin = vi.hoisted(() => ({ openSignIn: vi.fn() }))
vi.mock('../../components/auth/signin-store', () => signin)

async function signIn(): Promise<string> {
  const identity = await createIdentityFromPrivateKey(generatePrivateKey())
  setIdentity(identity)
  return identity.signer
}

afterEach(() => {
  clearIdentity()
  useWallet.setState({ session: null, restored: false })
})

describe('wallet store', () => {
  it('does not know the visitor until the session has been read', () => {
    expect(useWallet.getState()).toMatchObject({ session: null, restored: false })
  })

  it('reports a visitor as restored and signed out', () => {
    useWallet.getState().restore()

    expect(useWallet.getState()).toMatchObject({ session: null, restored: true })
  })

  it('holds the account Sites is signed in with and names it to the rest of the code', async () => {
    const address = await signIn()

    useWallet.getState().restore()

    expect(useWallet.getState().session).toMatchObject({ address })
    expect(currentAddress()).toBe(address)
  })

  it('follows a sign-in and a sign-out that happen after it was restored', async () => {
    useWallet.getState().restore()

    const address = await signIn()
    expect(useWallet.getState().session).toMatchObject({ address })

    clearIdentity()
    expect(useWallet.getState().session).toBeNull()
  })

  it('keeps the same session object while the account does not change', async () => {
    await signIn()
    useWallet.getState().restore()
    const session = useWallet.getState().session

    useWallet.getState().restore()

    expect(useWallet.getState().session).toBe(session)
  })

  it("opens Sites' sign-in", () => {
    useWallet.getState().signIn()

    expect(signin.openSignIn).toHaveBeenCalled()
  })
})
