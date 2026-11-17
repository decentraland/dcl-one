import { describe, expect, it } from 'vitest'
import { hasRenderableContent } from '../content.helpers'

describe('when checking optional native component content', () => {
  it.each([null, undefined, false, true, '', [], [null, false, ['']]])(
    'should omit empty content %j',
    (content) => expect(hasRenderableContent(content)).toBe(false)
  )
  it('should retain zero and nested present content', () => {
    expect(hasRenderableContent(0)).toBe(true)
    expect(hasRenderableContent([null, [false, 0]])).toBe(true)
    expect(hasRenderableContent([null, ['Label']])).toBe(true)
  })
  it('should leave element rendering to React', () => {
    expect(hasRenderableContent(<span>Label</span>)).toBe(true)
  })
})
