// The wallet client reads, writes and signatures share; kept apart from the key code in ./signer, which only
// creating an identity needs.
import { createWalletClient, custom, type Account, type Address, type CustomTransport, type WalletClient } from 'viem'
import { type Eip1193Provider } from './auth'

// Never retried: a wallet request may be a prompt, and a retry would show it again.
export function walletClientOf(
  provider: Eip1193Provider,
  address: string
): WalletClient<CustomTransport, undefined, Account> {
  return createWalletClient({ account: address as Address, transport: custom(provider, { retryCount: 0 }) })
}
