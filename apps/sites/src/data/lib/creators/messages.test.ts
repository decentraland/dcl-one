import { describe, expect, it } from 'vitest'
import { flattenMessages } from './messages'
import en from '@/data/lib/creators/intl/en.json'
import es from '@/data/lib/creators/intl/es.json'
import zh from '@/data/lib/creators/intl/zh.json'

describe('flattenMessages', () => {
  it('flattens nested objects into dotted ids', () => {
    expect(
      flattenMessages({
        page: { title: 'Title', empty: { description: 'Nothing here' } },
        standalone: 'Alone'
      })
    ).toEqual({
      'page.title': 'Title',
      'page.empty.description': 'Nothing here',
      standalone: 'Alone'
    })
  })

  it('returns an empty map for an empty object', () => {
    expect(flattenMessages({})).toEqual({})
  })

  it.each([
    ['es', es],
    ['zh', zh]
  ])('keeps the %s key set in sync with en', (_, messages) => {
    expect(Object.keys(flattenMessages(messages)).sort()).toEqual(Object.keys(flattenMessages(en)).sort())
  })
})
