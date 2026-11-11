import { cp, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { applyRetirements, pendingRetirements, planRetirements } from './apply-retirements.js'
import type { Retirements } from './retirements.js'
import { GEN, PLACES, SNAP, writeTree } from './test-fixtures.js'

const placeFiles = [
  `${GEN}/components/places/PlaceCard.tsx`,
  `${SNAP}/components/Places.json`,
  'components/places/PlaceCard.tsx'
]
// The capture cache is pinned input: the record names its page there as identity, and nothing removes it.
const placeOutputs = placeFiles.filter((file) => !file.startsWith(`${SNAP}/`))
const records = (
  owned: string[],
  extra: Partial<Retirements['retired'][number]> = {}
): Retirements => ({
  schema: 1,
  retired: [
    {
      pageId: PLACES,
      name: 'Places',
      reason: 'gone',
      owned,
      sharedExclusions: [],
      ...extra
    }
  ]
})

describe('apply retirements', () => {
  let root: string
  beforeEach(async () => {
    root = await mkdtemp(path.join(os.tmpdir(), 'dcl-retire-'))
    await writeTree(root)
  })
  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })

  const exists = (file: string) =>
    readFile(path.join(root, file)).then(
      () => true,
      () => false
    )

  describe('when the record owns its page artifacts', () => {
    it('should delete only those files and leave the capture cache untouched', async () => {
      const capture = await readFile(path.join(root, `${SNAP}/components/Places.json`))
      const deleted = await applyRetirements(root, records(placeFiles))

      expect(deleted.sort()).toEqual([...placeOutputs].sort())
      expect(await exists(placeFiles[0]!)).toBe(false)
      expect(await readFile(path.join(root, `${SNAP}/components/Places.json`))).toEqual(capture)
      expect(await exists(`${GEN}/components/buttons/Button.tsx`)).toBe(true)
    })

    it('should be a no-op the second time', async () => {
      await applyRetirements(root, records(placeFiles))

      expect(await applyRetirements(root, records(placeFiles))).toEqual([])
    })

    it('should report the files as pending until they are applied', async () => {
      expect((await pendingRetirements(root, records(placeFiles))).sort()).toEqual(
        [...placeOutputs].sort()
      )

      await applyRetirements(root, records(placeFiles))

      expect(await pendingRetirements(root, records(placeFiles))).toEqual([])
    })

    it('should work from a clean copy of the checkout', async () => {
      const copy = await mkdtemp(path.join(os.tmpdir(), 'dcl-retire-copy-'))
      try {
        await cp(root, copy, { recursive: true })

        expect(await applyRetirements(copy, records(placeFiles))).toHaveLength(placeOutputs.length)
      } finally {
        await rm(copy, { recursive: true, force: true })
      }
    })
  })

  describe('when a record claims something it does not own', () => {
    it('should refuse and delete nothing', async () => {
      const wrong = [placeFiles[0]!, `${GEN}/components/buttons/Button.tsx`]

      await expect(applyRetirements(root, records(wrong))).rejects.toThrow(
        /not owned|owned by page/
      )
      expect(await exists(placeFiles[0]!)).toBe(true)
      expect(await exists(wrong[1]!)).toBe(true)
    })

    it('should refuse a hand-written path outside every guarded owner', async () => {
      const plan = await planRetirements(root, records(['components/Modal/Modal.tsx']))

      expect(plan.delete).toEqual([])
      expect(plan.rejected).toHaveLength(1)
    })
  })

  describe('when an artifact is shared with live items', () => {
    it('should never delete it', async () => {
      const plan = await planRetirements(
        root,
        records(placeFiles, { sharedExclusions: [placeFiles[0]!] })
      )

      expect(plan.delete).not.toContain(placeFiles[0])
      expect(plan.rejected.map((item) => item.reason)).toContain('listed as shared with live items')
    })
  })

  describe('when only one component of a page is retired', () => {
    it('should delete that component and keep the rest of the page', async () => {
      const plan = await planRetirements(root, records([placeFiles[0]!], { componentId: 'p1' }))

      expect(plan.rejected).toEqual([])
      expect(plan.delete).toEqual([placeFiles[0]])
    })

    it('should reject a component that is not on the retired page', async () => {
      const plan = await planRetirements(root, records([placeFiles[0]!], { componentId: 'b1' }))

      expect(plan.rejected[0]?.reason).toContain('not owned')
    })
  })

  describe('when the record also owns library exports and the promotion entry', () => {
    it('should remove only those lines and keep every other export', async () => {
      const owned = [
        'index.ts#value:PlaceCard',
        'index.ts#type:PlaceCardProps',
        'components/promotion.json#component:PlaceCard'
      ]

      await applyRetirements(root, records(owned))

      const index = await readFile(path.join(root, 'index.ts'), 'utf8')
      expect(index).not.toContain('PlaceCard')
      expect(index).toContain("export { Button } from './components/buttons/Button'")
      expect(index).toContain('export type { ButtonProps } from')
      expect(index).toContain("export { Modal } from './Modal/Modal'")
      expect(
        JSON.parse(await readFile(path.join(root, 'components/promotion.json'), 'utf8')).components
      ).toEqual(['Button'])
    })

    it('should remove a promotion entry that carries accepted limits', async () => {
      const limits = [{ id: 'pending-contracts', text: 'Contracts remain follow-ups.' }]
      const promotion = path.join(root, 'components/promotion.json')
      const registry = JSON.parse(await readFile(promotion, 'utf8'))
      await writeFile(
        promotion,
        JSON.stringify({ ...registry, components: ['Button', { name: 'PlaceCard', limits }] })
      )

      await applyRetirements(root, records(['components/promotion.json#component:PlaceCard']))

      expect(JSON.parse(await readFile(promotion, 'utf8')).components).toEqual(['Button'])
      expect(
        await pendingRetirements(root, records(['components/promotion.json#component:PlaceCard']))
      ).toEqual([])
    })

    it('should reject an unverified export and preserve its shared statement', async () => {
      const file = path.join(root, 'index.ts')
      await writeFile(file, "export { PlaceCard, Other } from './x'\n")

      await expect(applyRetirements(root, records(['index.ts#value:PlaceCard']))).rejects.toThrow(
        'not owned'
      )

      expect(await readFile(file, 'utf8')).toContain('PlaceCard, Other')
    })
  })
})
