import { describe, it, expect } from 'vitest'
import { loadPendingPublish, loadSentRoot, savePendingPublish, saveSentRoot } from './pendingLinkedPayment'

function memoryStore() {
  const values = new Map<string, string>()
  return {
    values,
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => void values.set(key, value),
    removeItem: (key: string) => void values.delete(key)
  }
}

const scope = { chainId: 80002, address: '0xAbC', collectionId: 'c1' }
const payment = { txHash: '0xabc', registration: true }

describe('pending linked publish', () => {
  it('remembers what is left per chain, wallet and collection, and forgets it once settled', () => {
    const store = memoryStore()
    savePendingPublish(scope, { payment }, store)

    expect(loadPendingPublish({ ...scope, address: '0xabc' }, store)).toEqual({ payment })
    expect(loadPendingPublish({ ...scope, address: '0xdef' }, store)).toEqual({})
    expect(loadPendingPublish({ ...scope, chainId: 137 }, store)).toEqual({})
    expect(loadPendingPublish({ ...scope, collectionId: 'c2' }, store)).toEqual({})

    savePendingPublish(scope, {}, store)
    expect(store.values.size).toBe(0)
  })

  it('keeps the sender recorded with a payment, and drops a malformed one', () => {
    const store = memoryStore()
    const sender = { from: '0x' + 'ab'.repeat(20), nonce: 4 }
    savePendingPublish(scope, { payment: { ...payment, sender } }, store)
    expect(loadPendingPublish(scope, store)).toEqual({ payment: { ...payment, sender } })
    savePendingPublish(scope, { payment: { ...payment, sender: { from: 'nope', nonce: 4 } } }, store)
    expect(loadPendingPublish(scope, store)).toEqual({ payment })
  })

  it('ignores anything stored that is not a payment', () => {
    const store = memoryStore()
    savePendingPublish(scope, { payment: { ...payment, txHash: 'nope' } }, store)
    expect(loadPendingPublish(scope, store)).toEqual({})
    store.setItem([...store.values.keys()][0] ?? 'x', 'not json')
    expect(loadPendingPublish(scope, store)).toEqual({})
  })

  it('survives blocked storage', () => {
    const blocked = {
      getItem: () => {
        throw new Error('blocked')
      },
      setItem: () => {
        throw new Error('blocked')
      },
      removeItem: () => {
        throw new Error('blocked')
      }
    }
    expect(loadPendingPublish(scope, blocked)).toEqual({})
    expect(() => savePendingPublish(scope, { payment }, blocked)).not.toThrow()
  })
})

describe('sent root', () => {
  it('remembers a root transaction per chain and third party until cleared, ignoring malformed records', () => {
    const store = memoryStore()
    const scope = { chainId: 80002, thirdPartyId: 'urn:tp' }
    const sent = { hash: '0xabc', merkleRoot: '0xdef' }
    saveSentRoot(scope, sent, store)
    expect(loadSentRoot(scope, store)).toEqual(sent)
    expect(loadSentRoot({ ...scope, thirdPartyId: 'urn:other' }, store)).toBeUndefined()
    saveSentRoot(scope, undefined, store)
    expect(loadSentRoot(scope, store)).toBeUndefined()
    store.setItem('linked-approval.root.80002.urn:tp', '{"hash":"nope"}')
    expect(loadSentRoot(scope, store)).toBeUndefined()
  })
})
