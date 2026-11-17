import { createElement, Fragment } from 'react'
import { describe, it, expect } from 'vitest'
import { hasRenderableContent, contentLabel, contentFallback } from '../content.helpers'
describe('when common component content supplies accessible fallback names', () => {
  it.each([undefined, null, false, true, '', '  ', [false, ' ']])(
    'should replace absent visible content %s',
    (value) => {
      expect(contentFallback(value, 'Fallback')).toBe('Fallback')
    }
  )
  it('should preserve numeric zero and meaningful nested caller content', () => {
    expect(contentFallback(0, 'Fallback')).toBe(0)
    const content = [false, [0, 'Value']]
    expect(contentFallback(content, 'Fallback')).toBe(content)
  })
  it('should preserve the existing whitespace behavior unless opted in', () => {
    expect(hasRenderableContent(' ')).toBe(true)
    expect(contentFallback(' ', 'Fallback')).toBe('Fallback')
  })
  it.each([undefined, null, false, 0, '', ' '])(
    'should supply a readable string name for %s',
    (value) => {
      expect(contentLabel(value, 'Fallback')).toBe('Fallback')
    }
  )
  it('should preserve meaningful caller text', () => {
    expect(contentLabel('Named control', 'Fallback')).toBe('Named control')
  })
  it('should preserve legacy Array filter predicate semantics', () => {
    expect(['', ' ', false, 0, 'Text'].filter(hasRenderableContent)).toEqual([' ', 0, 'Text'])
  })
  it('should replace empty nested React fragments without consuming custom elements', () => {
    expect(contentFallback(createElement(Fragment, null, false), 'Fallback')).toBe('Fallback')
    expect(
      contentFallback(
        createElement(Fragment, null, ' ', createElement(Fragment, null, false)),
        'Fallback'
      )
    ).toBe('Fallback')
    const opaque = createElement('span', null, false)
    expect(contentFallback(opaque, 'Fallback')).toBe(opaque)
  })
})
