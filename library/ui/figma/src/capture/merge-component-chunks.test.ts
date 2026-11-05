import { describe, expect, it } from 'vitest'
import { mergeChunks } from './merge-component-chunks.js'

const component = (id: string) => ({
  kind: 'component',
  id,
  name: `C${id}`,
  section: [],
  x: 0,
  y: 0,
  properties: {},
  base: { id, name: `C${id}`, props: {}, tree: { type: 'COMPONENT', w: 1, h: 1 } },
  others: []
})

const chunk = (pageId: string, ids: string[], labels?: unknown[]) => ({
  page: { id: pageId, name: 'Page' },
  labels,
  components: ids.map(component)
})

describe('mergeChunks', () => {
  describe('when chunks of the same page are merged', () => {
    it('should concatenate components in order and keep the labels of the first chunk', () => {
      const merged = mergeChunks([
        chunk('1:1', ['a', 'b'], [{ text: 'Title', x: 0, y: 0 }]),
        chunk('1:1', ['c'])
      ])

      expect(merged.components.map((entry) => entry.id)).toEqual(['a', 'b', 'c'])
      expect(merged.labels).toEqual([{ text: 'Title', x: 0, y: 0 }])
    })
  })

  describe('when a component appears twice', () => {
    it('should fail so a re-run does not duplicate it silently', () => {
      expect(() => mergeChunks([chunk('1:1', ['a']), chunk('1:1', ['a'])])).toThrow(
        /more than one chunk/
      )
    })
  })

  describe('when chunks belong to different pages', () => {
    it('should refuse to merge them', () => {
      expect(() => mergeChunks([chunk('1:1', ['a']), chunk('2:2', ['b'])])).toThrow(
        /cannot be merged/
      )
    })
  })

  describe('when a chunk does not match the snapshot schema', () => {
    it('should fail with a validation error', () => {
      expect(() =>
        mergeChunks([{ page: { id: '1', name: 'P' }, components: [{ id: 'x' }] }])
      ).toThrow()
    })
  })
})
