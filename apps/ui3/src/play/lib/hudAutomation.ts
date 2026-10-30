// The HUD's side of the engine's test commands (rendering/bevy-explorer/docs/guidelines/running.md,
// "Test commands"): find a control the way assistive technology does, by role and accessible name,
// and say whether a user could click it — visible, enabled, not covered.
//
// On the web the engine cannot reach this DOM, so the page answers the commands itself:
// installHudAutomation registers /hud_list, /hud_read, /hud_wait, /hud_click, /hud_fill,
// /press_key and /click_at with engine.js, which runs them here instead of sending them to the
// engine. Clicks and keys are real input when a test runner drives the browser and has added
// window.__dclTrustedInput (local-harness/web-driver, over the DevTools Protocol): the page finds
// the control, asks the runner to click or type, and waits for its answer. Without one they are
// DOM events, which a page can tell from a user's (isTrusted is false). Natively the engine asks
// over window.cef (cefNativeBridge.ts, answerHudRequest, answerHudWait) and then clicks and types
// with real input through CEF. Waiting happens here on both: the page polls, the engine just
// relays.

export interface Selector {
  /** the accessible name; exact (case-insensitive) unless `exact` is false */
  name?: string
  role?: string
  /** which of several matches, in document order */
  index?: number
  exact?: boolean
}

/** What a command's JSON can carry beyond the selector: the text /hud_fill types, and the state
 *  and timeout /hud_wait waits for. */
export interface CommandSelector extends Selector {
  text?: string
  state?: string
  timeout?: number
}

export interface Control {
  found: true
  role: string
  name: string
  text: string
  value?: string
  checked?: boolean
  disabled: boolean
  /** always true: a control is found only while it is visible */
  visible: boolean
  /** a user could click it: visible, enabled, inside the viewport and not covered */
  actionable: boolean
  reason?: string
  x: number
  y: number
  width: number
  height: number
}

export type Description = Control | { found: false; error?: string }

const INPUT_ROLES: Record<string, string> = {
  button: 'button',
  submit: 'button',
  reset: 'button',
  image: 'button',
  checkbox: 'checkbox',
  radio: 'radio',
  range: 'slider',
  search: 'searchbox',
  number: 'spinbutton'
}

// Roles that take their name from their content when nothing names them otherwise.
const NAME_FROM_CONTENT = new Set([
  'button',
  'cell',
  'checkbox',
  'columnheader',
  'heading',
  'link',
  'listitem',
  'menuitem',
  'menuitemcheckbox',
  'menuitemradio',
  'option',
  'radio',
  'rowheader',
  'switch',
  'tab',
  'tooltip',
  'treeitem'
])

const normalize = (text: string): string => text.replace(/\s+/g, ' ').trim()

export function roleOf(el: Element): string | null {
  const explicit = el.getAttribute('role')?.trim().split(/\s+/)[0]
  if (explicit) return explicit
  const tag = el.tagName.toLowerCase()
  switch (tag) {
    case 'button':
    case 'summary':
      return 'button'
    case 'a':
    case 'area':
      return el.hasAttribute('href') ? 'link' : null
    case 'textarea':
      return 'textbox'
    case 'select': {
      const select = el as HTMLSelectElement
      return select.multiple || select.size > 1 ? 'listbox' : 'combobox'
    }
    case 'input': {
      const type = ((el as HTMLInputElement).type || 'text').toLowerCase()
      if (type === 'hidden') return null
      return INPUT_ROLES[type] ?? (el.hasAttribute('list') ? 'combobox' : 'textbox')
    }
    case 'img':
      return el.getAttribute('alt') === '' ? null : 'img'
    case 'h1':
    case 'h2':
    case 'h3':
    case 'h4':
    case 'h5':
    case 'h6':
      return 'heading'
    case 'dialog':
      return 'dialog'
    case 'option':
      return 'option'
    case 'li':
      return 'listitem'
    case 'progress':
      return 'progressbar'
    default:
      return null
  }
}

// The text an element's content names it with: text, and the names of the images and labelled
// elements inside it, skipping what is hidden from assistive technology.
function contentText(el: Element): string {
  const parts: string[] = []
  for (const node of Array.from(el.childNodes)) {
    if (node.nodeType === Node.TEXT_NODE) {
      parts.push(node.textContent ?? '')
    } else if (node instanceof Element) {
      if (node.getAttribute('aria-hidden') === 'true' || node.hasAttribute('hidden')) continue
      const label = node.getAttribute('aria-label')
      if (label) parts.push(label)
      else if (node instanceof HTMLImageElement) parts.push(node.alt)
      else parts.push(contentText(node))
    }
  }
  return normalize(parts.join(' '))
}

export function nameOf(el: Element, role: string): string {
  const labelledBy = el.getAttribute('aria-labelledby')
  if (labelledBy) {
    const text = normalize(
      labelledBy
        .split(/\s+/)
        .map((id) => el.ownerDocument.getElementById(id)?.textContent ?? '')
        .join(' ')
    )
    if (text) return text
  }
  const label = normalize(el.getAttribute('aria-label') ?? '')
  if (label) return label
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement || el instanceof HTMLSelectElement) {
    const labels = normalize(Array.from(el.labels ?? []).map((l) => l.textContent ?? '').join(' '))
    if (labels) return labels
    if (el instanceof HTMLInputElement && ['button', 'submit', 'reset'].includes(el.type)) {
      return normalize(el.value) || (el.type === 'submit' ? 'Submit' : el.type === 'reset' ? 'Reset' : '')
    }
    if (el instanceof HTMLInputElement && el.type === 'image') return normalize(el.alt)
    const placeholder = normalize(el.getAttribute('placeholder') ?? '')
    if (placeholder) return placeholder
  }
  if (el instanceof HTMLImageElement) return normalize(el.alt)
  if (NAME_FROM_CONTENT.has(role)) {
    const text = contentText(el)
    if (text) return text
  }
  return normalize(el.getAttribute('title') ?? '')
}

// Visible as Playwright counts it: a non-empty box, not display:none or visibility:hidden, and
// not hidden from assistive technology. Opacity does not count, as it does not for Playwright.
function isVisible(el: Element): boolean {
  if (el.closest('[aria-hidden="true"]')) return false
  const check = (el as Element & { checkVisibility?: (options?: object) => boolean }).checkVisibility
  if (typeof check === 'function') {
    if (!check.call(el, { visibilityProperty: true })) return false
  } else {
    for (let e: Element | null = el; e; e = e.parentElement) {
      if (getComputedStyle(e).display === 'none') return false
    }
    const visibility = getComputedStyle(el).visibility
    if (visibility === 'hidden' || visibility === 'collapse') return false
  }
  const box = el.getBoundingClientRect()
  return box.width > 0 && box.height > 0
}

function isDisabled(el: Element): boolean {
  return el.matches(':disabled') || el.closest('[aria-disabled="true"]') != null
}

export function findControls(selector: Selector, root: ParentNode = document): Element[] {
  const role = selector.role?.toLowerCase()
  const name = selector.name != null ? normalize(selector.name).toLowerCase() : null
  const matches: Element[] = []
  for (const el of Array.from(root.querySelectorAll('*'))) {
    const elRole = roleOf(el)
    if (elRole == null || (role != null && elRole !== role) || !isVisible(el)) continue
    if (name != null) {
      const elName = nameOf(el, elRole).toLowerCase()
      if (selector.exact === false ? !elName.includes(name) : elName !== name) continue
    }
    matches.push(el)
  }
  return matches
}

// Describes an element findControls returned, so a visible one.
function describe(el: Element): Control {
  const role = roleOf(el) ?? ''
  const box = el.getBoundingClientRect()
  const disabled = isDisabled(el)
  const control: Control = {
    found: true,
    role,
    name: nameOf(el, role),
    text: contentText(el),
    disabled,
    visible: true,
    actionable: false,
    x: box.left,
    y: box.top,
    width: box.width,
    height: box.height
  }
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement || el instanceof HTMLSelectElement) {
    control.value = el.value
  }
  if (el instanceof HTMLInputElement && (el.type === 'checkbox' || el.type === 'radio')) {
    control.checked = el.checked
  } else if (el.hasAttribute('aria-checked')) {
    control.checked = el.getAttribute('aria-checked') === 'true'
  } else if (el.hasAttribute('aria-pressed')) {
    control.checked = el.getAttribute('aria-pressed') === 'true'
  }
  const cx = box.left + box.width / 2
  const cy = box.top + box.height / 2
  if (disabled) control.reason = 'disabled'
  else if (cx < 0 || cy < 0 || cx >= window.innerWidth || cy >= window.innerHeight) control.reason = 'outside the viewport'
  else {
    const hit = document.elementFromPoint(cx, cy)
    if (hit != null && (hit === el || el.contains(hit))) control.actionable = true
    else control.reason = hit ? `covered by <${hit.tagName.toLowerCase()}>` : 'covered'
  }
  return control
}

function pick(selector: Selector, scroll: boolean): { el: Element; control: Control } | { found: false; error?: string } {
  const matches = findControls(selector)
  if (matches.length === 0) return { found: false }
  let el: Element | undefined
  if (selector.index != null) {
    el = matches[selector.index]
    if (el == null) return { found: false, error: `${matches.length} controls match; index ${selector.index} is past them` }
  } else if (matches.length > 1) {
    return { found: false, error: `${matches.length} controls match; pass an index` }
  } else {
    el = matches[0]
  }
  // A control clipped by a scrolling panel reads as covered, so bring it fully into view first:
  // a no-op when it already is. Instant, since smooth scrolling would be measured mid-scroll;
  // `describe` measures after.
  if (scroll) el.scrollIntoView?.({ block: 'nearest', inline: 'nearest', behavior: 'instant' })
  return { el, control: describe(el) }
}

export function describeControl(selector: Selector, { scroll = false } = {}): Description {
  const picked = pick(selector, scroll)
  return 'el' in picked ? picked.control : picked
}

export function listControls(role?: string): { role: string; name: string }[] {
  return findControls({ role })
    .map((el) => {
      const elRole = roleOf(el) ?? ''
      return { role: elRole, name: nameOf(el, elRole) }
    })
    .filter((control) => control.name !== '')
}

// --- waiting ---

export const WAIT_STATES = ['visible', 'hidden', 'enabled', 'actionable'] as const
export type WaitState = (typeof WAIT_STATES)[number]

/** How long /hud_wait waits unless told otherwise. The engine's console command
 *  (src/react_hud_cef/automation.rs) has the same default. */
export const DEFAULT_WAIT_TIMEOUT_MS = 10_000
const WAIT_POLL_MS = 100

type WaitCheck = { done: true; description: Description } | { done: false; why: string }

// Does the control meet the state now? Only a visible control is ever found, so `enabled` and
// `actionable` include visible. `actionable` scrolls the control into view as a click would.
function checkWait(selector: Selector, state: WaitState): WaitCheck {
  if (state === 'hidden') {
    const matches = findControls(selector)
    const shown = selector.index != null ? matches[selector.index] != null : matches.length > 0
    return shown ? { done: false, why: 'the control is still visible' } : { done: true, description: { found: false } }
  }
  const picked = pick(selector, state === 'actionable')
  if (!('el' in picked)) return { done: false, why: picked.error ?? 'no HUD control matches' }
  const { control } = picked
  if (state === 'enabled' && control.disabled) return { done: false, why: 'the control is disabled' }
  if (state === 'actionable' && !control.actionable) return { done: false, why: `the control can't be clicked: ${control.reason}` }
  return { done: true, description: control }
}

/**
 * Wait until the control a selector names is in a state, polling every 100 ms: `visible` (the
 * default: found), `hidden` (no match), `enabled` (found and not disabled) or `actionable` (a
 * click would land). Resolves with the control's description (`{found: false}` for `hidden`) and
 * rejects when `timeout` ms (default 10 s) pass first, saying what it last saw. Several matches
 * without an `index` count as not yet found, since a list may still be filling.
 */
export function waitForControl(options: CommandSelector): Promise<Description> {
  return new Promise((resolve, reject) => {
    const { name, role, index, exact } = options
    // a JSON null is no value, as for the engine's command
    const state = options.state ?? 'visible'
    const timeout = options.timeout ?? DEFAULT_WAIT_TIMEOUT_MS
    const selector: Selector = { name, role, index, exact }
    if (!(WAIT_STATES as readonly string[]).includes(state)) {
      return reject(new Error(`unknown state \`${state}\`: use ${WAIT_STATES.join(', ')}`))
    }
    if (typeof timeout !== 'number' || !Number.isFinite(timeout) || timeout < 0) {
      return reject(new Error('expected "timeout" to be a number of milliseconds'))
    }
    const deadline = Date.now() + timeout
    const poll = (): void => {
      let check: WaitCheck
      try {
        check = checkWait(selector, state as WaitState)
      } catch (e) {
        return reject(e)
      }
      if (check.done) return resolve(check.description)
      const left = deadline - Date.now()
      if (left <= 0) {
        const what = JSON.stringify(selector)
        return reject(new Error(`timed out after ${timeout} ms waiting for ${what} to be ${state}: ${check.why}`))
      }
      setTimeout(poll, Math.min(WAIT_POLL_MS, left))
    }
    poll()
  })
}

// --- trusted input: the browser's own, delivered by a test runner ---

/** How a command's input reached the page: from a test runner (`trusted`: the browser's own
 *  events, `isTrusted` true) or dispatched by this page as DOM events (`synthetic`). */
export type InputKind = 'trusted' | 'synthetic'

type MouseButtonName = 'left' | 'right' | 'middle'

/**
 * What the page asks the runner for, as `window.__dclTrustedInput(JSON.stringify({ id, ...request }))`.
 * A click's position is in CSS pixels of this frame's viewport. A key is named as the DOM names it,
 * with its Windows virtual key code and the text it types; without a `state` it is pressed and
 * released.
 */
export type TrustedInputRequest =
  | { op: 'click'; x: number; y: number; button: MouseButtonName }
  | { op: 'key'; code: string; key: string; keyCode: number; text?: string; location: number; state?: 'down' | 'up' }
  | { op: 'insertText'; text: string }

/** How long a command waits for the runner's answer before failing. */
export const TRUSTED_INPUT_TIMEOUT_MS = 10_000

type TrustedInputWindow = Window & {
  __dclTrustedInput?: (request: string) => void
  __dclTrustedInputDone?: (id: number, error: string | null) => void
}

/** Is a test runner driving the browser: has it added `window.__dclTrustedInput`? */
export function trustedInputAvailable(): boolean {
  return typeof (window as TrustedInputWindow).__dclTrustedInput === 'function'
}

const pendingInput = new Map<number, { resolve: () => void; reject: (error: Error) => void; timer: ReturnType<typeof setTimeout> }>()
let lastInputId = 0

// The runner answers each request by calling window.__dclTrustedInputDone(id, error), once its
// input has been delivered: `error` is a message, or null when it went well. An answer for a
// request the page has given up on is ignored.
function settleTrustedInput(id: number, error: string | null): void {
  const pending = pendingInput.get(id)
  if (!pending) return
  pendingInput.delete(id)
  clearTimeout(pending.timer)
  if (error == null) pending.resolve()
  else pending.reject(new Error(String(error)))
}

function requestTrustedInput(request: TrustedInputRequest): Promise<void> {
  return new Promise((resolve, reject) => {
    const id = ++lastInputId
    const timer = setTimeout(() => {
      pendingInput.delete(id)
      reject(new Error(`the test runner did not answer a trusted ${request.op} within ${TRUSTED_INPUT_TIMEOUT_MS} ms`))
    }, TRUSTED_INPUT_TIMEOUT_MS)
    pendingInput.set(id, { resolve, reject, timer })
    try {
      ;(window as TrustedInputWindow).__dclTrustedInput?.(JSON.stringify({ id, ...request }))
    } catch (e) {
      settleTrustedInput(id, e instanceof Error ? e.message : String(e))
    }
  })
}

// --- the web's input: trusted from a test runner, else DOM events as close to a user's as a page can make them ---

function clickable(selector: Selector): { el: Element; control: Control } {
  const picked = pick(selector, true)
  if (!('el' in picked)) throw new Error(picked.error ?? 'no HUD control matches')
  if (!picked.control.actionable) throw new Error(`the control can't be clicked: ${picked.control.reason}`)
  return picked
}

const centreOf = (control: Control): { x: number; y: number } => ({
  x: control.x + control.width / 2,
  y: control.y + control.height / 2
})

// `button` is MouseEvent.button and `buttons` the bit MouseEvent.buttons holds while it is down.
const MOUSE_BUTTONS: Record<MouseButtonName, { button: number; buttons: number }> = {
  left: { button: 0, buttons: 1 },
  right: { button: 2, buttons: 2 },
  middle: { button: 1, buttons: 4 }
}

// What ends the click depends on the button: `click` for left, `auxclick` for middle, `contextmenu` for right.
function dispatchClick(target: Element, x: number, y: number, which: MouseButtonName = 'left'): void {
  const { button, buttons } = MOUSE_BUTTONS[which]
  const mouse = { bubbles: true, cancelable: true, composed: true, clientX: x, clientY: y, button: 0 }
  const pointer = { ...mouse, pointerId: 1, pointerType: 'mouse', isPrimary: true }
  const Pointer = typeof PointerEvent === 'function' ? PointerEvent : MouseEvent
  target.dispatchEvent(new Pointer('pointerover', pointer))
  target.dispatchEvent(new Pointer('pointerenter', { ...pointer, bubbles: false }))
  target.dispatchEvent(new MouseEvent('mouseover', mouse))
  target.dispatchEvent(new MouseEvent('mouseenter', { ...mouse, bubbles: false }))
  target.dispatchEvent(new Pointer('pointermove', pointer))
  target.dispatchEvent(new MouseEvent('mousemove', mouse))
  target.dispatchEvent(new Pointer('pointerdown', { ...pointer, button, buttons }))
  target.dispatchEvent(new MouseEvent('mousedown', { ...mouse, button, buttons }))
  // a real mousedown focuses what it lands on; a dispatched one does not
  const focusable = target.closest('button, a[href], input, select, textarea, summary, [tabindex], [contenteditable="true"]')
  if (focusable instanceof HTMLElement) focusable.focus()
  target.dispatchEvent(new Pointer('pointerup', { ...pointer, button }))
  target.dispatchEvent(new MouseEvent('mouseup', { ...mouse, button }))
  const up = { ...mouse, button, detail: 1 }
  target.dispatchEvent(new MouseEvent(which === 'left' ? 'click' : which === 'middle' ? 'auxclick' : 'contextmenu', up))
}

export async function clickControl(selector: Selector): Promise<Control & { input: InputKind }> {
  const { el, control } = clickable(selector)
  const { x, y } = centreOf(control)
  if (trustedInputAvailable()) {
    await requestTrustedInput({ op: 'click', x, y, button: 'left' })
    return { ...control, input: 'trusted' }
  }
  dispatchClick(el, x, y)
  return { ...control, input: 'synthetic' }
}

// The input types that take typed text.
const TEXT_INPUT_TYPES = new Set(['text', 'search', 'email', 'url', 'tel', 'password', 'number'])

/** Why a control cannot be filled (a checkbox, a button, a read-only field), or null when it can. */
export function fillProblem(el: Element): string | null {
  let readOnly = el.getAttribute('aria-readonly') === 'true'
  if (el instanceof HTMLInputElement) {
    if (!TEXT_INPUT_TYPES.has(el.type)) return `the control is not a text field: it is an <input type="${el.type}">`
    readOnly ||= el.readOnly
  } else if (el instanceof HTMLTextAreaElement) {
    readOnly ||= el.readOnly
  } else if (!(el instanceof HTMLElement && el.isContentEditable)) {
    return `the control is not a text field: it is a <${el.tagName.toLowerCase()}>`
  }
  return readOnly ? 'the field is read-only' : null
}

// Select everything a field holds, so typing replaces it.
function selectContents(el: Element): void {
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
    el.select()
    return
  }
  const range = document.createRange()
  range.selectNodeContents(el)
  const selection = window.getSelection()
  selection?.removeAllRanges()
  selection?.addRange(range)
}

export async function fillControl(selector: Selector, text: string): Promise<Control & { input: InputKind }> {
  const { el, control } = clickable(selector)
  // before the click, which would toggle a checkbox it then refuses to fill
  const problem = fillProblem(el)
  if (problem) throw new Error(problem)
  const { x, y } = centreOf(control)
  if (trustedInputAvailable()) {
    await requestTrustedInput({ op: 'click', x, y, button: 'left' })
    // the click focuses the field, unless the page moved focus off it again
    if (el instanceof HTMLElement && !el.contains(document.activeElement)) el.focus()
    selectContents(el)
    if (text === '') await pressKey('Delete')
    else await requestTrustedInput({ op: 'insertText', text })
    // what the field holds, as the page may have changed what was typed
    const value = el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement ? el.value : (el.textContent ?? '')
    return { ...control, value, input: 'trusted' }
  }
  dispatchClick(el, x, y)
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
    // through the prototype's setter, so React's value tracking sees a change
    const proto = el instanceof HTMLInputElement ? HTMLInputElement.prototype : HTMLTextAreaElement.prototype
    const setValue = Object.getOwnPropertyDescriptor(proto, 'value')?.set
    if (setValue) setValue.call(el, text)
    else el.value = text
  } else {
    el.textContent = text
  }
  const Input = typeof InputEvent === 'function' ? InputEvent : Event
  el.dispatchEvent(new Input('input', { bubbles: true, composed: true, data: text, inputType: 'insertText' } as InputEventInit))
  el.dispatchEvent(new Event('change', { bubbles: true }))
  return { ...control, value: text, input: 'synthetic' }
}

/** What a key is to the browser: the DOM `key`, the Windows virtual key code a trusted key event
 *  carries, the text the key types (none for keys that type nothing, Tab included) and the
 *  DOM's `location` (0 standard, 1 left, 2 right). */
type KeyDefinition = { key: string; keyCode: number; text?: string; location: number }

const NAMED_KEYS: Record<string, KeyDefinition> = {
  Space: { key: ' ', keyCode: 32, text: ' ', location: 0 },
  Enter: { key: 'Enter', keyCode: 13, text: '\r', location: 0 },
  Tab: { key: 'Tab', keyCode: 9, location: 0 },
  Escape: { key: 'Escape', keyCode: 27, location: 0 },
  Backspace: { key: 'Backspace', keyCode: 8, location: 0 },
  Delete: { key: 'Delete', keyCode: 46, location: 0 },
  ArrowUp: { key: 'ArrowUp', keyCode: 38, location: 0 },
  ArrowDown: { key: 'ArrowDown', keyCode: 40, location: 0 },
  ArrowLeft: { key: 'ArrowLeft', keyCode: 37, location: 0 },
  ArrowRight: { key: 'ArrowRight', keyCode: 39, location: 0 },
  Home: { key: 'Home', keyCode: 36, location: 0 },
  End: { key: 'End', keyCode: 35, location: 0 },
  PageUp: { key: 'PageUp', keyCode: 33, location: 0 },
  PageDown: { key: 'PageDown', keyCode: 34, location: 0 },
  ShiftLeft: { key: 'Shift', keyCode: 16, location: 1 },
  ShiftRight: { key: 'Shift', keyCode: 16, location: 2 },
  ControlLeft: { key: 'Control', keyCode: 17, location: 1 },
  ControlRight: { key: 'Control', keyCode: 17, location: 2 },
  AltLeft: { key: 'Alt', keyCode: 18, location: 1 },
  AltRight: { key: 'Alt', keyCode: 18, location: 2 },
  Minus: { key: '-', keyCode: 189, text: '-', location: 0 },
  Equal: { key: '=', keyCode: 187, text: '=', location: 0 },
  Comma: { key: ',', keyCode: 188, text: ',', location: 0 },
  Period: { key: '.', keyCode: 190, text: '.', location: 0 },
  Slash: { key: '/', keyCode: 191, text: '/', location: 0 },
  Semicolon: { key: ';', keyCode: 186, text: ';', location: 0 },
  Quote: { key: "'", keyCode: 222, text: "'", location: 0 },
  Backquote: { key: '`', keyCode: 192, text: '`', location: 0 },
  BracketLeft: { key: '[', keyCode: 219, text: '[', location: 0 },
  BracketRight: { key: ']', keyCode: 221, text: ']', location: 0 },
  Backslash: { key: '\\', keyCode: 220, text: '\\', location: 0 }
}

/** A DOM `KeyboardEvent.code` name -> its definition on an unshifted US layout, for the codes the
 *  engine knows; null for any other. */
export function keyDefinition(code: string): KeyDefinition | null {
  const letter = /^Key([A-Z])$/.exec(code)
  if (letter) {
    const text = letter[1].toLowerCase()
    return { key: text, keyCode: letter[1].charCodeAt(0), text, location: 0 }
  }
  const digit = /^Digit([0-9])$/.exec(code)
  if (digit) return { key: digit[1], keyCode: digit[1].charCodeAt(0), text: digit[1], location: 0 }
  const fn = /^F([1-9]|1[0-2])$/.exec(code)
  if (fn) return { key: code, keyCode: 111 + Number(fn[1]), location: 0 }
  return Object.hasOwn(NAMED_KEYS, code) ? { ...NAMED_KEYS[code] } : null
}

/** A DOM `KeyboardEvent.code` name -> the `key` an unshifted US layout gives, as the engine maps it. */
export function keyFromCode(code: string): string | null {
  return keyDefinition(code)?.key ?? null
}

export async function pressKey(code: string, state?: 'down' | 'up'): Promise<InputKind> {
  const definition = keyDefinition(code)
  if (definition == null) throw new Error(`unknown key code \`${code}\``)
  if (trustedInputAvailable()) {
    await requestTrustedInput({ op: 'key', code, ...definition, state })
    return 'trusted'
  }
  const active = document.activeElement
  const target = active != null && active !== document.body ? active : document.body
  const init = { key: definition.key, code, bubbles: true, cancelable: true, composed: true }
  if (state !== 'up') target.dispatchEvent(new KeyboardEvent('keydown', init))
  if (state !== 'down') target.dispatchEvent(new KeyboardEvent('keyup', init))
  return 'synthetic'
}

export async function clickAt(x: number, y: number, button: MouseButtonName = 'left'): Promise<InputKind> {
  // a trusted click lands on whatever is there, as a user's does (the world's canvas, say): no check
  if (trustedInputAvailable()) {
    await requestTrustedInput({ op: 'click', x, y, button })
    return 'trusted'
  }
  const target = document.elementFromPoint(x, y)
  if (target == null) throw new Error(`nothing at (${x}, ${y})`)
  dispatchClick(target, x, y, button)
  return 'synthetic'
}

// --- the commands ---

/** The text after a `/hud_*` command: a JSON selector object, or just an accessible name, bare or
 *  as a JSON string (what `window.engine.hudClick('Backpack')` sends). */
export function parseSelector(rest: string): CommandSelector {
  const text = rest.trim()
  try {
    const parsed: unknown = JSON.parse(text)
    if (typeof parsed === 'string') return { name: parsed }
    if (parsed != null && typeof parsed === 'object' && !Array.isArray(parsed)) return parsed as CommandSelector
  } catch {
    /* not JSON: a name */
  }
  return { name: text }
}

interface HudRequestBase {
  id: number
  selector: CommandSelector
}

/**
 * Native: the engine's /hud_* question. `locate` (a click to come) and `fill` (a fill to come, which
 * also refuses a field that takes no text) scroll the control into view; `wait` is answered later,
 * by answerHudWait, once its state holds.
 */
export type HudRequest = (HudRequestBase & { op: 'list' | 'describe' | 'locate' | 'fill' }) | (HudRequestBase & { op: 'wait' })

export function answerHudRequest(request: HudRequest & { op: Exclude<HudRequest['op'], 'wait'> }): Description | { role: string; name: string }[] {
  try {
    if (request.op === 'list') return listControls(request.selector?.role ?? undefined)
    if (request.op === 'fill') {
      const picked = pick(request.selector ?? {}, true)
      if (!('el' in picked)) return picked
      // an unclickable control is reported as such first, as on the web
      const problem = picked.control.actionable ? fillProblem(picked.el) : null
      return problem ? { found: false, error: problem } : picked.control
    }
    return describeControl(request.selector ?? {}, { scroll: request.op === 'locate' })
  } catch (e) {
    return { found: false, error: e instanceof Error ? e.message : String(e) }
  }
}

/** Native: the reply to a `wait` request, once the control is in its state or the time is up. */
export function answerHudWait(request: HudRequest): Promise<{ result: Description } | { error: string }> {
  return waitForControl(request.selector ?? {}).then(
    (result) => ({ result }),
    (e: unknown) => ({ error: e instanceof Error ? e.message : String(e) })
  )
}

interface PageCommandArg {
  name: string
  kind: 'string' | 'json'
  optional: boolean
  help?: string
}

export interface PageCommand {
  args: PageCommandArg[]
  help: string
  /** a Promise for a command that takes time; a throw or rejection fails the command */
  run: (rest: string) => string | Promise<string>
}

const SELECTOR_ARG: PageCommandArg = { name: 'json', kind: 'json', optional: false, help: 'the accessible name, or a JSON selector' }

// The same commands and help as the engine registers natively (src/automation.rs,
// src/react_hud_cef/automation.rs).
export const PAGE_COMMANDS: Record<string, PageCommand> = {
  '/hud_list': {
    args: [{ name: 'role', kind: 'string', optional: true, help: 'only controls with this role' }],
    help: 'List the HUD\'s visible named controls as JSON ([{"role", "name"}]), optionally of one role',
    run: (rest) => JSON.stringify(listControls(rest.trim() || undefined))
  },
  '/hud_read': {
    args: [SELECTOR_ARG],
    help: 'Describe the HUD control a selector names, as JSON ({"found": false} when none does). The selector is the control\'s accessible name, or JSON {"name", "role", "index", "exact"}',
    run: (rest) => JSON.stringify(describeControl(parseSelector(rest)))
  },
  '/hud_wait': {
    args: [{ name: 'json', kind: 'json', optional: false, help: 'the accessible name, or a JSON selector with "state" and "timeout"' }],
    help: 'Wait until the HUD control a selector names is in a state, then describe it as JSON ({"found": false} for hidden). The selector is as /hud_read takes it, plus "state" (visible, hidden, enabled or actionable; default visible) and "timeout" in ms (default 10000); fails if the timeout passes first',
    run: async (rest) => JSON.stringify(await waitForControl(parseSelector(rest)))
  },
  '/hud_click': {
    args: [SELECTOR_ARG],
    help: 'Click the HUD control a selector names, at its centre, if it is visible, enabled and not covered. The selector is as /hud_read takes it. The reply\'s "input" is "trusted" for a real click, delivered by the test runner driving the browser, and "synthetic" for DOM events',
    run: async (rest) => JSON.stringify(await clickControl(parseSelector(rest)))
  },
  '/hud_fill': {
    args: [{ name: 'json', kind: 'json', optional: false, help: 'a JSON selector with the text to type' }],
    help: 'Type into the HUD field a selector names: click it and replace its text. JSON {"name", "role", "index", "exact", "text"}. The reply\'s "input" is "trusted" for real input from the test runner driving the browser, "synthetic" for DOM events',
    run: async (rest) => {
      const selector = parseSelector(rest)
      if (typeof selector.text !== 'string') throw new Error('expected JSON {"name": ..., "text": ...}')
      return JSON.stringify(await fillControl(selector, selector.text))
    }
  },
  '/press_key': {
    args: [{ name: 'code', kind: 'string', optional: false, help: "the key's code name" }],
    help: 'Press a key as the keyboard would: the world and the HUD both get it. The key is named as DOM KeyboardEvent.code names it (KeyB, Digit1, Space, Enter, Escape, ArrowUp, F1, ...). The web page asks the test runner driving the browser for a real key press, and dispatches DOM events without one',
    run: async (rest) => {
      const [code, flag, value] = rest.trim().split(/\s+/)
      const state = flag === '--state' ? value : undefined
      if (state != null && state !== 'down' && state !== 'up') throw new Error(`unknown state \`${state}\`: use down or up`)
      await pressKey(code ?? '', state as 'down' | 'up' | undefined)
      return `pressed ${code}`
    }
  },
  '/click_at': {
    args: [
      { name: 'x', kind: 'string', optional: false },
      { name: 'y', kind: 'string', optional: false }
    ],
    help: 'Click at a page position, in CSS pixels from the top left, as the mouse would: the HUD gets it where it is, the world elsewhere. --button is left (the default), right or middle. The web page asks the test runner driving the browser for a real click, and dispatches DOM events without one',
    run: async (rest) => {
      const [xText, yText, flag, value] = rest.trim().split(/\s+/)
      const x = Number(xText)
      const y = Number(yText)
      if (!Number.isFinite(x) || !Number.isFinite(y)) throw new Error('expected /click_at <x> <y>')
      const button = flag === '--button' ? value : undefined
      if (button != null && button !== 'left' && button !== 'right' && button !== 'middle') {
        throw new Error(`unknown button \`${button}\`: use left, right or middle`)
      }
      await clickAt(x, y, button)
      return `clicked at (${x}, ${y})`
    }
  }
}

/** Web: hand engine.js the commands it runs in the page instead of sending them to the engine, and
 *  the hook a test runner answers its trusted input requests through. */
export function installHudAutomation(): void {
  const w = window as Window & { __dclPageCommands?: Record<string, PageCommand> } & TrustedInputWindow
  w.__dclPageCommands = { ...(w.__dclPageCommands ?? {}), ...PAGE_COMMANDS }
  w.__dclTrustedInputDone = settleTrustedInput
}
