import { describe, expect, it } from 'vitest'
import { formatParts } from './format'

const text = (message: string, values?: Parameters<typeof formatParts>[1]) => formatParts(message, values).join('')

describe('formatParts', () => {
  it('fills in named values', () => {
    expect(text('Hello {name}, you have {count} items', { name: 'Ana', count: 3 })).toBe('Hello Ana, you have 3 items')
  })

  it('picks the plural arm for the count and writes it where the message has #', () => {
    const message = '{count, plural, one {# item} other {# items}}'
    expect(text(message, { count: 1 })).toBe('1 item')
    expect(text(message, { count: 1200 })).toBe('1,200 items')
  })

  it('prefers an exact plural arm', () => {
    expect(text('{count, plural, =0 {None} one {One} other {Many}}', { count: 0 })).toBe('None')
  })

  it('picks the select arm for the value and falls back to other', () => {
    const message = '{variant, select, create {New Collection} other {Rename Collection}}'
    expect(text(message, { variant: 'create' })).toBe('New Collection')
    expect(text(message, { variant: 'edit' })).toBe('Rename Collection')
  })

  it('groups the digits of a number argument', () => {
    expect(text('{label} ({count, number})', { label: 'Rare', count: 5000 })).toBe('Rare (5,000)')
  })

  it('fills values inside a plural arm', () => {
    expect(text('{count, plural, one {# item from “{name}”} other {# items from “{name}”}}', { count: 2, name: 'Hats' })).toBe(
      '2 items from “Hats”'
    )
  })

  it('hands the content of a tag to its function', () => {
    const parts = formatParts('Drop files or <browse>browse your device</browse> to start.', {
      browse: (chunks: string[]) => `[${chunks.join('')}]`
    })
    expect(parts.join('')).toBe('Drop files or [browse your device] to start.')
  })

  it('keeps the content of a tag that has no function', () => {
    expect(text('Pay the <fee>publication fee</fee>.')).toBe('Pay the publication fee.')
  })
})
