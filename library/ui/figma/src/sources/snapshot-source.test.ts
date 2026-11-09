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
    it('should expose the four variable collections with 10, 5, 8 and 5 variables', async () => {
      const collections = await snapshotSource().getTokenCollections()

      expect(collections.map((c) => [c.name, c.variables.length])).toEqual([
        ['Primitives', 10],
        ['Semantic', 5],
        ['Spacing', 8],
        ['Layout', 5]
      ])
    })

    it('should keep the four layout modes', async () => {
      const layout = (await snapshotSource().getTokenCollections()).find((c) => c.name === 'Layout')

      expect(layout?.modes).toEqual(['desktop', 'large-desktop', 'tablet', 'mobile'])
    })

    it('should expose 7 text styles and 2 gradients', async () => {
      const source = snapshotSource()

      expect(await source.getTextStyles()).toHaveLength(7)
      expect(await source.getGradients()).toHaveLength(2)
    })

    it('should load every icon with its SVG markup', async () => {
      const icons = await snapshotSource().getIcons()

      expect(icons).toHaveLength(26)
      expect(icons.every((icon) => icon.svg.startsWith('<svg'))).toBe(true)
    })

    it('should read a captured component page with every set of the Demo Controls page', async () => {
      const page = await snapshotSource().getComponentPage('Demo Controls')

      expect(page?.page.name).toBe('Demo Controls')
      expect(page?.components).toHaveLength(10)
    })

    it('should select the configured canonical snapshot by ID without deleting legacy captures', async () => {
      expect((await snapshotSource().getComponentPageById('10:7', 'Demo Nav'))?.page.name).toBe(
        'Demo Nav'
      )
      expect((await snapshotSource().getComponentPage('Demo: Legacy Bars'))?.page.id).toBe('10:7')
    })

    it('should reject a canonical snapshot with a different page ID', async () => {
      await expect(
        snapshotSource().getComponentPageById('different-id', 'Demo Nav')
      ).rejects.toThrow('Snapshot page identity changed')
    })

    it('should reject an ambiguous ID when no canonical snapshot is configured', async () => {
      await expect(snapshotSource().getComponentPageById('10:7')).rejects.toThrow(
        'Ambiguous snapshot page ID'
      )
    })

    it('should return nothing for a page that has not been captured', async () => {
      expect(await snapshotSource().getComponentPage('Not A Page')).toBeUndefined()
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
  it('should use an unclaimed ID match only when the canonical file is absent', async () => {
    await save('Unclaimed')
    expect((await source.getComponentPageById(id, 'Demo Nav'))?.page.name).toBe('Unclaimed')
    await save('Demo Nav')
    expect((await source.getComponentPageById(id, 'Demo Nav'))?.page.name).toBe('Demo Nav')
  })
  it('should never scan past a present canonical file, even if another file is malformed', async () => {
    await save('Demo Nav')
    await writeFile(path.join(root, 'components', 'Unclaimed.json'), 'invalid')
    expect((await source.getComponentPageById(id, 'Demo Nav'))?.page.name).toBe('Demo Nav')
  })
  it('should fail an embedded-ID mismatch instead of falling back to another match', async () => {
    await save('Demo Nav', '9:9')
    await save('Unclaimed')
    await expect(source.getComponentPageById(id, 'Demo Nav')).rejects.toThrow(
      'Snapshot page identity changed'
    )
  })
  it('should reject two unclaimed ID matches as ambiguous', async () => {
    await save('First')
    await save('Second')
    await expect(source.getComponentPageById(id, 'Demo Nav')).rejects.toThrow(
      'Ambiguous snapshot page ID'
    )
  })
  it('should not use a legacy-claimed file to replace the missing canonical capture', async () => {
    expect(await source.getComponentPageById(id, 'Demo Nav')).toBeUndefined()
    expect((await source.getComponentPage(legacy))?.page.id).toBe(id)
  })
  it('should obtain legacy exclusions from the app configuration', async () => {
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
    const app = await createApp(root)
    expect(await app.createSource('snapshot').getComponentPageById!(id, 'Demo Nav')).toBeUndefined()
  })
})
