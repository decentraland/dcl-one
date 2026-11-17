import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { TimeoutError, until } from '../driver/wait.ts'

describe('until', () => {
  test('returns the first read that is done, without waiting out an interval first', async () => {
    let reads = 0
    const value = await until(async () => ++reads, (read) => read >= 3, { timeoutMs: 1000, intervalMs: 5, what: 'the third read' })
    assert.equal(value, 3)
    assert.equal(await until(() => 'now', () => true, { timeoutMs: 1000, intervalMs: 60_000, what: 'a ready value' }), 'now')
  })

  test('retries a read that throws, and is done once one answers', async () => {
    let reads = 0
    const value = await until(
      async () => {
        if (++reads < 3) throw new Error('not yet')
        return 'answered'
      },
      () => true,
      { timeoutMs: 1000, intervalMs: 5, what: 'an answer' }
    )
    assert.equal(value, 'answered')
  })

  test('is a TimeoutError naming what it waited for once the ceiling passes', async () => {
    await assert.rejects(until(async () => 'busy', (read) => read === 'ready', { timeoutMs: 30, intervalMs: 5, what: 'the grid to be ready' }), (error: unknown) => {
      assert.ok(error instanceof TimeoutError)
      assert.match(error.message, /waiting for the grid to be ready/)
      return true
    })
  })

  test("rethrows the read's last error when the ceiling passes on one", async () => {
    await assert.rejects(
      until(
        async () => {
          throw new Error('no HUD page')
        },
        () => true,
        { timeoutMs: 30, intervalMs: 5, what: 'the HUD' }
      ),
      /no HUD page/
    )
  })
})
