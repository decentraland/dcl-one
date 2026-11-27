// DOMAIN: the engine page's key forwarding (deploy/web/engine/boot.js): winit listens on the canvas,
// so keys pressed while a HUD control holds the focus are cloned to it. A text field keeps its keys
// (typing never moves the avatar), and so does a slider (its arrows, Home, End and Page keys adjust
// it; the HUD closes the top layer on its cancel key itself). A switch, a checkbox or a radio takes no
// text, so Escape and the movement keys still reach the engine from it, while Space and Enter press it
// instead of reaching Jump or Chat. The HUD declares text focus to the engine by the same rule
// (isEditableTarget, which states it), and the last block holds the two to it. boot.js is imported as
// the page does, over a stand-in for the engine.

import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest'
import { isCancelKey, isEditableTarget, isSliderTarget, setBindingsSnapshot } from '@hud/lib/bindingLabels'
import { SLIDER_KEYS } from '@hud/test/sliderKeys'
// boot.js reads window.__serviceUrl as it is evaluated: the host defines it there
import '../lib/baseDomain'

vi.mock('../../../deploy/web/engine/engine.js', () => ({
  initEngine: () => Promise.resolve(),
  // the engine never gets further than its boot: only the page's key forwarding is under test
  prepareRender: () => new Promise(() => undefined),
  start: vi.fn(),
  applyOptionsToUrlParams: vi.fn(),
  engine_home_scene: vi.fn(),
  engine_terms_accepted: vi.fn(),
  engine_crash_event: vi.fn(() => null),
  engine_set_pulse_certificate_hash: vi.fn(),
  engine_set_perf_stream: vi.fn(),
  engine_perf_take: vi.fn(),
  engine_set_scene_loop_mode: vi.fn()
}))

const realError = console.error
/** What the engine heard: the key events that reached the canvas, as `keydown Escape`. */
const heard: string[] = []
let canvas: HTMLCanvasElement

beforeAll(async () => {
  // before the import: the crash watchdog's interval starts as boot.js is evaluated
  vi.useFakeTimers()
  canvas = document.createElement('canvas')
  canvas.id = 'mygame-canvas'
  document.body.append(canvas)
  for (const type of ['keydown', 'keyup']) canvas.addEventListener(type, (e) => heard.push(`${e.type} ${(e as KeyboardEvent).code}`))
  // @ts-expect-error TS7016: plain JS served as is by the engine page, so it has no declarations
  await import('../../../deploy/web/engine/boot.js')
})

afterAll(() => {
  console.error = realError
  vi.useRealTimers()
  canvas.remove()
})

afterEach(() => {
  heard.length = 0
  document.body.querySelectorAll('[data-panel]').forEach((panel) => panel.remove())
  setBindingsSnapshot([]) // module store leaks across tests
})

/** `html`'s first element, inside a HUD panel on the page. */
function control(html: string): HTMLElement {
  const panel = document.createElement('div')
  panel.dataset.panel = ''
  panel.innerHTML = html
  document.body.append(panel)
  return panel.querySelector('[data-focus]') ?? (panel.firstElementChild as HTMLElement)
}

/** A key pressed and released while `target` holds the focus; what the engine heard of it. */
function press(target: HTMLElement, code: string, key = code): string[] {
  heard.length = 0
  for (const type of ['keydown', 'keyup']) target.dispatchEvent(new KeyboardEvent(type, { code, key, bubbles: true, cancelable: true, composed: true }))
  return [...heard]
}

const both = (code: string) => [`keydown ${code}`, `keyup ${code}`]

// The HUD's Switch (library/ui/components/Switch): a native checkbox in a label.
const SWITCH = '<label><span>Auto</span><span><input data-focus type="checkbox" role="switch"><span aria-hidden="true"></span></span></label>'
const NON_TEXT = { switch: SWITCH, checkbox: '<input type="checkbox">', radio: '<input type="radio" name="level">' }
const TEXT_FIELDS = {
  'an input with no type': '<input>',
  'a text input': '<input type="text">',
  'a search field': '<input type="search" placeholder="Search">',
  'a textarea': '<textarea></textarea>'
}
const MOVEMENT = ['KeyW', 'KeyA', 'KeyS', 'KeyD']
// The HUD's sliders: the shared Slider (the Volume panel's Master) and the ColorPicker's bars.
const SLIDERS = {
  'the Volume slider': '<input type="range" min="0" max="100" aria-label="Master">',
  'a colour bar': '<input type="range" min="0" max="1" step="0.001" aria-label="Hue">'
}

describe('a focused switch, checkbox or radio', () => {
  it.each(Object.entries(NON_TEXT))('leaves Escape to the engine from a %s, so Cancel closes its panel', (_, html) => {
    expect(press(control(html), 'Escape')).toEqual(both('Escape'))
  })

  it.each(Object.entries(NON_TEXT))('leaves the movement keys to the engine from a %s', (_, html) => {
    const target = control(html)
    for (const code of MOVEMENT) expect(press(target, code, code.slice(3).toLowerCase()), code).toEqual(both(code))
  })

  it.each(Object.entries(NON_TEXT))('keeps Space and Enter on a %s: they press it, never Jump or Chat', (_, html) => {
    const target = control(html)
    for (const [code, key] of [['Space', ' '], ['Enter', 'Enter'], ['NumpadEnter', 'Enter']]) expect(press(target, code, key), code).toEqual([])
    // the press's keyup was withheld with it, and the next key reaches the engine again
    expect(press(target, 'KeyW', 'w')).toEqual(both('KeyW'))
  })

  it('inside a shadow root (the scene editor) leaves Escape to the engine too', () => {
    const host = control('<div></div>')
    const root = host.attachShadow({ mode: 'open' })
    root.innerHTML = SWITCH
    expect(press(root.querySelector('input')!, 'Escape')).toEqual(both('Escape'))
  })
})

describe('a focused text field keeps its keys', () => {
  it.each(Object.entries(TEXT_FIELDS))('typing w in %s does not move the avatar, and Escape and Space stay with it', (_, html) => {
    const target = control(html)
    expect(press(target, 'KeyW', 'w')).toEqual([])
    expect(press(target, 'Escape')).toEqual([])
    expect(press(target, 'Space', ' ')).toEqual([])
  })

  it('in a contenteditable element', () => {
    const target = control('<div contenteditable="true"></div>')
    // jsdom does not compute isContentEditable; the browser reports it for this element
    Object.defineProperty(target, 'isContentEditable', { value: true })
    expect(press(target, 'KeyW', 'w')).toEqual([])
    expect(press(target, 'Escape')).toEqual([])
  })

  it('inside a shadow root (the scene editor)', () => {
    const host = control('<div></div>')
    const root = host.attachShadow({ mode: 'open' })
    root.innerHTML = '<input type="search">'
    expect(press(root.querySelector('input')!, 'KeyW', 'w')).toEqual([])
  })
})

describe('a focused slider keeps its keys', () => {
  it.each(Object.entries(SLIDERS))('the keys that adjust %s never reach the engine: no movement or camera', (_, html) => {
    const target = control(html)
    for (const code of SLIDER_KEYS) expect(press(target, code), code).toEqual([])
  })

  it.each(Object.entries(SLIDERS))('nor do the movement keys, Space or Escape from %s: the HUD acts on its cancel key', (_, html) => {
    const target = control(html)
    for (const code of MOVEMENT) expect(press(target, code, code.slice(3).toLowerCase()), code).toEqual([])
    expect(press(target, 'Space', ' ')).toEqual([])
    expect(press(target, 'Escape')).toEqual([])
  })
})

describe('a focused button (unchanged)', () => {
  it('leaves Escape to the engine and keeps Space for itself', () => {
    const target = control('<button type="button">Skybox</button>')
    expect(press(target, 'Escape')).toEqual(both('Escape'))
    expect(press(target, 'Space', ' ')).toEqual([])
  })
})

describe("the page and the HUD's text focus agree", () => {
  // The HUD declares text focus to the engine (no action resolves) by isEditableTarget; the page
  // withholds keys by its own rule. Where they disagree, a key either reaches an engine that resolves
  // nothing, or never reaches one that would have.
  const cases = {
    ...Object.fromEntries(Object.entries(NON_TEXT).map(([name, html]) => [`a ${name}`, html])),
    ...TEXT_FIELDS,
    ...SLIDERS,
    'a button': '<button type="button">Open</button>',
    'a focusable div': '<div tabindex="0"></div>'
  }
  const keys: [code: string, key: string][] = [
    ['Escape', 'Escape'],
    ['KeyW', 'w'],
    ['KeyR', 'r'],
    ...SLIDER_KEYS.map((code): [string, string] => [code, code])
  ]
  it.each(Object.entries(cases))('for %s, on every key', (_, html) => {
    const target = control(html)
    for (const [code, key] of keys) expect(press(target, code, key).length > 0, code).toBe(!isEditableTarget(target))
  })

  // A cancel key the page withholds can only be acted on by the HUD: a text field types a printable one,
  // and a slider, which types nothing, cancels on it. Rebound to R, only a slider withholds it and cancels.
  it.each(Object.entries(cases))('for %s, a printable cancel key', (_, html) => {
    setBindingsSnapshot([[{ System: 'Cancel' }, ['KeyR']]])
    const target = control(html)
    const withheld = press(target, 'KeyR', 'r').length === 0
    expect(withheld && isCancelKey({ key: 'r', code: 'KeyR', target })).toBe(isSliderTarget(target))
  })
})
