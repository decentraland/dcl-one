import { afterEach, describe, expect, it } from 'vitest'
import { isCancelKey, isEditableTarget, isSliderTarget, setBindingsSnapshot } from '../lib/bindingLabels'
import { SLIDER_KEYS } from './sliderKeys'

// A focused text field takes the keys as typing and a focused slider adjusts itself with them; a focused switch,
// checkbox or radio leaves them to the engine. A slider types nothing, so its cancel key cancels.
function element(html: string): HTMLElement {
  const host = document.createElement('div')
  host.innerHTML = html
  return host.firstElementChild as HTMLElement
}

describe('isEditableTarget', () => {
  it('is true for text fields and for a slider, which keep the keys from the engine', () => {
    for (const html of ['<input>', '<input type="text">', '<input type="search">', '<input type="email">', '<input type="range">', '<textarea></textarea>']) {
      expect(isEditableTarget(element(html)), html).toBe(true)
    }
  })

  it('is false for a switch, a checkbox and a radio, which are inputs that take no text', () => {
    for (const html of ['<input type="checkbox" role="switch">', '<input type="checkbox">', '<input type="radio">', '<input type="RADIO">']) {
      expect(isEditableTarget(element(html)), html).toBe(false)
    }
  })

  it('is false for a button, another element and nothing', () => {
    expect(isEditableTarget(element('<button></button>'))).toBe(false)
    expect(isEditableTarget(element('<div></div>'))).toBe(false)
    expect(isEditableTarget(null)).toBe(false)
    expect(isEditableTarget(undefined)).toBe(false)
  })
})

describe('isSliderTarget', () => {
  it('is true only for a range input', () => {
    expect(isSliderTarget(element('<input type="range">'))).toBe(true)
    for (const html of ['<input>', '<input type="search">', '<input type="checkbox">', '<input type="radio">', '<textarea></textarea>', '<div role="slider" tabindex="0"></div>', '<button></button>']) {
      expect(isSliderTarget(element(html)), html).toBe(false)
    }
    expect(isSliderTarget(null)).toBe(false)
    expect(isSliderTarget(undefined)).toBe(false)
  })
})

describe('isCancelKey with a switch or a radio focused', () => {
  afterEach(() => setBindingsSnapshot([])) // module store leaks across tests

  it('lets Escape through', () => {
    for (const html of ['<input type="checkbox" role="switch">', '<input type="radio">']) {
      expect(isCancelKey({ key: 'Escape', code: 'Escape', target: element(html) }), html).toBe(true)
    }
  })

  it('lets a printable cancel key through, as it does outside a text field, while a text field types it', () => {
    setBindingsSnapshot([[{ System: 'Cancel' }, ['KeyR']]])
    for (const html of ['<input type="checkbox" role="switch">', '<input type="radio">']) {
      expect(isCancelKey({ key: 'r', code: 'KeyR', target: element(html) }), html).toBe(true)
    }
    expect(isCancelKey({ key: 'r', code: 'KeyR', target: element('<input type="search">') })).toBe(false)
  })
})

describe('isCancelKey with a slider focused', () => {
  afterEach(() => setBindingsSnapshot([])) // module store leaks across tests

  it('lets Escape through, and none of the keys that adjust the slider', () => {
    const slider = element('<input type="range">')
    expect(isCancelKey({ key: 'Escape', code: 'Escape', target: slider })).toBe(true)
    for (const code of SLIDER_KEYS) {
      expect(isCancelKey({ key: code, code, target: slider }), code).toBe(false)
    }
  })

  it('lets a printable cancel key through: a slider types nothing', () => {
    setBindingsSnapshot([[{ System: 'Cancel' }, ['KeyR']]])
    const slider = element('<input type="range">')
    expect(isCancelKey({ key: 'r', code: 'KeyR', target: slider })).toBe(true)
    // the rebind moved Cancel off Escape, from a slider too
    expect(isCancelKey({ key: 'Escape', code: 'Escape', target: slider })).toBe(false)
  })

  it('lets nothing through while Cancel is unbound', () => {
    setBindingsSnapshot([[{ System: 'Cancel' }, []], [{ System: 'Map' }, ['KeyM']]])
    expect(isCancelKey({ key: 'Escape', code: 'Escape', target: element('<input type="range">') })).toBe(false)
  })
})
