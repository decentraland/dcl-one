import { describe, expect, it } from 'vitest'
import { blogHighlightRuns } from './blogHighlight'

describe('blogHighlightRuns', () => {
  describe('when the query is a prefix of a word in another case', () => {
    it('should mark the whole word the prefix matches', () => {
      expect(blogHighlightRuns('Spot Scams & Stay Safe in Decentraland', 'decen')).toEqual([
        { text: 'Spot Scams & Stay Safe in ', match: false },
        { text: 'Decentraland', match: true }
      ])
    })
  })

  describe('when the query has several words', () => {
    it('should mark every word of the query, keeping the text between them', () => {
      expect(blogHighlightRuns('Genesis Plaza relaunched', 'plaza GEN')).toEqual([
        { text: 'Genesis', match: true },
        { text: ' ', match: false },
        { text: 'Plaza', match: true },
        { text: ' relaunched', match: false }
      ])
    })
  })

  describe('when the query only matches inside a word', () => {
    it('should not mark the match', () => {
      expect(blogHighlightRuns('Regenesis Labs', 'genesis')).toEqual([
        { text: 'Regenesis Labs', match: false }
      ])
    })
  })

  describe('when the query has punctuation and the text has accented words', () => {
    it('should ignore the punctuation and mark the accented word', () => {
      expect(blogHighlightRuns('Música en vivo', '"mús"!')).toEqual([
        { text: 'Música', match: true },
        { text: ' en vivo', match: false }
      ])
    })
  })

  describe('when the query is blank', () => {
    it('should leave the text unmarked', () => {
      expect(blogHighlightRuns('Decentraland', '  ')).toEqual([
        { text: 'Decentraland', match: false }
      ])
    })
  })

  describe('when the text is empty', () => {
    it('should return no runs', () => {
      expect(blogHighlightRuns('', 'decen')).toEqual([])
    })
  })
})
