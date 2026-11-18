// Raw EIP-1193 requests the navbar needs on the first load (the wallet's network, and switching it), kept free of
// viem so that it stays out of the overview's bundle. Everything that encodes, signs or sends lives in ./transactions.
import { config } from '~/config'
import { WrongAccountError } from '~/lib/walletErrors'
import { type Session } from './auth'

const toHex = (value: number) => `0x${value.toString(16)}`

// Wallets that don't know the chain yet (EIP-3085) answer a switch with 4902 and need it added first. Only this
// environment's Polygon is ever added; its details live in the env config.
function addChainParams(chainId: number): object | undefined {
  if (chainId !== Number(config.get('MATIC_CHAIN_ID'))) return undefined
  return {
    chainId: toHex(chainId),
    chainName: config.get('MATIC_CHAIN_NAME'),
    nativeCurrency: { name: 'POL', symbol: 'POL', decimals: 18 },
    rpcUrls: [config.get('MATIC_RPC_URL')],
    blockExplorerUrls: [config.get('MATIC_EXPLORER_URL')]
  }
}

// MetaMask mobile wraps the 4902 in an internal error.
function isUnknownChain(error: unknown): boolean {
  const { code, data } = (error ?? {}) as { code?: number; data?: { originalError?: { code?: number } } }
  return code === 4902 || data?.originalError?.code === 4902
}

/**
 * Asks the wallet to move to `chainId`, adding this environment's Polygon first when the wallet doesn't know it.
 * Call it from a click: wallets refuse `wallet_*` requests they can't attribute to a user action. A refusal
 * (code 4001) propagates: the user answered, nothing moved.
 */
export async function switchWalletChain(session: Session, chainId: number): Promise<void> {
  try {
    await session.provider.request({
      method: 'wallet_switchEthereumChain',
      params: [{ chainId: toHex(chainId) }]
    })
  } catch (error) {
    const add = addChainParams(chainId)
    if (!isUnknownChain(error) || !add) throw error
    await session.provider.request({ method: 'wallet_addEthereumChain', params: [add] })
  }
}

// Wallets answer in hex, as EIP-1193 says, but some answer a number or a decimal string.
function toChainId(raw: unknown): number {
  if (typeof raw === 'number') return Number.isSafeInteger(raw) ? raw : 0
  if (typeof raw === 'string' && /^0x[0-9a-fA-F]+$/.test(raw)) {
    const parsed = Number(BigInt(raw))
    return Number.isSafeInteger(parsed) ? parsed : 0
  }
  if (typeof raw === 'string' && /^\d+$/.test(raw)) return Number(raw)
  return 0
}

/**
 * The chain the wallet is on right now, asked over `eth_chainId` so the answer is the wallet's and not one
 * read earlier. Throws on anything that is not a positive chain id (a wallet answering null would otherwise
 * read as chain 0).
 */
export async function readWalletChainId(session: Session): Promise<number> {
  const raw = await session.provider.request({ method: 'eth_chainId', params: [] })
  const parsed = toChainId(raw)
  if (parsed <= 0) throw new Error(`Wallet answered eth_chainId with an unusable value: ${String(raw)}`)
  return parsed
}

/**
 * Runs `onChange` whenever the wallet announces a chain change, and on an account change too, since another
 * account can sit on another chain without the wallet re-announcing it. Returns the unsubscribe; a provider
 * without events (Magic) subscribes to nothing.
 */
export function subscribeWalletChain(session: Session, onChange: () => void): () => void {
  const wallet = session.provider
  if (!wallet.on || !wallet.removeListener) return () => undefined
  wallet.on('chainChanged', onChange)
  wallet.on('accountsChanged', onChange)
  return () => {
    wallet.removeListener?.('chainChanged', onChange)
    wallet.removeListener?.('accountsChanged', onChange)
  }
}

/** Throws `WrongAccountError` unless the wallet's active account is the session's. */
export async function requireAccount(session: Session): Promise<void> {
  const accounts = (await session.provider.request({ method: 'eth_accounts', params: [] })) as string[]
  const active = (accounts[0] ?? '').toLowerCase()
  if (active !== session.address.toLowerCase()) throw new WrongAccountError(session.address, active)
}
