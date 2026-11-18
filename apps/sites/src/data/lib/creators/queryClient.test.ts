import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { HttpError } from '~/lib/http'
import { setErrorForwarder } from '~/lib/monitoring'
import { toDiscountError } from '~/lib/coupons'
import { toSellItemError } from '~/lib/sales'
import { WrongAccountError, WrongChainError } from '~/lib/walletErrors'
import { createQueryClient } from './queryClient'

const reported = vi.fn()

beforeEach(() => {
  vi.spyOn(console, 'error').mockImplementation(() => undefined)
  setErrorForwarder(reported)
})

afterEach(() => {
  setErrorForwarder(null)
  reported.mockReset()
  vi.restoreAllMocks()
})

describe('createQueryClient', () => {
  it('reports a query that fails by name, keeping the ids and search text in its key out of the report', async () => {
    const client = createQueryClient()

    await client
      .fetchQuery({
        queryKey: ['collections', '0xabc', 1, 'my secret drop'],
        queryFn: () => Promise.reject(new Error('503')),
        retry: false
      })
      .catch(() => undefined)

    expect(reported).toHaveBeenCalledWith(
      expect.objectContaining({ message: '503' }),
      expect.objectContaining({ flow: 'query', query_key: 'collections' })
    )
    expect(JSON.stringify(reported.mock.calls[0][1])).not.toContain('my secret drop')
  })

  it('does not report a query whose signer call the creator rejected', async () => {
    const client = createQueryClient()

    await client
      .fetchQuery({
        queryKey: ['mana-allowance', '0xabc'],
        queryFn: () => Promise.reject(Object.assign(new Error('User rejected the request'), { code: 4001 })),
        retry: false
      })
      .catch(() => undefined)

    expect(reported).not.toHaveBeenCalled()
  })

  it('tags a failed response with its HTTP status', async () => {
    const client = createQueryClient()

    await client
      .fetchQuery({
        queryKey: ['hot-scenes'],
        queryFn: () => Promise.reject(new HttpError('hot scenes request failed', 503)),
        retry: false,
        meta: { reportOnlyHttpErrors: true }
      })
      .catch(() => undefined)

    expect(reported).toHaveBeenCalledTimes(1)
    expect(reported).toHaveBeenCalledWith(
      expect.any(HttpError),
      expect.objectContaining({ flow: 'query', query_key: 'hot-scenes', http_status: 503 })
    )
  })

  it('stays quiet about the network failures of a query that opts out of them', async () => {
    const client = createQueryClient()
    const failures = [new TypeError('Failed to fetch'), new DOMException('timed out', 'TimeoutError')]

    for (const failure of failures) {
      await client
        .fetchQuery({
          queryKey: ['latest-blog-posts', failure.name],
          queryFn: () => Promise.reject(failure),
          retry: false,
          meta: { reportOnlyHttpErrors: true }
        })
        .catch(() => undefined)
    }

    expect(reported).not.toHaveBeenCalled()
  })

  it('still reports the network failures of every other query', async () => {
    const client = createQueryClient()

    await client
      .fetchQuery({
        queryKey: ['collections'],
        queryFn: () => Promise.reject(new TypeError('Failed to fetch')),
        retry: false
      })
      .catch(() => undefined)

    expect(reported).toHaveBeenCalledTimes(1)
  })

  it('reports a mutation that fails', async () => {
    const client = createQueryClient()
    const mutation = client.getMutationCache().build(client, {
      mutationKey: ['save-collection'],
      mutationFn: () => Promise.reject(new Error('reverted'))
    })

    await mutation.execute(undefined).catch(() => undefined)

    expect(reported).toHaveBeenCalledWith(
      expect.objectContaining({ message: 'reverted' }),
      expect.objectContaining({ flow: 'mutation', mutation_key: 'save-collection' })
    )
  })

  it('reports a mutation without a key with no name rather than a made-up one', async () => {
    const client = createQueryClient()
    const mutation = client.getMutationCache().build(client, {
      mutationFn: () => Promise.reject(new Error('reverted'))
    })

    await mutation.execute(undefined).catch(() => undefined)

    expect(reported).toHaveBeenCalledWith(
      expect.objectContaining({ message: 'reverted' }),
      expect.objectContaining({ flow: 'mutation', mutation_key: undefined })
    )
  })

  it('does not report the creator dismissing the wallet prompt', async () => {
    const client = createQueryClient()
    const mutation = client.getMutationCache().build(client, {
      mutationFn: () => Promise.reject(Object.assign(new Error('User rejected the request'), { code: 4001 }))
    })

    await mutation.execute(undefined).catch(() => undefined)

    expect(reported).not.toHaveBeenCalled()
  })

  it('does not report a wallet sitting on another account or network, even wrapped by the flow', async () => {
    const client = createQueryClient()
    const failures = [
      new WrongAccountError('0xme', '0xother'),
      toSellItemError(new WrongChainError(137, 1)),
      toDiscountError(new WrongAccountError('0xme', '0xother'))
    ]

    for (const failure of failures) {
      const mutation = client.getMutationCache().build(client, { mutationFn: () => Promise.reject(failure) })
      await mutation.execute(undefined).catch(() => undefined)
    }

    expect(reported).not.toHaveBeenCalled()
  })
})
