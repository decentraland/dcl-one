import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { specName, withinBudget } from '../driver/harness.ts'
import { TimeoutError } from '../driver/wait.ts'

describe('withinBudget', () => {
  test('resolves with the work that finishes in time', async () => {
    assert.equal(await withinBudget(Promise.resolve(7), 1000, 'the work'), 7)
  })

  test('is a TimeoutError naming the work once the budget is spent', async () => {
    const never = new Promise<void>(() => {})
    await assert.rejects(withinBudget(never, 30, 'the scenario "x" to finish'), (error: unknown) => {
      assert.ok(error instanceof TimeoutError)
      assert.equal(error.message, 'timed out after 30 ms waiting for the scenario "x" to finish')
      return true
    })
  })

  test("rejects with the work's own error", async () => {
    await assert.rejects(withinBudget(Promise.reject(new Error('boom')), 1000, 'the work'), /boom/)
  })
})

describe('specName', () => {
  test('is the spec file without its suffix', () => {
    assert.equal(specName('file:///suite/specs/minimap.test.ts'), 'minimap')
  })
})
