// @vitest-environment jsdom
import './test/setup'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { TransactionExecutionError, UnknownRpcError } from 'viem'
import { captureError, redact, rpcFactsFrom, toReportable } from './monitoring'

const SIGNATURE = `0x${'a'.repeat(130)}`
const HEX32 = `0x${'b'.repeat(64)}`

beforeEach(() => {
  vi.spyOn(console, 'error').mockImplementation(() => undefined)
  vi.spyOn(console, 'warn').mockImplementation(() => undefined)
})

afterEach(() => {
  vi.restoreAllMocks()
})

const logged = () => vi.mocked(console.error).mock.calls.map(args => args.join(' ')).join('\n')

describe('captureError', () => {
  it('logs the failure with the flow that produced it and the plain facts of its context', () => {
    captureError(new TypeError('upload failed'), { flow: 'add-items', http_status: 500, item: { id: 'x' } })

    expect(console.error).toHaveBeenCalledTimes(1)
    expect(logged()).toContain('[creators] error in add-items [http_status=500]: TypeError: upload failed')
  })

  it('never logs a signature or a sensitive context value', () => {
    captureError(new Error(`bad signature ${SIGNATURE}`), { flow: 'publish', auth_token: 'abc', note: HEX32 })

    expect(logged()).not.toContain(SIGNATURE)
    expect(logged()).not.toContain('abc')
    expect(logged()).not.toContain(HEX32)
  })

  it('reports a wallet failure that is not an Error with a readable message and its rpc facts', () => {
    captureError({ code: -32603, message: 'Failed to fetch', data: { httpStatus: 401 } }, { flow: 'send-items' })

    expect(logged()).toContain('Failed to fetch (code -32603)')
    expect(logged()).toContain('rpc_code=-32603 http_status=401')
  })

  it('keeps the facts ahead of a long message, within what the site reports', () => {
    captureError(new Error(`rpc failed ${'x'.repeat(2_000)}`), { flow: 'publish', http_status: 502 })

    expect(logged().slice(0, 500)).toContain('[creators] error in publish [http_status=502]: Error: rpc failed')
  })

  it('describes a thrown value that is not an Error by its plain facts, never a sensitive one', () => {
    captureError({ code: 4001, token: 'abc', mnemonic: 'seed words' }, { flow: 'sell-item' })

    expect(logged()).toContain('Object [code=4001]')
    expect(logged()).not.toContain('abc')
    expect(logged()).not.toContain('seed words')
    expect(logged()).not.toContain('[object Object]')
  })

  it('lets the caller override a fact read off the thrown value', () => {
    captureError({ code: 1 }, { rpc_code: 'known-better' })

    expect(logged()).toContain('rpc_code=known-better')
  })

  it('does not throw back into the caller when logging itself fails', () => {
    vi.mocked(console.error).mockImplementation(() => {
      throw new Error('console is gone')
    })

    expect(() => captureError(new Error('boom'), { flow: 'sell-item' })).not.toThrow()
  })
})

describe('toReportable', () => {
  it('keeps a real Error as it was thrown', () => {
    const error = new Error('as thrown')
    expect(toReportable(error)).toBe(error)
  })

  it('keeps the original stack so failures still group by where they happened', () => {
    const reportable = toReportable({ message: 'rejected', stack: 'at sellItem (sales.ts:1:1)' })
    expect((reportable as Error).stack).toBe('at sellItem (sales.ts:1:1)')
  })
})

describe('rpcFactsFrom', () => {
  it('finds nothing to report in a value that carries no codes', () => {
    expect(rpcFactsFrom('a string')).toEqual({})
    expect(rpcFactsFrom(null)).toEqual({})
  })

  it("reads the wallet's own code and HTTP status from under a chain library's wrappers", () => {
    const wallet = Object.assign(new Error('Unauthorized'), { code: -32099, data: { httpStatus: 401 } })
    const wrapped = new TransactionExecutionError(new UnknownRpcError(wallet), { account: null })
    expect(rpcFactsFrom(wrapped)).toEqual({ rpc_code: -32099, http_status: 401 })
    expect(rpcFactsFrom({ code: 4001, message: 'User rejected' })).toEqual({ rpc_code: 4001 })
  })
})

describe('redact', () => {
  it('leaves ordinary copy alone', () => {
    expect(redact('the collection is locked')).toBe('the collection is locked')
  })

  it('takes out a bearer token whole, payload included', () => {
    const jwt = 'eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIweGNyZWF0b3IifQ.c2lnbmF0dXJl'

    expect(redact(`request failed with ${jwt}`)).toBe('request failed with <jwt>')
  })
})
