import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { ConsoleError, FakeConsole } from '../driver/console.ts'
import { Explorer } from '../driver/explorer.ts'
import { CONSOLE_TAIL_LINES, REST_ESCAPES, closeToRest, failureEvidence, specName, withinBudget } from '../driver/harness.ts'
import { TimeoutError } from '../driver/wait.ts'

const button = (name: string, actionable: boolean, checked = false, reason?: string) =>
  JSON.stringify({ found: true, role: 'button', name, text: '', disabled: false, visible: true, actionable, checked, reason, x: 0, y: 0, width: 10, height: 10 })

const worldState = (keyboardClaimed: boolean | null) => JSON.stringify({ inWorld: true, arrivals: 1, keyboardClaimed })

/** A HUD with `layers` modals over the rail, one closed per Escape, each listed as a dialog;
 *  `typing` keeps the chat input focused, or `chat` until the first Escape; `panel` names a rail
 *  panel, and `popover` the rail's settings popover, open until the first Escape; `claimed` has a
 *  field hold the keyboard until the first Escape (`always`: through every press), and `relay`
 *  more reads of `/world_state` after it; `sliding` has the rail read still moving that many
 *  times. */
function hud(
  layers: number,
  options: {
    typing?: boolean
    chat?: boolean
    empty?: boolean
    panel?: string
    popover?: boolean
    claimed?: boolean | 'always'
    relay?: number
    sliding?: number
  } = {}
): FakeConsole {
  let escapes = 0
  let releasedReads = 0
  let railReads = 0
  return new FakeConsole({
    press_key: () => {
      escapes += 1
      return 'pressed Escape'
    },
    hud_read: (line) => {
      if (options.empty) return '{"found":false}'
      if (line.includes('Write a message')) return options.typing || (options.chat && escapes === 0) ? button('Write a message', true) : '{"found":false}'
      if (line.includes('"role":"dialog"')) return options.popover && escapes === 0 ? button('Sidebar settings', true).replace('"button"', '"dialog"') : '{"found":false}'
      if (line.includes('Sidebar settings')) {
        if (escapes < layers) return button('Sidebar settings', false, false, 'covered')
        return ++railReads <= (options.sliding ?? 0) ? button('Sidebar settings', false, false, 'still moving') : button('Sidebar settings', true)
      }
      const name = /"name":"([^"]+)"/.exec(line)?.[1] ?? ''
      return button(name, true, name === options.panel && escapes === 0)
    },
    hud_list: () => {
      if (options.empty) return '[]'
      const modals = Array.from({ length: Math.max(0, layers - escapes) }, (_, i) => ({ role: 'dialog', name: `Modal ${i + 1}` }))
      return JSON.stringify([{ role: 'button', name: 'Close' }, ...modals])
    },
    world_state: () => {
      if (options.claimed === 'always') return worldState(true)
      if (options.claimed !== true) return worldState(false)
      if (escapes === 0) return worldState(true)
      return worldState(++releasedReads <= (options.relay ?? 0))
    }
  })
}

/** A page with a dropdown open over it: the first Escape closes the dropdown, the second the
 *  page, after which the rail slides back in, `moving` reads long. */
function pageWithDropdown(moving: number): FakeConsole {
  let escapes = 0
  let railReads = 0
  return new FakeConsole({
    press_key: () => {
      escapes += 1
      return 'pressed Escape'
    },
    hud_read: (line) => {
      if (line.includes('"role":"dialog"') || line.includes('Write a message')) return '{"found":false}'
      if (line.includes('Sidebar settings')) {
        if (escapes < 2) return '{"found":false}'
        railReads += 1
        return railReads <= moving ? button('Sidebar settings', false, false, 'still moving') : button('Sidebar settings', true)
      }
      const name = /"name":"([^"]+)"/.exec(line)?.[1] ?? ''
      return button(name, true)
    },
    hud_list: () => {
      if (escapes >= 2) return '[{"role":"button","name":"Sidebar settings"}]'
      const page = [{ role: 'heading', name: 'Backpack' }]
      return JSON.stringify(escapes === 0 ? [...page, { role: 'option', name: 'Newest' }] : page)
    },
    world_state: () => worldState(false)
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

  test("presses Escape for the rail's settings popover, which its button shows as expanded, not pressed", async () => {
    const fake = hud(0, { popover: true })
    await closeToRest(new Explorer(fake), 300)
    assert.deepEqual(fake.sent('press_key'), ['/press_key Escape'])
  })

  test('presses Escape until the rail takes a click: a modal over a page needs two', async () => {
    const fake = hud(2)
    await closeToRest(new Explorer(fake), 300)
    assert.equal(fake.sent('press_key').length, 2)
  })

  test('a field holding the keyboard is not at rest: Escape lets it go', async () => {
    const fake = hud(0, { claimed: true })
    await closeToRest(new Explorer(fake), 300)
    assert.deepEqual(fake.sent('press_key'), ['/press_key Escape'])
  })

  test('after leaving the chat input, waits for the engine to hear of it before another press', { timeout: 10_000 }, async () => {
    // the HUD shows the input left at once; the engine holds the keys for three more reads
    const fake = hud(0, { chat: true, claimed: true, relay: 3 })
    await closeToRest(new Explorer(fake), 600_000)
    assert.deepEqual(fake.sent('press_key'), ['/press_key Escape'])
  })

  test('leaves a field that keeps the keys through every press to the scenario, without failing first', async () => {
    const fake = hud(0, { claimed: 'always' })
    await closeToRest(new Explorer(fake), 50)
    assert.equal(fake.sent('press_key').length, REST_ESCAPES)
  })

  test('a rail still sliding in with nothing open is at rest once it holds still, with no press', { timeout: 10_000 }, async () => {
    const fake = hud(0, { sliding: 3 })
    await closeToRest(new Explorer(fake), 600_000)
    assert.deepEqual(fake.sent('press_key'), [])
  })

  test('follows a press as soon as the HUD shows it closed a layer, not after a window', { timeout: 10_000 }, async () => {
    const fake = pageWithDropdown(0)
    await closeToRest(new Explorer(fake), 600_000)
    assert.equal(fake.sent('press_key').length, 2)
  })

  test('waits for the rail to come back in before calling the HUD at rest, without another press', { timeout: 10_000 }, async () => {
    const fake = pageWithDropdown(4)
    await closeToRest(new Explorer(fake), 600_000)
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

  test("a client without the HUD's commands (the standalone without its bundle) gets no press", async () => {
    const fake = new FakeConsole({ press_key: 'pressed Escape' })
    await closeToRest(new Explorer(fake), 300)
    assert.equal(fake.sent('press_key').length, 0)
  })

  test("any other error is the scenario's, not a HUD at rest", async () => {
    const fake = new FakeConsole({
      press_key: 'pressed Escape',
      hud_read: (line) => {
        throw new ConsoleError(line, 'the test runner did not answer a trusted click within 10000 ms')
      }
    })
    await assert.rejects(closeToRest(new Explorer(fake), 300), /did not answer/)
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

  test("scrubs the credentials in the page's console lines it keeps", async () => {
    const console_ = Object.assign(new FakeConsole(), {
      evidence: async () => ({ console: ['ok', 'connecting to livekit:wss://rooms.example.test/r?access_token=tok3n&x=1'], screenshot: undefined })
    })
    const files = await failureEvidence(console_)
    assert.equal(files['console.txt'], 'ok\nconnecting to livekit:wss://rooms.example.test/r?access_token=REDACTED&x=1\n')
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
