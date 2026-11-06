import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { FakeConsole } from '../driver/console.ts'
import { Explorer } from '../driver/explorer.ts'
import { CONSOLE_TAIL_LINES, REST_ESCAPES, closeToRest, failureEvidence, specName, withinBudget } from '../driver/harness.ts'
import { TimeoutError } from '../driver/wait.ts'

const button = (name: string, actionable: boolean, checked = false) =>
  JSON.stringify({ found: true, role: 'button', name, text: '', disabled: false, visible: true, actionable, checked, x: 0, y: 0, width: 10, height: 10 })

/** A HUD that needs `layers` Escapes before the rail takes a click; `typing` keeps the chat input
 *  focused; `panel` names a rail panel open until the first Escape. */
function hud(layers: number, options: { typing?: boolean; empty?: boolean; panel?: string } = {}): FakeConsole {
  let escapes = 0
  return new FakeConsole({
    press_key: () => {
      escapes += 1
      return 'pressed Escape'
    },
    hud_read: (line) => {
      if (options.empty) return '{"found":false}'
      if (line.includes('Write a message')) return options.typing ? button('Write a message', true) : '{"found":false}'
      if (line.includes('Sidebar settings')) return button('Sidebar settings', escapes >= layers)
      const name = /"name":"([^"]+)"/.exec(line)?.[1] ?? ''
      return button(name, true, name === options.panel && escapes === 0)
    },
    hud_list: () => (options.empty ? '[]' : '[{"role":"button","name":"Close"}]')
  })
}

describe('closeToRest', () => {
  test('presses nothing when the HUD is at rest', async () => {
    const fake = hud(0)
    await closeToRest(new Explorer(fake), 300)
    assert.deepEqual(fake.sent('press_key'), [])
  })

  test('presses Escape for a panel open beside the rail', async () => {
    const fake = hud(0, { panel: 'Notifications' })
    await closeToRest(new Explorer(fake), 300)
    assert.deepEqual(fake.sent('press_key'), ['/press_key Escape'])
  })

  test('presses Escape until the rail takes a click: a modal over a page needs two', async () => {
    const fake = hud(2)
    await closeToRest(new Explorer(fake), 300)
    assert.equal(fake.sent('press_key').length, 2)
  })

  test('a focused chat input is not at rest', async () => {
    const fake = hud(0, { typing: true })
    await closeToRest(new Explorer(fake), 200)
    assert.equal(fake.sent('press_key').length, REST_ESCAPES)
  })

  test('gives up after its presses, leaving the scenario to fail on what it finds', async () => {
    const fake = hud(REST_ESCAPES + 3)
    await closeToRest(new Explorer(fake), 200)
    assert.equal(fake.sent('press_key').length, REST_ESCAPES)
  })

  test('a client without a HUD gets no press', async () => {
    const fake = hud(1, { empty: true })
    await closeToRest(new Explorer(fake), 300)
    assert.equal(fake.sent('press_key').length, 0)
  })
})

describe('failureEvidence', () => {
  test("keeps the page's screenshot and the last console lines", async () => {
    const lines = Array.from({ length: CONSOLE_TAIL_LINES + 5 }, (_, i) => `line ${i}`)
    const console_ = Object.assign(new FakeConsole(), { evidence: async () => ({ console: lines, screenshot: Buffer.from([1, 2, 3]) }) })
    const files = await failureEvidence(console_)
    assert.deepEqual(files['page.jpg'], Buffer.from([1, 2, 3]))
    const kept = String(files['console.txt']).trimEnd().split('\n')
    assert.equal(kept.length, CONSOLE_TAIL_LINES)
    assert.equal(kept[0], 'line 5')
  })

  test('gives up on a page that does not answer', async () => {
    const console_ = Object.assign(new FakeConsole(), { evidence: () => new Promise<never>(() => {}) })
    await assert.rejects(failureEvidence(console_, 30), TimeoutError)
  })

  test('is nothing for a transport that cannot tell', async () => {
    assert.deepEqual(await failureEvidence(new FakeConsole()), {})
  })
})

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
