import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { describe, expect, it } from 'vitest'
import { rawComponentPageSchema, type RawNode } from '../ir/raw-components.js'
import { snapshotDir } from '../../test/snapshot.js'
import { applyPatch, expandVariants } from './expand-variants.js'

const leaf = (name: string): RawNode => ({ type: 'RECTANGLE', name, w: 1, h: 1 })

describe('expandVariants', () => {
  describe('when a patch sets and unsets properties of a nested node', () => {
    it('should change only the targeted node and leave the base untouched', () => {
      const base: RawNode = {
        type: 'COMPONENT',
        w: 10,
        h: 10,
        children: [{ ...leaf('a'), fillStyle: 'old' }, leaf('b')]
      }

      const patched = applyPatch(structuredClone(base), {
        at: [0],
        set: { w: 5 },
        unset: ['fillStyle']
      })

      expect(patched.children?.[0]).toEqual({ type: 'RECTANGLE', name: 'a', w: 5, h: 1 })
      expect(base.children?.[0]?.fillStyle).toBe('old')
    })
  })

  describe('when a patch replaces the root', () => {
    it('should return the replacement tree', () => {
      const base: RawNode = { type: 'COMPONENT', w: 1, h: 1 }

      const patched = applyPatch(base, { at: [], replace: { type: 'COMPONENT', w: 9, h: 9 } })

      expect(patched.w).toBe(9)
    })
  })

  describe('when a patch points outside the tree', () => {
    it('should fail loudly instead of corrupting the variant', () => {
      const base: RawNode = { type: 'COMPONENT', w: 1, h: 1, children: [] }

      expect(() => applyPatch(base, { at: [3], set: { w: 2 } })).toThrow(/does not exist/)
    })
  })

  describe('when expanding every component of the captured Buttons page', () => {
    it('should return one tree per Figma variant', async () => {
      const raw = await readFile(path.join(snapshotDir, 'components/Buttons.json'), 'utf8')
      const page = rawComponentPageSchema.parse(JSON.parse(raw))

      const friend = page.components.find((component) => component.name === 'FriendCTA')!
      const variants = expandVariants(friend)

      expect(variants).toHaveLength(15)
      const hover = variants.find((variant) => variant.name === 'Property 1=AddFriendHover')!
      expect(hover.tree.fills?.[0]).toMatchObject({ token: 'Primitives/Primary/Primary Dark 1' })
      expect(hover.tree.children).toHaveLength(2)
    })
  })
})
