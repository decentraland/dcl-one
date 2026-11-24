import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import path from 'node:path'
import os from 'node:os'
import { createApp } from '../app.js'
import { SnapshotSource } from './snapshot-source.js'
import { snapshotSource } from '../../test/snapshot.js'
import { encodePageFileName } from '../capture/page-file-name.js'

describe('SnapshotSource', () => {
  describe('when reading the synthetic snapshot', () => {
    let source: SnapshotSource

    beforeEach(() => {
      source = snapshotSource()
    })

    it('should expose the four variable collections with 10, 5, 8 and 5 variables', async () => {
      const collections = await source.getTokenCollections()

      expect(collections.map((c) => [c.name, c.variables.length])).toEqual([
        ['Primitives', 10],
        ['Semantic', 5],
        ['Spacing', 8],
        ['Layout', 5]
      ])
    })

    it('should keep the four layout modes', async () => {
      const layout = (await source.getTokenCollections()).find((c) => c.name === 'Layout')

      expect(layout?.modes).toEqual(['desktop', 'large-desktop', 'tablet', 'mobile'])
    })

    it('should expose 7 text styles', async () => {
      expect(await source.getTextStyles()).toHaveLength(7)
    })

    it('should expose 2 gradients', async () => {
      expect(await source.getGradients()).toHaveLength(2)
    })

    it('should load every icon', async () => {
      expect(await source.getIcons()).toHaveLength(26)
    })

    it('should load each icon with its SVG markup', async () => {
      const icons = await source.getIcons()

      expect(icons.every((icon) => icon.svg.startsWith('<svg'))).toBe(true)
    })

    it('should read a captured component page by its name', async () => {
      const page = await source.getComponentPage('Demo Controls')

      expect(page?.page.name).toBe('Demo Controls')
    })

    it('should read every set of the Demo Controls page', async () => {
      const page = await source.getComponentPage('Demo Controls')

      expect(page?.components).toHaveLength(10)
    })

    it('should select the configured canonical snapshot by ID', async () => {
      expect((await source.getComponentPageById('10:7', 'Demo Nav'))?.page.name).toBe('Demo Nav')
    })

    it('should keep the legacy capture that shares the canonical ID', async () => {
      expect((await source.getComponentPage('Demo: Legacy Bars'))?.page.id).toBe('10:7')
    })

    describe('and the canonical snapshot has a different page ID', () => {
      it('should reject the snapshot', async () => {
        await expect(source.getComponentPageById('different-id', 'Demo Nav')).rejects.toThrow(
          'Snapshot page identity changed'
        )
      })
    })

    describe('and no canonical snapshot is configured for an ambiguous ID', () => {
      it('should reject the ID as ambiguous', async () => {
        await expect(source.getComponentPageById('10:7')).rejects.toThrow(
          'Ambiguous snapshot page ID'
        )
      })
    })

    describe('and the page has not been captured', () => {
      it('should return nothing', async () => {
        expect(await source.getComponentPage('Not A Page')).toBeUndefined()
      })
    })
  })
})

// These small captures isolate fallback selection from the committed capture inventory.
describe('when a canonical snapshot is missing', () => {
  let root: string
  let source: SnapshotSource
  const id = '10:7'
  const legacy = 'Demo: Legacy Bars'
  const save = async (name: string, pageId = id) =>
    writeFile(
      path.join(root, 'components', encodePageFileName(name)),
      JSON.stringify({
        page: { id: pageId, name },
        components: []
      })
    )

  beforeEach(async () => {
    root = await mkdtemp(path.join(os.tmpdir(), 'snapshot-id-'))
    await mkdir(path.join(root, 'components'))
    await save(legacy)
    source = new SnapshotSource(root, [legacy])
  })

  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })

  describe('and one unclaimed file matches the ID', () => {
    beforeEach(async () => {
      await save('Unclaimed')
    })

    it('should use the unclaimed ID match', async () => {
      expect((await source.getComponentPageById(id, 'Demo Nav'))?.page.name).toBe('Unclaimed')
    })

    describe('and the canonical file is present too', () => {
      beforeEach(async () => {
        await save('Demo Nav')
      })

      it('should use the canonical file instead of the unclaimed match', async () => {
        expect((await source.getComponentPageById(id, 'Demo Nav'))?.page.name).toBe('Demo Nav')
      })
    })
  })

  describe('and the canonical file is present next to a malformed file', () => {
    beforeEach(async () => {
      await save('Demo Nav')
      await writeFile(path.join(root, 'components', 'Unclaimed.json'), 'invalid')
    })

    it('should read the canonical file without scanning past it', async () => {
      expect((await source.getComponentPageById(id, 'Demo Nav'))?.page.name).toBe('Demo Nav')
    })
  })

  describe('and the canonical file embeds a different ID', () => {
    beforeEach(async () => {
      await save('Demo Nav', '9:9')
      await save('Unclaimed')
    })

    it('should fail instead of falling back to another match', async () => {
      await expect(source.getComponentPageById(id, 'Demo Nav')).rejects.toThrow(
        'Snapshot page identity changed'
      )
    })
  })

  describe('and two unclaimed files match the ID', () => {
    beforeEach(async () => {
      await save('First')
      await save('Second')
    })

    it('should reject them as ambiguous', async () => {
      await expect(source.getComponentPageById(id, 'Demo Nav')).rejects.toThrow(
        'Ambiguous snapshot page ID'
      )
    })
  })

  describe('and only a legacy-claimed file matches the ID', () => {
    it('should not use it to replace the missing canonical capture', async () => {
      expect(await source.getComponentPageById(id, 'Demo Nav')).toBeUndefined()
    })

    it('should still serve the legacy capture by its name', async () => {
      expect((await source.getComponentPage(legacy))?.page.id).toBe(id)
    })

    describe('and the legacy exclusions come from the app configuration', () => {
      let app: Awaited<ReturnType<typeof createApp>>

      beforeEach(async () => {
        await writeFile(
          path.join(root, 'figma.config.json'),
          JSON.stringify({
            fileKey: 'AAAAAAAAAAAAAAAAAAAAAA',
            snapshotDir: '.',
            outDir: 'out',
            pages: {
              [id]: { id, name: 'Demo Nav', kind: 'components' },
              [`legacy:${legacy}`]: { id: null, name: legacy, kind: 'components' }
            }
          })
        )
        app = await createApp(root, undefined, { unverifiedSnapshot: true })
      })

      it('should not use the legacy-claimed file', async () => {
        expect(
          await app.createSource('snapshot').getComponentPageById!(id, 'Demo Nav')
        ).toBeUndefined()
      })
    })
  })
})
