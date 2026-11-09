import { mkdtemp, readFile, rm, writeFile, mkdir } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { snapshotSource, testConfig } from '../test/snapshot.js'
import { generateAll } from './generators/index.js'
import { diffFiles, isEmptyDiff, writeFiles } from './sync.js'

const context = {}
const generate = async () => generateAll(snapshotSource(), context, await testConfig())

describe('sync', () => {
  let outDir: string

  beforeEach(async () => {
    outDir = await mkdtemp(path.join(os.tmpdir(), 'dcl-figma-sync-'))
  })

  afterEach(async () => {
    await rm(outDir, { recursive: true, force: true })
  })

  describe('when syncing into an empty directory', () => {
    it('should add every generated file', async () => {
      const { files } = await generate()

      const diff = await writeFiles(outDir, files)

      expect(diff.added).toHaveLength(files.length)
      expect(await readFile(path.join(outDir, 'tokens/tokens.css'), 'utf8')).toContain(
        '--dcl-bg-page'
      )
    })
  })

  describe('when the file key is configured', () => {
    afterEach(() => vi.unstubAllEnvs())

    it('should generate no file that carries it or a Figma file URL', async () => {
      vi.stubEnv('FIGMA_FILE_KEY', 'FAKEKEY000example000000')
      const { files } = await generate()

      for (const file of files) {
        expect(file.content, file.path).not.toContain('FAKEKEY000example000000')
        expect(file.content, file.path).not.toMatch(/figma\.com\/(design|file|proto|board)\//)
      }
    })
  })

  describe('when syncing twice in a row', () => {
    it('should leave an empty diff the second time', async () => {
      const first = await generate()
      await writeFiles(outDir, first.files)

      const second = await generate()
      const diff = await writeFiles(outDir, second.files)

      expect(isEmptyDiff(diff)).toBe(true)
      expect(isEmptyDiff(await diffFiles(outDir, second.files))).toBe(true)
    })
  })

  describe('when a generated file was edited by hand', () => {
    it('should report it as changed and restore it on sync', async () => {
      const { files } = await generate()
      await writeFiles(outDir, files)
      await writeFile(path.join(outDir, 'tokens/theme.ts'), '// hand edit')

      const diff = await writeFiles(outDir, files)

      expect(diff.changed).toEqual(['tokens/theme.ts'])
      expect(await readFile(path.join(outDir, 'tokens/theme.ts'), 'utf8')).not.toBe('// hand edit')
    })
  })

  describe('when a stale file exists in a generated directory', () => {
    it('should report it, keep it, and never touch hand-written siblings', async () => {
      const { files } = await generate()
      await mkdir(path.join(outDir, 'icons'), { recursive: true })
      await writeFile(path.join(outDir, 'icons/Gone.tsx'), 'old')
      await mkdir(path.join(outDir, 'custom'), { recursive: true })
      await writeFile(path.join(outDir, 'custom/Keep.tsx'), 'mine')

      const diff = await writeFiles(outDir, files)

      expect(diff.removed).toEqual(['icons/Gone.tsx'])
      expect(await readFile(path.join(outDir, 'icons/Gone.tsx'), 'utf8')).toBe('old')
      expect(await readFile(path.join(outDir, 'custom/Keep.tsx'), 'utf8')).toBe('mine')
    })
  })

  describe('when the generation is empty but output exists', () => {
    it('should refuse to write', async () => {
      const { files } = await generate()
      await writeFiles(outDir, files)

      await expect(writeFiles(outDir, [])).rejects.toThrow(/empty generation/)
      expect(await readFile(path.join(outDir, 'tokens/tokens.css'), 'utf8')).toContain(
        '--dcl-bg-page'
      )
    })
  })

  describe('when reading the output fails for a reason other than absence', () => {
    it('should fail instead of treating the file as missing', async () => {
      const { files } = await generate()
      await mkdir(path.join(outDir, 'tokens'), { recursive: true })
      // A directory where a file is expected makes the read fail with EISDIR, not ENOENT.
      await mkdir(path.join(outDir, 'tokens/tokens.css'))

      await expect(diffFiles(outDir, files)).rejects.toThrow()
    })
  })
})
