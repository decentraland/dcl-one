import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  PAGE_COMMANDS,
  TRUSTED_INPUT_TIMEOUT_MS,
  answerHudRequest,
  answerHudWait,
  clickAt,
  clickControl,
  describeControl,
  fillControl,
  findControls,
  installHudAutomation,
  keyDefinition,
  keyFromCode,
  listControls,
  parseSelector,
  pressKey,
  trustedInputAvailable,
  waitForControl
} from './hudAutomation'
import { installCefNativeBridge } from './cefNativeBridge'

// jsdom lays nothing out: give elements boxes by hand, and say what is under a point.
const boxes = new WeakMap<Element, DOMRect>()
let atPoint: (x: number, y: number) => Element | null = () => null

function place(el: Element, x: number, y: number, width = 100, height = 30): Element {
  boxes.set(el, new DOMRect(x, y, width, height))
  return el
}

function render(html: string): HTMLElement {
  const root = document.createElement('div')
  root.innerHTML = html
  document.body.appendChild(root)
  return root
}

beforeEach(() => {
  vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (this: Element) {
    return boxes.get(this) ?? new DOMRect(0, 0, 0, 0)
  })
  // the topmost placed element whose box holds the point, as a hit test would find it
  atPoint = (x, y) => {
    const hits = Array.from(document.body.querySelectorAll('*')).filter((el) => {
      const box = boxes.get(el)
      return box != null && x >= box.left && x < box.right && y >= box.top && y < box.bottom
    })
    return hits[hits.length - 1] ?? null
  }
  document.elementFromPoint = (x: number, y: number) => atPoint(x, y)
})

afterEach(() => {
  vi.useRealTimers()
  vi.restoreAllMocks()
  document.body.innerHTML = ''
})

describe('finding controls', () => {
  it('names a control by its content, aria-label, label or placeholder', () => {
    const root = render(`
      <button id="a">Back<b>pack</b></button>
      <button id="b" aria-label="Close"><svg></svg></button>
      <label for="c">Search places</label><input id="c">
      <input id="d" placeholder="Say something">
      <button id="e"><img alt="Settings"><span aria-hidden="true">⚙</span></button>`)
    for (const el of Array.from(root.querySelectorAll('button, input'))) place(el, 0, 0)
    expect(listControls()).toEqual([
      { role: 'button', name: 'Back pack' },
      { role: 'button', name: 'Close' },
      { role: 'textbox', name: 'Search places' },
      { role: 'textbox', name: 'Say something' },
      { role: 'button', name: 'Settings' }
    ])
  })

  it('matches a name exactly, ignoring case, unless exact is false', () => {
    const root = render('<button>Explore as guest</button><a href="#">Explore</a>')
    for (const el of Array.from(root.children)) place(el, 0, 0)
    expect(findControls({ name: 'explore as GUEST' })).toHaveLength(1)
    expect(findControls({ name: 'Explore' })).toHaveLength(1)
    expect(findControls({ name: 'explore', exact: false })).toHaveLength(2)
    expect(findControls({ name: 'Explore', role: 'button' })).toHaveLength(0)
  })

  it('skips what a user cannot see', () => {
    const root = render(`
      <button id="shown">Go</button>
      <button id="empty">Go</button>
      <div aria-hidden="true"><button id="aria">Go</button></div>
      <button id="none" style="display: none">Go</button>`)
    for (const id of ['shown', 'aria', 'none']) place(root.querySelector(`#${id}`)!, 0, 0)
    expect(findControls({ name: 'Go' }).map((el) => el.id)).toEqual(['shown'])
  })
})

describe('describing a control', () => {
  it('is actionable when nothing covers its centre', () => {
    const root = render('<button>Backpack</button>')
    place(root.firstElementChild!, 10, 20, 100, 40)
    expect(describeControl({ name: 'Backpack' })).toMatchObject({
      found: true,
      role: 'button',
      name: 'Backpack',
      visible: true,
      actionable: true,
      x: 10,
      y: 20,
      width: 100,
      height: 40
    })
  })

  it('says why a user could not click it', () => {
    const root = render('<button id="a">Under</button><div id="cover"></div><button id="b" disabled>Off</button>')
    place(root.querySelector('#a')!, 0, 0)
    place(root.querySelector('#cover')!, 0, 0, 500, 500)
    place(root.querySelector('#b')!, 600, 0)
    expect(describeControl({ name: 'Under' })).toMatchObject({ actionable: false, reason: 'covered by <div>' })
    expect(describeControl({ name: 'Off' })).toMatchObject({ actionable: false, reason: 'disabled' })
  })

  it('needs an index when several controls match, and reports none as not found', () => {
    const root = render('<button>Equip</button><button>Equip</button>')
    place(root.children[0], 0, 0)
    place(root.children[1], 0, 100)
    expect(describeControl({ name: 'Equip' })).toEqual({ found: false, error: '2 controls match; pass an index' })
    expect(describeControl({ name: 'Equip', index: 1 })).toMatchObject({ found: true, y: 100 })
    expect(describeControl({ name: 'Equip', index: 2 })).toEqual({
      found: false,
      error: '2 controls match; index 2 is past them'
    })
    expect(describeControl({ name: 'Unequip' })).toEqual({ found: false })
  })

  it('reads values and checked state', () => {
    const root = render('<input type="checkbox" aria-label="Mute" checked><button aria-pressed="true">Mic</button>')
    for (const el of Array.from(root.children)) place(el, 0, 0)
    expect(describeControl({ name: 'Mute' })).toMatchObject({ role: 'checkbox', checked: true, value: 'on' })
    expect(describeControl({ name: 'Mic' })).toMatchObject({ checked: true })
  })
})

describe('acting on the web', () => {
  it('clicks a control with the events a user click fires', async () => {
    const root = render('<button>Backpack</button>')
    const button = place(root.firstElementChild!, 0, 0)
    const seen: string[] = []
    for (const type of ['pointerdown', 'mousedown', 'pointerup', 'mouseup', 'click']) {
      button.addEventListener(type, () => seen.push(type))
    }
    await clickControl({ name: 'Backpack' })
    expect(seen).toEqual(['pointerdown', 'mousedown', 'pointerup', 'mouseup', 'click'])
    expect(document.activeElement).toBe(button)
  })

  it('refuses a control a user could not click', async () => {
    const root = render('<button disabled>Off</button>')
    place(root.firstElementChild!, 0, 0)
    await expect(clickControl({ name: 'Off' })).rejects.toThrow("the control can't be clicked: disabled")
    await expect(clickControl({ name: 'Nothing' })).rejects.toThrow('no HUD control matches')
  })

  it('fills a text field so input listeners see the new value', async () => {
    const root = render('<input aria-label="Search" value="old">')
    const input = place(root.firstElementChild!, 0, 0) as HTMLInputElement
    const values: string[] = []
    input.addEventListener('input', () => values.push(input.value))
    expect(await fillControl({ name: 'Search' }, 'Genesis Plaza')).toMatchObject({ value: 'Genesis Plaza' })
    expect(values).toEqual(['Genesis Plaza'])
  })

  it('presses keys on the focused element, as the engine names them', async () => {
    const root = render('<button>Focus</button>')
    const button = root.firstElementChild as HTMLButtonElement
    button.focus()
    const seen: string[] = []
    button.addEventListener('keydown', (e) => seen.push(`down ${e.key} ${e.code}`))
    button.addEventListener('keyup', (e) => seen.push(`up ${e.key} ${e.code}`))
    expect(await pressKey('KeyB')).toBe('synthetic')
    await pressKey('Space', 'down')
    expect(seen).toEqual(['down b KeyB', 'up b KeyB', 'down   Space'])
    await expect(pressKey('Hyper')).rejects.toThrow('unknown key code `Hyper`')
  })

  it('maps key codes like the engine', () => {
    expect(keyFromCode('KeyZ')).toBe('z')
    expect(keyFromCode('Digit0')).toBe('0')
    expect(keyFromCode('F12')).toBe('F12')
    expect(keyFromCode('Escape')).toBe('Escape')
    for (const unknown of ['Keyb', 'F13', 'F0', 'Digit10', 'constructor']) expect(keyFromCode(unknown)).toBeNull()
  })
})

describe('scrolling a control into view', () => {
  // jsdom has no scrollIntoView: stand one in, which moves the box as a scroll would
  let scrolls: unknown[]
  let onScroll: (el: Element) => void
  beforeEach(() => {
    scrolls = []
    onScroll = () => undefined
    Element.prototype.scrollIntoView = function (this: Element, options?: boolean | ScrollIntoViewOptions) {
      scrolls.push(options)
      onScroll(this)
    }
  })
  afterEach(() => {
    delete (Element.prototype as Partial<Element>).scrollIntoView
  })

  it('scrolls instantly to the nearest edge for a click, and measures after', () => {
    // a panel clips the button: its centre is in the window but under the panel's footer
    const root = render('<button>Equip</button><div id="footer"></div>')
    const button = place(root.firstElementChild!, 10, 90, 100, 30)
    place(root.querySelector('#footer')!, 0, 100, 500, 100)
    expect(describeControl({ name: 'Equip' })).toMatchObject({ actionable: false, reason: 'covered by <div>' })
    expect(scrolls).toEqual([])
    onScroll = (el) => place(el, 10, 20, 100, 30)
    expect(answerHudRequest({ id: 1, op: 'locate', selector: { name: 'Equip' } })).toMatchObject({
      actionable: true,
      y: 20
    })
    expect(scrolls).toEqual([{ block: 'nearest', inline: 'nearest', behavior: 'instant' }])
    expect(boxes.get(button)?.y).toBe(20)
  })

  it('reads a control without moving it', () => {
    const root = render('<button>Equip</button>')
    place(root.firstElementChild!, 0, 0)
    describeControl({ name: 'Equip' })
    answerHudRequest({ id: 1, op: 'describe', selector: { name: 'Equip' } })
    expect(scrolls).toEqual([])
  })

  it('scrolls before a web click, so a clipped control can be clicked', async () => {
    const root = render('<button>Equip</button><div id="footer"></div>')
    const button = place(root.firstElementChild!, 10, 90, 100, 30)
    place(root.querySelector('#footer')!, 0, 100, 500, 100)
    onScroll = (el) => place(el, 10, 20, 100, 30)
    let clicked = false
    button.addEventListener('click', () => (clicked = true))
    await clickControl({ name: 'Equip' })
    expect(clicked).toBe(true)
  })
})

describe('filling a control', () => {
  it('fills text-like inputs, text areas and editable content', async () => {
    const root = render(`
      <input aria-label="email" type="email"><input aria-label="secret" type="password">
      <input aria-label="count" type="number"><textarea aria-label="bio"></textarea>
      <div role="textbox" aria-label="note" contenteditable="true"></div>`)
    for (const [i, el] of Array.from(root.children).entries()) place(el, 0, i * 40)
    // jsdom does not compute isContentEditable
    Object.defineProperty(root.querySelector('[contenteditable]')!, 'isContentEditable', { value: true })
    for (const [name, text] of [
      ['email', 'a@example.com'],
      ['secret', 'x'],
      ['count', '3'],
      ['bio', 'hi'],
      ['note', 'hello']
    ]) {
      expect(await fillControl({ name }, text)).toMatchObject({ value: text })
    }
    expect(root.querySelector('[contenteditable]')!.textContent).toBe('hello')
  })

  it('refuses what takes no text, without clicking it', async () => {
    const root = render(`
      <input type="checkbox" aria-label="Mute"><input type="range" aria-label="Volume">
      <input type="file" aria-label="Upload"><input type="color" aria-label="Tint">
      <button>Send</button>`)
    for (const [i, el] of Array.from(root.children).entries()) place(el, 0, i * 40)
    const clicked: string[] = []
    root.addEventListener('click', (e) => clicked.push((e.target as Element).tagName))
    await expect(fillControl({ name: 'Mute' }, 'x')).rejects.toThrow('the control is not a text field: it is an <input type="checkbox">')
    await expect(fillControl({ name: 'Volume' }, '5')).rejects.toThrow('<input type="range">')
    await expect(fillControl({ name: 'Upload' }, 'x')).rejects.toThrow('<input type="file">')
    await expect(fillControl({ name: 'Tint' }, '#fff')).rejects.toThrow('<input type="color">')
    await expect(fillControl({ name: 'Send' }, 'x')).rejects.toThrow('the control is not a text field: it is a <button>')
    expect(clicked).toEqual([])
    expect((root.querySelector('[type=checkbox]') as HTMLInputElement).checked).toBe(false)
  })

  it('refuses a read-only field', async () => {
    const root = render(`
      <input aria-label="Id" value="42" readonly><textarea aria-label="Log" readonly></textarea>
      <div role="textbox" aria-readonly="true" aria-label="Doc" contenteditable="true"></div>`)
    for (const [i, el] of Array.from(root.children).entries()) place(el, 0, i * 40)
    Object.defineProperty(root.querySelector('div')!, 'isContentEditable', { value: true })
    for (const name of ['Id', 'Log', 'Doc']) await expect(fillControl({ name }, 'x')).rejects.toThrow('the field is read-only')
    expect((root.querySelector('input') as HTMLInputElement).value).toBe('42')
  })

  it('tells the native engine a field takes no text, after saying a control cannot be clicked', () => {
    const root = render(`
      <input type="checkbox" aria-label="Mute"><input aria-label="Id" readonly>
      <input aria-label="Name"><button disabled>Off</button>`)
    for (const [i, el] of Array.from(root.children).entries()) place(el, 0, i * 40)
    const ask = (name: string) => answerHudRequest({ id: 1, op: 'fill', selector: { name } })
    expect(ask('Mute')).toEqual({ found: false, error: 'the control is not a text field: it is an <input type="checkbox">' })
    expect(ask('Id')).toEqual({ found: false, error: 'the field is read-only' })
    expect(ask('Name')).toMatchObject({ found: true, actionable: true })
    expect(ask('Off')).toMatchObject({ found: true, actionable: false, reason: 'disabled' })
    expect(ask('Nothing')).toEqual({ found: false })
  })
})

describe('the commands', () => {
  it('take a JSON selector or a name', () => {
    expect(parseSelector(' Explore as guest ')).toEqual({ name: 'Explore as guest' })
    expect(parseSelector('{"name":"Search","text":"x"}')).toEqual({ name: 'Search', text: 'x' })
    expect(parseSelector('42')).toEqual({ name: '42' })
  })

  it("take the name window.engine's wrapper sends as a JSON string", () => {
    // engine.hudClick('Backpack') arrives as "Backpack", quotes included
    expect(parseSelector('"Backpack"')).toEqual({ name: 'Backpack' })
    expect(parseSelector(' "Explore as guest" ')).toEqual({ name: 'Explore as guest' })
    expect(parseSelector('["Backpack"]')).toEqual({ name: '["Backpack"]' })
    const root = render('<button>Backpack</button>')
    place(root.firstElementChild!, 0, 0)
    expect(describeControl(parseSelector('"Backpack"'))).toMatchObject({ found: true, name: 'Backpack' })
  })

  it('answer as the console RPC does, with a string', async () => {
    const root = render('<button>Backpack</button>')
    place(root.firstElementChild!, 0, 0)
    expect(JSON.parse(await PAGE_COMMANDS['/hud_read'].run(' Backpack'))).toMatchObject({ found: true })
    expect(JSON.parse(await PAGE_COMMANDS['/hud_read'].run('"Backpack"'))).toMatchObject({ found: true })
    expect(JSON.parse(await PAGE_COMMANDS['/hud_click'].run('"Backpack"'))).toMatchObject({ found: true })
    expect(JSON.parse(await PAGE_COMMANDS['/hud_list'].run(' button'))).toEqual([{ role: 'button', name: 'Backpack' }])
    await expect(PAGE_COMMANDS['/hud_fill'].run('{"name":"Backpack"}')).rejects.toThrow('expected JSON')
    await expect(PAGE_COMMANDS['/press_key'].run('KeyB --state sideways')).rejects.toThrow('unknown state')
    await expect(PAGE_COMMANDS['/click_at'].run('7')).rejects.toThrow('expected /click_at <x> <y>')
  })

  it("answer the native engine's requests", () => {
    const root = render('<button>Backpack</button>')
    place(root.firstElementChild!, 0, 0)
    expect(answerHudRequest({ id: 1, op: 'list', selector: {} })).toEqual([{ role: 'button', name: 'Backpack' }])
    expect(answerHudRequest({ id: 2, op: 'locate', selector: { name: 'Backpack' } })).toMatchObject({
      found: true,
      actionable: true
    })
  })
})

describe('waiting for a control', () => {
  // the page changes while a wait polls: advance fake time, then change the DOM
  const later = async (ms: number, change: () => void = () => undefined): Promise<void> => {
    await vi.advanceTimersByTimeAsync(ms)
    change()
  }
  const settled = <T>(promise: Promise<T>): Promise<{ ok: true; value: T } | { ok: false; error: string }> =>
    promise.then(
      (value) => ({ ok: true as const, value }),
      (e: Error) => ({ ok: false as const, error: e.message })
    )

  beforeEach(() => {
    vi.useFakeTimers()
  })

  it('answers at once when the control is already there', async () => {
    const root = render('<button>Backpack</button>')
    place(root.firstElementChild!, 0, 0)
    expect(await waitForControl({ name: 'Backpack' })).toMatchObject({ found: true, role: 'button', actionable: true })
  })

  it('waits for a control to appear, polling', async () => {
    const wait = settled(waitForControl({ name: 'Backpack', timeout: 5000 }))
    await later(350, () => place(render('<button>Backpack</button>').firstElementChild!, 0, 0))
    await later(100)
    expect(await wait).toMatchObject({ ok: true, value: { found: true, name: 'Backpack' } })
  })

  it('waits for a control to go, answering that none is found', async () => {
    const root = render('<button>Spinner</button>')
    place(root.firstElementChild!, 0, 0)
    const wait = settled(waitForControl({ name: 'Spinner', state: 'hidden' }))
    await later(250, () => root.remove())
    await later(100)
    expect(await wait).toEqual({ ok: true, value: { found: false } })
    // already gone: at once
    expect(await waitForControl({ name: 'Spinner', state: 'hidden' })).toEqual({ found: false })
  })

  it('counts only visible controls as there, and an index past the matches as gone', async () => {
    const root = render('<button>Row</button><button>Row</button>')
    for (const [i, el] of Array.from(root.children).entries()) place(el, 0, i * 40)
    const wait = settled(waitForControl({ name: 'Row', index: 1, state: 'hidden', timeout: 1000 }))
    await later(150, () => root.children[1].remove())
    await later(100)
    expect(await wait).toEqual({ ok: true, value: { found: false } })
    const stillThere = settled(waitForControl({ name: 'Row', index: 0, state: 'hidden', timeout: 0 }))
    expect(await stillThere).toMatchObject({ ok: false, error: expect.stringContaining('to be hidden: the control is still visible') })
  })

  it('waits for a control to be enabled', async () => {
    const root = render('<button disabled>Equip</button>')
    const button = place(root.firstElementChild!, 0, 0) as HTMLButtonElement
    const wait = settled(waitForControl({ name: 'Equip', state: 'enabled' }))
    await later(300, () => (button.disabled = false))
    await later(100)
    expect(await wait).toMatchObject({ ok: true, value: { found: true, disabled: false } })
  })

  it('waits for a control to be actionable, not covered', async () => {
    const root = render('<button>Equip</button><div id="cover"></div>')
    place(root.firstElementChild!, 0, 0)
    place(root.querySelector('#cover')!, 0, 0, 500, 500)
    // visible and enabled count at once; actionable waits for the cover to go
    expect(await waitForControl({ name: 'Equip', state: 'enabled' })).toMatchObject({ actionable: false })
    const wait = settled(waitForControl({ name: 'Equip', state: 'actionable' }))
    await later(200, () => root.querySelector('#cover')!.remove())
    await later(100)
    expect(await wait).toMatchObject({ ok: true, value: { found: true, actionable: true } })
  })

  it('keeps waiting while several controls match, until one is left', async () => {
    const root = render('<button>Equip</button><button>Equip</button>')
    for (const [i, el] of Array.from(root.children).entries()) place(el, 0, i * 40)
    const wait = settled(waitForControl({ name: 'Equip' }))
    await later(250, () => root.children[0].remove())
    await later(100)
    expect(await wait).toMatchObject({ ok: true, value: { found: true, y: 40 } })
  })

  it('fails with what it last saw when the time is up', async () => {
    const root = render(`
      <button>Equip</button><div id="cover"></div><button disabled>Off</button>
      <button>Dup</button><button>Dup</button>`)
    place(root.children[0], 0, 0)
    place(root.querySelector('#cover')!, 0, 0, 500, 500)
    place(root.children[2], 0, 600)
    place(root.children[3], 0, 700)
    place(root.children[4], 0, 800)
    const failure = async (options: Parameters<typeof waitForControl>[0]): Promise<string> => {
      const wait = settled(waitForControl(options))
      await later(options.timeout ?? 10_000)
      const result = await wait
      if (result.ok) throw new Error('did not time out')
      return result.error
    }
    expect(await failure({ name: 'Equip', state: 'actionable', timeout: 3000 })).toBe(
      'timed out after 3000 ms waiting for {"name":"Equip"} to be actionable: the control can\'t be clicked: covered by <div>'
    )
    expect(await failure({ name: 'Off', state: 'enabled', timeout: 500 })).toBe(
      'timed out after 500 ms waiting for {"name":"Off"} to be enabled: the control is disabled'
    )
    expect(await failure({ name: 'Nothing', role: 'link', timeout: 250 })).toBe(
      'timed out after 250 ms waiting for {"name":"Nothing","role":"link"} to be visible: no HUD control matches'
    )
    expect(await failure({ name: 'Dup', timeout: 200 })).toContain('2 controls match; pass an index')
    expect(await failure({ name: 'Equip', state: 'hidden', timeout: 100 })).toBe(
      'timed out after 100 ms waiting for {"name":"Equip"} to be hidden: the control is still visible'
    )
  })

  it('waits ten seconds unless told otherwise, and checks once more at the deadline', async () => {
    let wait = settled(waitForControl({ name: 'Never' }))
    let done = false
    void wait.then(() => (done = true))
    await later(9_900)
    expect(done).toBe(false)
    await later(100)
    expect(await wait).toMatchObject({ ok: false, error: expect.stringContaining('timed out after 10000 ms') })

    // the control arrives just before the deadline: still found
    wait = settled(waitForControl({ name: 'Late', timeout: 1000 }))
    await later(990, () => place(render('<button>Late</button>').firstElementChild!, 0, 0))
    await later(10)
    expect(await wait).toMatchObject({ ok: true })
  })

  it('takes a JSON null for the default state and timeout', async () => {
    const root = render('<button>Backpack</button>')
    place(root.firstElementChild!, 0, 0)
    const nulls = { state: null, timeout: null } as unknown as { state: string; timeout: number }
    expect(JSON.parse(await PAGE_COMMANDS['/hud_wait'].run(JSON.stringify({ name: 'Backpack', ...nulls })))).toMatchObject({ found: true })
    const wait = settled(waitForControl({ name: 'Never', ...nulls }))
    await later(10_000)
    expect(await wait).toMatchObject({ ok: false, error: expect.stringContaining('timed out after 10000 ms') })
  })

  it('refuses a state or timeout it does not know', async () => {
    await expect(waitForControl({ name: 'x', state: 'ready' })).rejects.toThrow(
      'unknown state `ready`: use visible, hidden, enabled, actionable'
    )
    for (const timeout of [-1, Number.NaN, '5s' as unknown as number]) {
      await expect(waitForControl({ name: 'x', timeout })).rejects.toThrow('expected "timeout" to be a number of milliseconds')
    }
  })

  it('fails at once for a selector it cannot read', async () => {
    await expect(waitForControl({ name: 7 as unknown as string })).rejects.toThrow()
  })

  it('is the /hud_wait command, for the web', async () => {
    const root = render('<button>Backpack</button>')
    place(root.firstElementChild!, 0, 0)
    const run = PAGE_COMMANDS['/hud_wait'].run
    expect(JSON.parse(await run('Backpack'))).toMatchObject({ found: true })
    // what window.engine.hudWait('Backpack') sends
    expect(JSON.parse(await run('"Backpack"'))).toMatchObject({ found: true })
    expect(JSON.parse(await run('{"name":"Backpack","state":"actionable","timeout":500}'))).toMatchObject({ actionable: true })
    expect(JSON.parse(await run('{"name":"Gone","state":"hidden"}'))).toEqual({ found: false })
    const timedOut = settled(Promise.resolve(run('{"name":"Gone","timeout":300}')))
    await later(300)
    expect(await timedOut).toMatchObject({ ok: false, error: expect.stringContaining('timed out after 300 ms') })
  })

  it("answers the native engine's wait with the control or the error", async () => {
    const root = render('<button>Backpack</button>')
    place(root.firstElementChild!, 0, 0)
    expect(await answerHudWait({ id: 1, op: 'wait', selector: { name: 'Backpack' } })).toMatchObject({
      result: { found: true, name: 'Backpack' }
    })
    expect(await answerHudWait({ id: 2, op: 'wait', selector: { name: 'Backpack', state: 'hidden', timeout: 0 } })).toEqual({
      error: 'timed out after 0 ms waiting for {"name":"Backpack"} to be hidden: the control is still visible'
    })
    expect(await answerHudWait({ id: 3, op: 'wait', selector: { state: 'nope' } })).toEqual({
      error: 'unknown state `nope`: use visible, hidden, enabled, actionable'
    })
  })
})

describe('trusted input from a test runner', () => {
  type Sent = { id: number; op: string; [field: string]: unknown }
  type RunnerWindow = Window & {
    __dclTrustedInput?: unknown
    __dclTrustedInputDone?: (id: number, error: string | null) => void
  }
  const runnerWindow = window as RunnerWindow
  // a request as the runner receives it: the page chooses the id
  const req = (fields: object) => ({ id: expect.any(Number), ...fields })

  let runner: ReturnType<typeof vi.fn<(json: string) => void>>
  let sent: Sent[]
  // what the input does to the page, as the browser would
  let effect: (request: Sent) => void
  // the runner's answer to a request: an error message, null for done, false for none
  let answer: (request: Sent) => string | null | false

  beforeEach(() => {
    sent = []
    effect = () => undefined
    answer = () => null
    runner = vi.fn((json: string) => {
      const request = JSON.parse(json) as Sent
      sent.push(request)
      effect(request)
      // the runner answers once its input call has returned: on a later turn
      const reply = answer(request)
      if (reply !== false) queueMicrotask(() => runnerWindow.__dclTrustedInputDone?.(request.id, reply))
    })
    runnerWindow.__dclTrustedInput = runner
    installHudAutomation()
  })
  afterEach(() => {
    delete runnerWindow.__dclTrustedInput
    delete runnerWindow.__dclTrustedInputDone
    delete (window as { __dclPageCommands?: unknown }).__dclPageCommands
  })

  const placeButton = (): Element => place(render('<button>Backpack</button>').firstElementChild!, 10, 20, 100, 40)
  // the command's promise, whether or not it ran to a promise
  const command = (name: string, rest: string): Promise<string> => Promise.resolve(PAGE_COMMANDS[name].run(rest))

  it('asks for input with one JSON string, a distinct integer id each time', async () => {
    placeButton()
    await PAGE_COMMANDS['/press_key'].run('KeyA')
    await PAGE_COMMANDS['/press_key'].run('KeyB')
    expect(runner.mock.calls).toEqual([[expect.any(String)], [expect.any(String)]])
    expect(sent.every((request) => Number.isInteger(request.id))).toBe(true)
    expect(sent[0].id).not.toBe(sent[1].id)
  })

  it('is available while the runner has added its function', () => {
    expect(trustedInputAvailable()).toBe(true)
    delete runnerWindow.__dclTrustedInput
    expect(trustedInputAvailable()).toBe(false)
    runnerWindow.__dclTrustedInput = 'not a function'
    expect(trustedInputAvailable()).toBe(false)
  })

  describe('clicking a control', () => {
    it('clicks its centre with the runner and dispatches no DOM events', async () => {
      const button = placeButton()
      const reached: string[] = []
      for (const type of ['pointerdown', 'mousedown', 'pointerup', 'mouseup', 'click']) {
        button.addEventListener(type, () => reached.push(type))
      }
      const result = JSON.parse(await PAGE_COMMANDS['/hud_click'].run('Backpack'))
      expect(sent).toEqual([req({ op: 'click', x: 60, y: 40, button: 'left' })])
      expect(result).toMatchObject({ found: true, name: 'Backpack', actionable: true, input: 'trusted' })
      expect(reached).toEqual([])
    })

    it('refuses a control a user could not click, asking for nothing', async () => {
      const root = render('<button disabled>Off</button>')
      place(root.firstElementChild!, 0, 0)
      await expect(PAGE_COMMANDS['/hud_click'].run('Off')).rejects.toThrow("the control can't be clicked: disabled")
      expect(sent).toEqual([])
    })
  })

  describe('filling a field', () => {
    const placeField = (html: string): HTMLInputElement =>
      place(render(html).firstElementChild!, 10, 20, 200, 30) as HTMLInputElement
    // the field's text a browser would replace when typing: what is selected, and where focus is
    const watchTyping = (input: HTMLInputElement, typed: { selected: string; focused: boolean }[]) =>
      (request: Sent): void => {
        if (request.op === 'click') return
        typed.push({
          selected: input.value.slice(input.selectionStart ?? 0, input.selectionEnd ?? 0),
          focused: document.activeElement === input
        })
        input.value = request.op === 'insertText' ? String(request.text) : ''
      }

    it('clicks it, selects its text and inserts the new text over it', async () => {
      const input = placeField('<input aria-label="Search" value="old">')
      const typed: { selected: string; focused: boolean }[] = []
      effect = watchTyping(input, typed)
      const result = JSON.parse(await PAGE_COMMANDS['/hud_fill'].run('{"name":"Search","text":"Genesis"}'))
      expect(sent).toEqual([
        req({ op: 'click', x: 110, y: 35, button: 'left' }),
        req({ op: 'insertText', text: 'Genesis' })
      ])
      // the field is focused, though the click was not a DOM event that could focus it
      expect(typed).toEqual([{ selected: 'old', focused: true }])
      expect(result).toMatchObject({ found: true, name: 'Search', value: 'Genesis', input: 'trusted' })
    })

    it('deletes the selected text for an empty text', async () => {
      const input = placeField('<input aria-label="Search" value="old">')
      const typed: { selected: string; focused: boolean }[] = []
      effect = watchTyping(input, typed)
      const result = JSON.parse(await PAGE_COMMANDS['/hud_fill'].run('{"name":"Search","text":""}'))
      expect(sent).toEqual([
        req({ op: 'click', x: 110, y: 35, button: 'left' }),
        req({ op: 'key', code: 'Delete', key: 'Delete', keyCode: 46, location: 0 })
      ])
      expect(typed).toEqual([{ selected: 'old', focused: true }])
      expect(result).toMatchObject({ value: '', input: 'trusted' })
    })

    it('reports the value the field holds afterwards, not the text it was given', async () => {
      placeField('<input aria-label="Search" value="old">')
      // the page refuses the change
      const result = JSON.parse(await PAGE_COMMANDS['/hud_fill'].run('{"name":"Search","text":"Genesis"}'))
      expect(result).toMatchObject({ value: 'old', input: 'trusted' })
    })

    it('selects the content of an editable element', async () => {
      const root = render('<div role="textbox" aria-label="note" contenteditable="true">old note</div>')
      const note = place(root.firstElementChild!, 0, 0)
      // jsdom does not compute isContentEditable
      Object.defineProperty(note, 'isContentEditable', { value: true })
      let selected = ''
      effect = (request) => {
        if (request.op !== 'insertText') return
        selected = window.getSelection()?.toString() ?? ''
        note.textContent = String(request.text)
      }
      const result = JSON.parse(await PAGE_COMMANDS['/hud_fill'].run('{"name":"note","text":"new note"}'))
      expect(sent.map((request) => request.op)).toEqual(['click', 'insertText'])
      expect(selected).toBe('old note')
      expect(result).toMatchObject({ value: 'new note', input: 'trusted' })
    })

    it('refuses what takes no text before any click, as a checkbox would toggle', async () => {
      const root = render('<input type="checkbox" aria-label="Mute"><input aria-label="Id" readonly>')
      for (const [i, el] of Array.from(root.children).entries()) place(el, 0, i * 40)
      await expect(PAGE_COMMANDS['/hud_fill'].run('{"name":"Mute","text":"x"}')).rejects.toThrow('the control is not a text field')
      await expect(PAGE_COMMANDS['/hud_fill'].run('{"name":"Id","text":"x"}')).rejects.toThrow('the field is read-only')
      expect(sent).toEqual([])
    })

    it("fails with the runner's error when typing fails", async () => {
      placeField('<input aria-label="Search">')
      answer = (request) => (request.op === 'insertText' ? 'the page lost focus' : null)
      await expect(PAGE_COMMANDS['/hud_fill'].run('{"name":"Search","text":"x"}')).rejects.toThrow('the page lost focus')
    })
  })

  describe('pressing a key', () => {
    it('sends the key with its virtual key code, location and the text it types', async () => {
      const run = PAGE_COMMANDS['/press_key'].run
      expect(await run('KeyB')).toBe('pressed KeyB')
      expect(await run('KeyB --state down')).toBe('pressed KeyB')
      expect(await run('KeyB --state up')).toBe('pressed KeyB')
      expect(await run('ShiftLeft')).toBe('pressed ShiftLeft')
      expect(await run('Enter')).toBe('pressed Enter')
      expect(sent).toEqual([
        req({ op: 'key', code: 'KeyB', key: 'b', keyCode: 66, text: 'b', location: 0 }),
        req({ op: 'key', code: 'KeyB', key: 'b', keyCode: 66, text: 'b', location: 0, state: 'down' }),
        req({ op: 'key', code: 'KeyB', key: 'b', keyCode: 66, text: 'b', location: 0, state: 'up' }),
        req({ op: 'key', code: 'ShiftLeft', key: 'Shift', keyCode: 16, location: 1 }),
        req({ op: 'key', code: 'Enter', key: 'Enter', keyCode: 13, text: '\r', location: 0 })
      ])
    })

    it('dispatches no DOM key events', async () => {
      const root = render('<button>Focus</button>')
      const button = root.firstElementChild as HTMLButtonElement
      button.focus()
      const seen: string[] = []
      for (const type of ['keydown', 'keyup']) button.addEventListener(type, () => seen.push(type))
      await PAGE_COMMANDS['/press_key'].run('KeyB')
      expect(seen).toEqual([])
    })

    it('refuses an unknown key or state, asking for nothing', async () => {
      await expect(PAGE_COMMANDS['/press_key'].run('Hyper')).rejects.toThrow('unknown key code `Hyper`')
      await expect(PAGE_COMMANDS['/press_key'].run('KeyB --state sideways')).rejects.toThrow('unknown state `sideways`: use down or up')
      expect(sent).toEqual([])
    })

    it('defines every key the engine names', () => {
      const key = (key: string, keyCode: number, text?: string, location = 0) =>
        text === undefined ? { key, keyCode, location } : { key, keyCode, text, location }
      for (const [i, letter] of Array.from('ABCDEFGHIJKLMNOPQRSTUVWXYZ').entries()) {
        expect(keyDefinition(`Key${letter}`)).toStrictEqual(key(letter.toLowerCase(), 65 + i, letter.toLowerCase()))
      }
      for (let digit = 0; digit <= 9; digit++) {
        expect(keyDefinition(`Digit${digit}`)).toStrictEqual(key(String(digit), 48 + digit, String(digit)))
      }
      for (let n = 1; n <= 12; n++) expect(keyDefinition(`F${n}`)).toStrictEqual(key(`F${n}`, 111 + n))
      const named: [string, ReturnType<typeof key>][] = [
        ['Space', key(' ', 32, ' ')],
        ['Enter', key('Enter', 13, '\r')],
        ['Tab', key('Tab', 9)],
        ['Escape', key('Escape', 27)],
        ['Backspace', key('Backspace', 8)],
        ['Delete', key('Delete', 46)],
        ['ArrowLeft', key('ArrowLeft', 37)],
        ['ArrowUp', key('ArrowUp', 38)],
        ['ArrowRight', key('ArrowRight', 39)],
        ['ArrowDown', key('ArrowDown', 40)],
        ['Home', key('Home', 36)],
        ['End', key('End', 35)],
        ['PageUp', key('PageUp', 33)],
        ['PageDown', key('PageDown', 34)],
        ['ShiftLeft', key('Shift', 16, undefined, 1)],
        ['ShiftRight', key('Shift', 16, undefined, 2)],
        ['ControlLeft', key('Control', 17, undefined, 1)],
        ['ControlRight', key('Control', 17, undefined, 2)],
        ['AltLeft', key('Alt', 18, undefined, 1)],
        ['AltRight', key('Alt', 18, undefined, 2)],
        ['Minus', key('-', 189, '-')],
        ['Equal', key('=', 187, '=')],
        ['Comma', key(',', 188, ',')],
        ['Period', key('.', 190, '.')],
        ['Slash', key('/', 191, '/')],
        ['Semicolon', key(';', 186, ';')],
        ['Quote', key("'", 222, "'")],
        ['Backquote', key('`', 192, '`')],
        ['BracketLeft', key('[', 219, '[')],
        ['BracketRight', key(']', 221, ']')],
        ['Backslash', key('\\', 220, '\\')]
      ]
      for (const [code, definition] of named) expect(keyDefinition(code)).toStrictEqual(definition)
      for (const unknown of ['Keyb', 'F13', 'F0', 'Digit10', 'Hyper', 'constructor']) expect(keyDefinition(unknown)).toBeNull()
    })
  })

  describe('clicking at a position', () => {
    it('clicks there with the left button unless told otherwise, wherever the point lands', async () => {
      const run = PAGE_COMMANDS['/click_at'].run
      expect(await run('10 20')).toBe('clicked at (10, 20)')
      expect(await run('10 20 --button right')).toBe('clicked at (10, 20)')
      expect(await run('10.5 20 --button middle')).toBe('clicked at (10.5, 20)')
      expect(await run('1 2 --button left')).toBe('clicked at (1, 2)')
      expect(sent).toEqual([
        req({ op: 'click', x: 10, y: 20, button: 'left' }),
        req({ op: 'click', x: 10, y: 20, button: 'right' }),
        req({ op: 'click', x: 10.5, y: 20, button: 'middle' }),
        req({ op: 'click', x: 1, y: 2, button: 'left' })
      ])
    })

    it('refuses a button it does not know, asking for nothing', async () => {
      await expect(PAGE_COMMANDS['/click_at'].run('10 20 --button x')).rejects.toThrow('unknown button `x`: use left, right or middle')
      expect(sent).toEqual([])
    })
  })

  describe('when the runner does not deliver', () => {
    it('fails a command with the error the runner answers with', async () => {
      placeButton()
      answer = () => 'Input.dispatchMouseEvent failed'
      await expect(PAGE_COMMANDS['/hud_click'].run('Backpack')).rejects.toThrow('Input.dispatchMouseEvent failed')
      await expect(PAGE_COMMANDS['/press_key'].run('KeyB')).rejects.toThrow('Input.dispatchMouseEvent failed')
      await expect(PAGE_COMMANDS['/click_at'].run('1 2')).rejects.toThrow('Input.dispatchMouseEvent failed')
    })

    it('fails a command whose request the runner cannot be handed', async () => {
      runnerWindow.__dclTrustedInput = () => {
        throw new Error('the binding is gone')
      }
      await expect(PAGE_COMMANDS['/press_key'].run('KeyB')).rejects.toThrow('the binding is gone')
    })

    it('fails with the time it waited when the runner never answers', async () => {
      vi.useFakeTimers()
      placeButton()
      answer = () => false
      let outcome = 'pending'
      const click = command('/hud_click', 'Backpack').then(
        () => (outcome = 'answered'),
        (e: Error) => (outcome = e.message)
      )
      await vi.advanceTimersByTimeAsync(TRUSTED_INPUT_TIMEOUT_MS - 1)
      expect(outcome).toBe('pending')
      await vi.advanceTimersByTimeAsync(1)
      await click
      expect(TRUSTED_INPUT_TIMEOUT_MS).toBe(10_000)
      expect(outcome).toBe('the test runner did not answer a trusted click within 10000 ms')
    })

    it('names the request that was not answered', async () => {
      vi.useFakeTimers()
      place(render('<input aria-label="Search">').firstElementChild!, 0, 0)
      // the click is answered, the text is not
      answer = (request) => (request.op === 'click' ? null : false)
      const fill = command('/hud_fill', '{"name":"Search","text":"x"}').catch((e: Error) => e.message)
      await vi.advanceTimersByTimeAsync(TRUSTED_INPUT_TIMEOUT_MS)
      expect(await fill).toBe('the test runner did not answer a trusted insertText within 10000 ms')
    })

    it('ignores an answer that is late or for no request', async () => {
      vi.useFakeTimers()
      placeButton()
      answer = () => false
      const click = command('/hud_click', 'Backpack').catch((e: Error) => e.message)
      await vi.advanceTimersByTimeAsync(TRUSTED_INPUT_TIMEOUT_MS)
      expect(await click).toContain('did not answer')
      expect(() => runnerWindow.__dclTrustedInputDone!(sent[0].id, null)).not.toThrow()
      expect(() => runnerWindow.__dclTrustedInputDone!(9999, 'nobody asked')).not.toThrow()
    })

    it('stops waiting once answered', async () => {
      vi.useFakeTimers()
      placeButton()
      await PAGE_COMMANDS['/hud_click'].run('Backpack')
      expect(vi.getTimerCount()).toBe(0)
    })

    it('settles each request by its id when answers come out of order', async () => {
      answer = () => false
      const first = command('/press_key', 'KeyA')
      const second = command('/press_key', 'KeyB')
      let firstDone = false
      void first.then(() => (firstDone = true))
      runnerWindow.__dclTrustedInputDone!(sent[1].id, null)
      expect(await second).toBe('pressed KeyB')
      expect(firstDone).toBe(false)
      runnerWindow.__dclTrustedInputDone!(sent[0].id, null)
      expect(await first).toBe('pressed KeyA')
    })
  })
})

describe('without a test runner', () => {
  it("is not available, and installing the commands adds the hook for the runner's answers", () => {
    expect(trustedInputAvailable()).toBe(false)
    installHudAutomation()
    const w = window as Window & { __dclPageCommands?: Record<string, unknown>; __dclTrustedInputDone?: unknown }
    expect(typeof w.__dclTrustedInputDone).toBe('function')
    expect(Object.keys(w.__dclPageCommands ?? {})).toEqual(Object.keys(PAGE_COMMANDS))
    delete w.__dclPageCommands
    delete w.__dclTrustedInputDone
  })

  it('dispatches DOM events and says so', async () => {
    const root = render('<button>Backpack</button><input aria-label="Search" value="old">')
    for (const [i, el] of Array.from(root.children).entries()) place(el, 0, i * 40)
    expect(JSON.parse(await PAGE_COMMANDS['/hud_click'].run('Backpack'))).toMatchObject({ found: true, input: 'synthetic' })
    expect(JSON.parse(await PAGE_COMMANDS['/hud_fill'].run('{"name":"Search","text":"x"}'))).toMatchObject({
      value: 'x',
      input: 'synthetic'
    })
    expect(await PAGE_COMMANDS['/press_key'].run('KeyB')).toBe('pressed KeyB')
    expect(await PAGE_COMMANDS['/click_at'].run('5 5')).toBe('clicked at (5, 5)')
  })

  it('clicks at a position with the button asked for: click, auxclick or contextmenu', async () => {
    const root = render('<div></div>')
    const target = place(root.firstElementChild!, 0, 0)
    const seen: string[] = []
    for (const type of ['mousedown', 'mouseup', 'click', 'auxclick', 'contextmenu']) {
      target.addEventListener(type, (e) => seen.push(`${type}:${(e as MouseEvent).button}:${(e as MouseEvent).buttons}`))
    }
    expect(await PAGE_COMMANDS['/click_at'].run('5 5')).toBe('clicked at (5, 5)')
    expect(seen.splice(0)).toEqual(['mousedown:0:1', 'mouseup:0:0', 'click:0:0'])
    expect(await PAGE_COMMANDS['/click_at'].run('5 5 --button right')).toBe('clicked at (5, 5)')
    expect(seen.splice(0)).toEqual(['mousedown:2:2', 'mouseup:2:0', 'contextmenu:2:0'])
    expect(await PAGE_COMMANDS['/click_at'].run('5 5 --button middle')).toBe('clicked at (5, 5)')
    expect(seen.splice(0)).toEqual(['mousedown:1:4', 'mouseup:1:0', 'auxclick:1:0'])
    await expect(PAGE_COMMANDS['/click_at'].run('5 5 --button x')).rejects.toThrow('unknown button `x`: use left, right or middle')
    expect(seen).toEqual([])
  })

  it('says when nothing is at the point', async () => {
    await expect(clickAt(900, 900)).rejects.toThrow('nothing at (900, 900)')
  })
})

describe("the native bridge's hudAutomation listener", () => {
  type Reply = { to: string; kind: string; id: number; result?: unknown; error?: string }
  let replies: Reply[]
  let hear: (payload: unknown) => void

  beforeEach(() => {
    replies = []
    const listeners = new Map<string, (payload: unknown) => void>()
    ;(window as unknown as { cef: unknown }).cef = {
      emit: (payload: Reply) => replies.push(payload),
      listen: (id: string, cb: (payload: unknown) => void) => listeners.set(id, cb)
    }
    vi.stubGlobal('BroadcastChannel', class { postMessage() {} })
    installCefNativeBridge()
    hear = listeners.get('hudAutomation')!
  })
  afterEach(() => {
    delete (window as unknown as { cef?: unknown }).cef
    vi.unstubAllGlobals()
  })

  it('drops what is not a request instead of throwing', () => {
    for (const payload of ['null', '7', '"list"', '[]', 'true', '{ not json', undefined]) {
      expect(() => hear(payload)).not.toThrow()
    }
    expect(replies).toEqual([])
  })

  it('answers a request with its id', () => {
    const root = render('<button>Backpack</button>')
    place(root.firstElementChild!, 0, 0)
    hear(JSON.stringify({ id: 4, op: 'describe', selector: { name: 'Backpack' } }))
    expect(replies).toMatchObject([{ to: 'engine', kind: 'hudAutomationReply', id: 4, result: { found: true } }])
  })

  it('answers a wait when it is over: the control, or the error', async () => {
    vi.useFakeTimers()
    hear(JSON.stringify({ id: 5, op: 'wait', selector: { name: 'Backpack', timeout: 300 } }))
    hear(JSON.stringify({ id: 6, op: 'wait', selector: { name: 'Gone', state: 'hidden' } }))
    await vi.advanceTimersByTimeAsync(0)
    expect(replies).toEqual([{ to: 'engine', kind: 'hudAutomationReply', id: 6, result: { found: false } }])
    const root = render('<button>Backpack</button>')
    place(root.firstElementChild!, 0, 0)
    await vi.advanceTimersByTimeAsync(100)
    expect(replies[1]).toMatchObject({ id: 5, result: { found: true, name: 'Backpack' } })
    hear(JSON.stringify({ id: 7, op: 'wait', selector: { name: 'Nothing', timeout: 200 } }))
    await vi.advanceTimersByTimeAsync(200)
    expect(replies[2]).toEqual({
      to: 'engine',
      kind: 'hudAutomationReply',
      id: 7,
      error: 'timed out after 200 ms waiting for {"name":"Nothing"} to be visible: no HUD control matches'
    })
  })
})
