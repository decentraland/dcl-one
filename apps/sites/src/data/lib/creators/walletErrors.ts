const WALLET_REJECTION_CODE = 4001
const WALLET_REJECTION_PATTERN = /user rejected|user denied|rejected the request/i
const MAX_CAUSES = 10

/**
 * The error and the causes under it, outermost first. Chain libraries wrap the wallet's own error (viem's
 * TransactionExecutionError over UserRejectedRequestError over the EIP-1193 object), so its code sits deeper.
 */
export function errorChain(error: unknown): unknown[] {
  const chain: unknown[] = []
  let current = error
  while (current !== undefined && current !== null && chain.length < MAX_CAUSES && !chain.includes(current)) {
    chain.push(current)
    current = typeof current === 'object' ? (current as { cause?: unknown }).cause : undefined
  }
  return chain
}

/** The creator dismissed the wallet prompt (EIP-1193 4001, ethers ACTION_REJECTED, or a wallet's own wording). */
export function isWalletRejection(error: unknown): boolean {
  return errorChain(error).some(link => {
    const message = link instanceof Error ? link.message : String(link)
    const code = (link as { code?: unknown }).code
    return code === WALLET_REJECTION_CODE || code === 'ACTION_REJECTED' || WALLET_REJECTION_PATTERN.test(message)
  })
}

/** The wallet is still on another chain after being asked to switch: nothing was sent. */
export class WrongChainError extends Error {
  constructor(
    public expected: number,
    public actual: number
  ) {
    super(`The wallet is on chain ${actual}, not ${expected}`)
    this.name = 'WrongChainError'
  }
}

/** The wallet's active account is not the session's: sending would act as somebody else. Nothing was sent. */
export class WrongAccountError extends Error {
  constructor(
    public expected: string,
    public actual: string
  ) {
    super(`The wallet's active account is ${actual}, not ${expected}`)
    this.name = 'WrongAccountError'
  }
}

/** The wallet sits on another account or network than the session's: the creator can switch it back and retry. */
export function isWalletStateError(error: unknown): boolean {
  return errorChain(error).some(link => link instanceof WrongAccountError || link instanceof WrongChainError)
}

/** A wallet outcome the creator chose or can fix themselves: tracked, never reported as a failure. */
export function isExpectedWalletError(error: unknown): boolean {
  return isWalletRejection(error) || isWalletStateError(error)
}
