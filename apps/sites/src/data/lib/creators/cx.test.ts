import { describe, expect, it } from 'vitest'
import { cx } from './cx'

describe('cx', () => {
  it('joins the classes it is given, in order', () => {
    expect(cx('root', 'consumer')).toBe('root consumer')
  })

  it('skips the classes a condition turned off', () => {
    expect(cx('root', false, undefined, null, '', 'active')).toBe('root active')
  })

  it('gives an empty class list when nothing applies', () => {
    expect(cx(undefined, false)).toBe('')
  })
})
