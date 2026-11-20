// @vitest-environment node
import { QueryClient } from '@tanstack/react-query'
import { describe, expect, it } from 'vitest'
import { privateDomain, privateKey } from './queryKeys'

describe('private query keys', () => {
  it('scopes an entry to the account whatever the casing of its address', () => {
    expect(privateKey('0xAbC', 'collection', 'c1')).toEqual(['private', '0xabc', 'collection', 'c1'])
  })

  it('leaves no entry of an account behind once its prefix is removed', () => {
    const client = new QueryClient()
    client.setQueryData(privateKey('0xA', 'collections', 1), [])
    client.setQueryData(privateKey('0xB', 'collections', 1), [])
    client.removeQueries({ queryKey: ['private', '0xa'] })
    expect(client.getQueryData(privateKey('0xA', 'collections', 1))).toBeUndefined()
    expect(client.getQueryData(privateKey('0xB', 'collections', 1))).toEqual([])
  })

  it('invalidates one domain, narrowed to the entries a write touched', async () => {
    const client = new QueryClient()
    const touched = privateKey('0xA', 'collection', 'c1')
    const other = privateKey('0xA', 'collection', 'c2')
    const list = privateKey('0xA', 'collections', 1)
    for (const key of [touched, other, list]) client.setQueryData(key, {})
    await client.invalidateQueries(privateDomain('collection', query => query.queryKey.includes('c1')))
    expect(client.getQueryState(touched)?.isInvalidated).toBe(true)
    expect(client.getQueryState(other)?.isInvalidated).toBe(false)
    expect(client.getQueryState(list)?.isInvalidated).toBe(false)
  })
})
