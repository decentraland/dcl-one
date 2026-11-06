import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { TimeoutError, settled } from '../driver/wait.ts'

describe('settled', () => {
  test('returns once two reads agree', async () => {
    let reads = 0
    const value = await settled(async () => Math.min(3, ++reads), { timeoutMs: 1000, intervalMs: 5, what: 'the value' })
    assert.equal(value, 3)
  })

  test('returns the last read when the reads never agree', async () => {
    let reads = 0
    const value = await settled(async () => ++reads, { timeoutMs: 40, intervalMs: 5, what: 'the value' })
    assert.ok(value > 1)
  })

  test('strict, throws a TimeoutError naming what never held still', async () => {
    let reads = 0
    await assert.rejects(settled(async () => ++reads, { timeoutMs: 40, intervalMs: 5, what: 'the button to hold still', strict: true }), (error: unknown) => {
      assert.ok(error instanceof TimeoutError)
      assert.match(error.message, /waiting for the button to hold still/)
      return true
    })
  })
})
