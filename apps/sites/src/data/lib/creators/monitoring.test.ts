// @vitest-environment jsdom
import '@/data/lib/creators/test/setup'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { TransactionExecutionError, UnknownRpcError } from 'viem'
import { captureError, clientErrorForwarder, redact, rpcFactsFrom, setErrorForwarder, toReportable } from './monitoring'

const SIGNATURE = `0x${'a'.repeat(130)}`
const HEX32 = `0x${'b'.repeat(64)}`

beforeEach(() => {
  vi.spyOn(console, 'error').mockImplementation(() => undefined)
  vi.spyOn(console, 'warn').mockImplementation(() => undefined)
})

afterEach(() => {
  setErrorForwarder(null)
  vi.restoreAllMocks()
})

describe('captureError', () => {
  it('always logs the failure and hands it to the reporter with the flow that produced it', () => {
    const reported = vi.fn()
    setErrorForwarder(reported)

    captureError(new Error('publish reverted'), { flow: 'publish-collection', collectionId: 'col-1' })

    expect(console.error).toHaveBeenCalled()
    expect(reported).toHaveBeenCalledWith(
      expect.objectContaining({ message: 'publish reverted' }),
      expect.objectContaining({ flow: 'publish-collection', collectionId: 'col-1' })
    )
  })

  it('does not throw back into the caller when reporting itself fails', () => {
    setErrorForwarder(() => {
      throw new Error('sentry is down')
    })

    expect(() => captureError(new Error('boom'), { flow: 'sell-item' })).not.toThrow()
    // A broken forwarder loses every report, so it has to be visible somewhere.
    expect(console.warn).toHaveBeenCalled()
  })

  it('reports a wallet failure that is not an Error with a readable name and its rpc facts', () => {
    const reported = vi.fn()
    setErrorForwarder(reported)

    captureError({ code: -32603, message: 'Failed to fetch', data: { httpStatus: 401 } }, { flow: 'send-items' })

    const [error, context] = reported.mock.calls[0]
    expect((error as Error).message).toBe('Failed to fetch (code -32603)')
    expect(context).toMatchObject({ rpc_code: -32603, http_status: 401, flow: 'send-items' })
  })

  it('lets the caller override a fact read off the thrown value', () => {
    const reported = vi.fn()
    setErrorForwarder(reported)

    captureError({ code: 1 }, { rpc_code: 'known-better' })

    expect(reported.mock.calls[0][1]).toMatchObject({ rpc_code: 'known-better' })
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

describe('clientErrorForwarder', () => {
  const posted = () => vi.mocked(fetch).mock.calls.map(([, init]) => JSON.parse(String(init?.body)))

  beforeEach(() => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response()))
  })

  afterEach(() => vi.unstubAllGlobals())

  it('reports the error with the flow that produced it and the plain facts of its context', () => {
    clientErrorForwarder(new TypeError('upload failed'), { flow: 'add-items', http_status: 500, item: { id: 'x' } })

    expect(fetch).toHaveBeenCalledWith('/internal/client-error', expect.objectContaining({ method: 'POST' }))
    expect(posted()[0]).toMatchObject({
      name: 'TypeError',
      message: 'upload failed [http_status=500]',
      source: 'creators:add-items'
    })
  })

  it('never sends a signature or a sensitive context value', () => {
    clientErrorForwarder(new Error(`bad signature ${SIGNATURE}`), { flow: 'publish', auth_token: 'abc', note: HEX32 })

    const report = JSON.stringify(posted()[0])
    expect(report).not.toContain(SIGNATURE)
    expect(report).not.toContain('abc')
    expect(report).not.toContain(HEX32)
  })

  it('reports the same error once within a minute', () => {
    clientErrorForwarder(new Error('same failure twice'), { flow: 'save-item' })
    clientErrorForwarder(new Error('same failure twice'), { flow: 'save-item' })

    expect(fetch).toHaveBeenCalledTimes(1)
  })
})
