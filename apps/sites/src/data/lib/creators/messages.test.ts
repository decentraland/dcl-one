// @vitest-environment jsdom
import './test/setup'
import { describe, expect, it } from 'vitest'
import { flattenMessages } from './messages'

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

})
