import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { TransactionExecutionError, UnknownRpcError } from 'viem'
import {
  captureError,
  isLocalhost,
  redact,
  rpcFactsFrom,
  scrubEvent,
  setErrorForwarder,
  tagsFrom,
  toReportable
} from './monitoring'

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

describe('scrubEvent', () => {
  it('never lets a signature, an identity key or a token-shaped field reach the reporter', () => {
    const event = scrubEvent({
      message: `signed with ${SIGNATURE}`,
      exception: { values: [{ value: `key ${HEX32}` }] },
      breadcrumbs: [{ message: `secret-value` }],
      request: { url: `https://x.test/?sig=${SIGNATURE}`, cookies: { session: 'x' }, headers: { auth: 'x' } },
      tags: { authorization: 'bearer x', flow: 'publish-collection' },
      extra: { identity: 'private key', collectionId: 'col-1' }
    })

    expect(event.message).toBe('signed with <signature>')
    expect(event.exception?.values?.[0].value).toBe('key <hex32>')
    expect(event.breadcrumbs?.[0].message).toBe('<secret>')
    expect(event.request?.url).not.toContain(SIGNATURE)
    expect(event.request?.cookies).toBeUndefined()
    expect(event.request?.headers).toBeUndefined()
    expect(event.tags).toEqual({ flow: 'publish-collection' })
    expect(event.extra).toEqual({ collectionId: 'col-1' })
  })

  it('scrubs the urls a request carries in its own data, not only the ones in a message', () => {
    const event = scrubEvent({
      breadcrumbs: [{ category: 'fetch', data: { url: `https://x.test/items?sig=${SIGNATURE}`, token: 'abc' } }],
      spans: [
        {
          span_id: 'span-1',
          trace_id: 'trace-1',
          start_timestamp: 0,
          status: 'ok',
          description: `GET ${HEX32}`,
          data: { 'http.url': `https://x.test/?k=${HEX32}` }
        }
      ]
    })

    expect(event.breadcrumbs?.[0].data?.url).toBe('https://x.test/items?sig=<signature>')
    expect(event.breadcrumbs?.[0].data?.token).toBeUndefined()
    expect(event.spans?.[0].description).toBe('GET <hex32>')
    expect(event.spans?.[0].data?.['http.url']).toBe('https://x.test/?k=<hex32>')
  })
})

describe('tagsFrom', () => {
  it('promotes only the fields worth searching by, as strings', () => {
    expect(tagsFrom({ flow: 'publish-collection', step: 'pay', rpc_code: -32603, collectionId: 'col-1' })).toEqual({
      flow: 'publish-collection',
      step: 'pay',
      rpc_code: '-32603'
    })
  })

  it('drops values a tag cannot be made of', () => {
    expect(tagsFrom({ flow: '', step: { nested: true }, http_status: 401 })).toEqual({ http_status: '401' })
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

describe('isLocalhost', () => {
  it('recognises the hosts a creator runs the app on locally', () => {
    expect(isLocalhost('localhost')).toBe(true)
    expect(isLocalhost('127.0.0.1')).toBe(true)
    expect(isLocalhost('my-mac.local')).toBe(true)
    expect(isLocalhost('decentraland.org')).toBe(false)
  })
})
