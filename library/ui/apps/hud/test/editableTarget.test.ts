import { afterEach, describe, expect, it } from 'vitest'
import { isCancelKey, isEditableTarget, setBindingsSnapshot } from '../lib/bindingLabels'

// A focused text field takes the keys as typing; a focused switch, checkbox or radio leaves them to the engine.
function element(html: string): HTMLElement {
  const host = document.createElement('div')
  host.innerHTML = html
  return host.firstElementChild as HTMLElement
}

describe('isEditableTarget', () => {
  it('is true for text fields, and for the other inputs the HUD has always treated as one', () => {
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
