import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { type Card, searchPages } from '../specs/backpack.ts'

const page = (...names: string[]): Card[] => names.map((name) => ({ name, selector: { name, role: 'button' } }))

/** A pager over `pages` that wraps from the last page to the first, as the HUD's does. */
function pager(pages: Card[][]): { next: () => Promise<Card[]> } {
  let at = 0
  return {
    next: async () => {
      at = (at + 1) % pages.length
      return pages[at]!
    }
  }
}

describe('searchPages', () => {
  test('stops on the page where the scan finds what it looks for, and says which', async () => {
    const pages = [page('a', 'b'), page('c', 'd'), page('e')]
    const { next } = pager(pages)
    const result = await searchPages(pages[0]!, next, async (cards) => cards.find((card) => card.name === 'd')?.name)
    assert.deepEqual(result, { found: 'd', page: 2 })
  })

  test('ends after every page of a wrapping pager when nothing is found', async () => {
    const pages = [page('a', 'b'), page('c', 'd'), page('e')]
    const { next } = pager(pages)
    const scanned: string[] = []
    const result = await searchPages(pages[0]!, next, async (cards) => {
      scanned.push(cards[0]!.name)
      return undefined
    })
    assert.equal(result, undefined)
    assert.deepEqual(scanned, ['a', 'c', 'e'])
  })

  test('scans a grid with no other page once', async () => {
    const only = page('a', 'b')
    let scans = 0
    const result = await searchPages(only, async () => only, async () => {
      scans += 1
      return undefined
    })
    assert.equal(result, undefined)
    assert.equal(scans, 1)
  })
})
