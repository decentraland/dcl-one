// The key code only creating an identity needs, loaded on demand: reads, writes and session restores go without it.
import { type Hex } from 'viem'
import { generatePrivateKey, privateKeyToAccount } from 'viem/accounts'
import { Authenticator, type AuthIdentity } from '@dcl/crypto'
import { type Eip1193Provider } from './auth'
import { walletClientOf } from './walletClient'

/** The key pair the auth chain delegates to: a fresh random one unless `privateKey` is given. */
export function createEphemeralIdentity(privateKey?: Hex) {
  const key = privateKey ?? generatePrivateKey()
  const account = privateKeyToAccount(key)
  return { address: account.address, publicKey: account.publicKey, privateKey: key }
}

/** A new auth chain delegating to a fresh ephemeral key, which costs the wallet one signature. */
export async function createIdentity(
  provider: Eip1193Provider,
  address: string,
  expirationMinutes: number
): Promise<AuthIdentity> {
  const wallet = walletClientOf(provider, address)
  return Authenticator.initializeAuthChain(address, createEphemeralIdentity(), expirationMinutes, message =>
    wallet.signMessage({ message })
  )
}
