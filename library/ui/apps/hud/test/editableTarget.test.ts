import { describe, expect, it } from 'vitest'
import { isCancelKey, isEditableTarget } from '../lib/bindingLabels'

// A focused text field takes the keys as typing; a focused switch or checkbox leaves them to the engine.
function element(html: string): HTMLElement {
  const host = document.createElement('div')
  host.innerHTML = html
  return host.firstElementChild as HTMLElement
}

describe('isEditableTarget', () => {
  it('is true for text fields, and for the other inputs the HUD has always treated as one', () => {
    for (const html of ['<input>', '<input type="text">', '<input type="search">', '<input type="email">', '<input type="range">', '<input type="radio">', '<textarea></textarea>']) {
      expect(isEditableTarget(element(html)), html).toBe(true)
    }
  })

  it('is false for a switch and a checkbox, which are inputs that take no text', () => {
    for (const html of ['<input type="checkbox" role="switch">', '<input type="checkbox">']) {
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

describe('isCancelKey with a switch focused', () => {
  it('lets a printable cancel key through, as it does outside a text field', () => {
    const target = element('<input type="checkbox" role="switch">')
    expect(isCancelKey({ key: 'Escape', code: 'Escape', target })).toBe(true)
  })
})
