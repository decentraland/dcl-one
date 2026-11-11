import { describe, expect, it } from 'vitest'
import { blogHighlightRuns } from './blogHighlight'

describe('blogHighlightRuns', () => {
  it('marks the whole word a prefix matches, ignoring case', () => {
    expect(blogHighlightRuns('Spot Scams & Stay Safe in Decentraland', 'decen')).toEqual([
      { text: 'Spot Scams & Stay Safe in ', match: false },
      { text: 'Decentraland', match: true }
    ])
  })

  it('marks every word of a multi-word query, keeping the text between them', () => {
    expect(blogHighlightRuns('Genesis Plaza relaunched', 'plaza GEN')).toEqual([
      { text: 'Genesis', match: true },
      { text: ' ', match: false },
      { text: 'Plaza', match: true },
      { text: ' relaunched', match: false }
    ])
  })

  it('does not mark a match inside a word', () => {
    expect(blogHighlightRuns('Regenesis Labs', 'genesis')).toEqual([
      { text: 'Regenesis Labs', match: false }
    ])
  })

  it('ignores punctuation in the query and handles accented words', () => {
    expect(blogHighlightRuns('Música en vivo', '"mús"!')).toEqual([
      { text: 'Música', match: true },
      { text: ' en vivo', match: false }
    ])
  })

  it('leaves the text unmarked for a blank query, and returns nothing for empty text', () => {
    expect(blogHighlightRuns('Decentraland', '  ')).toEqual([
      { text: 'Decentraland', match: false }
    ])
    expect(blogHighlightRuns('', 'decen')).toEqual([])
  })
})
