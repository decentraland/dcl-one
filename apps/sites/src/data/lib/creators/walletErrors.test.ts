import { describe, it, expect } from 'vitest'
import {
  WrongAccountError,
  WrongChainError,
  isExpectedWalletError,
  isWalletRejection,
  isWalletStateError
} from './walletErrors'

const caused = (message: string, cause: unknown) => Object.assign(new Error(message), { cause })

describe('isWalletRejection', () => {
  it('recognizes the EIP-1193 code, the ethers code and wallet wording', () => {
    expect(isWalletRejection({ code: 4001, message: 'x' })).toBe(true)
    expect(isWalletRejection(Object.assign(new Error('boom'), { code: 'ACTION_REJECTED' }))).toBe(true)
    expect(isWalletRejection(new Error('MetaMask Tx Signature: User denied transaction signature.'))).toBe(true)
  })

  it('recognizes a rejection wrapped by a chain library, through its causes', () => {
    expect(isWalletRejection(caused('Transaction failed', caused('Request failed', { code: 4001 })))).toBe(true)
    expect(isWalletRejection(caused('Transaction failed', new Error('nonce too low')))).toBe(false)
  })

  it('leaves other failures alone', () => {
    expect(isWalletRejection(new Error('insufficient funds'))).toBe(false)
    expect(isWalletRejection('nope')).toBe(false)
  })

  it('does not count a wallet on the wrong account or network as a dismissal', () => {
    expect(isWalletRejection(new WrongAccountError('0xme', '0xother'))).toBe(false)
    expect(isWalletRejection(new WrongChainError(137, 1))).toBe(false)
  })
})

describe('isWalletStateError', () => {
  it('recognizes a wallet on another account or another network', () => {
    expect(isWalletStateError(new WrongAccountError('0xme', '0xother'))).toBe(true)
    expect(isWalletStateError(new WrongChainError(137, 1))).toBe(true)
  })

  it('recognizes them wrapped by a flow error, through its causes', () => {
    expect(isWalletStateError(caused('Publish failed', new WrongAccountError('0xme', '0xother')))).toBe(true)
    expect(isWalletStateError(caused('Send failed', caused('Wrapped', new WrongChainError(137, 1))))).toBe(true)
  })

  it('leaves dismissals and other failures alone', () => {
    expect(isWalletStateError({ code: 4001, message: 'User rejected the request' })).toBe(false)
    expect(isWalletStateError(caused('Transaction failed', new Error('execution reverted')))).toBe(false)
    expect(isWalletStateError(undefined)).toBe(false)
  })
})

describe('isExpectedWalletError', () => {
  it('covers a dismissal and a wallet on the wrong account or network, never a real failure', () => {
    expect(isExpectedWalletError({ code: 4001 })).toBe(true)
    expect(isExpectedWalletError(caused('Approve failed', new WrongAccountError('0xme', '0xother')))).toBe(true)
    expect(isExpectedWalletError(new Error('execution reverted'))).toBe(false)
  })
})
