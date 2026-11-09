import { cp, mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from 'vitest'
import { createApp, runSync, type App } from './app.js'
import { GENERATED, SNAPSHOT } from './guard/ownership.js'
import { fixtureBytes } from './guard/lifecycle-fixture.js'
import { copySnapshot, testConfigFile } from '../test/snapshot.js'
import { initializeFixtureGit } from './guard/git-fixture.js'
import { encodePageFileName } from './capture/page-file-name.js'

// A throwaway repository: figma with the synthetic snapshot, and the generated folder beside it.
describe('runSync guard and retirements', () => {
  let root: string
  let app: App

  // The generated baseline is the same for every case: build it once and give each case its own copy.
  let template: string
  beforeAll(async () => {
    template = await mkdtemp(path.join(os.tmpdir(), 'dcl-app-sync-template-'))
    const mcp = path.join(template, 'figma')
    await mkdir(mcp, { recursive: true })
    await copySnapshot(path.join(mcp, 'figma-snapshot'))
    const config = JSON.parse(await readFile(testConfigFile, 'utf8'))
    config.snapshotDir = 'figma-snapshot'
    config.outDir = `../${GENERATED}`
    await writeFile(path.join(mcp, 'figma.config.json'), JSON.stringify(config))
    // The first generation is a developer action, as it is in a real checkout.
    await runSync(await createApp(mcp, path.join(mcp, 'figma.config.json')), {
      source: 'snapshot',
      developer: true
    })
  })

  afterAll(async () => {
    await rm(template, { recursive: true, force: true })
  })

  beforeEach(async () => {
    root = await mkdtemp(path.join(os.tmpdir(), 'dcl-app-sync-'))
    await cp(template, root, { recursive: true, verbatimSymlinks: true })
    const mcp = path.join(root, 'figma')
    app = await createApp(mcp, path.join(mcp, 'figma.config.json'))
  })

  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })

  const generated = (file: string) => path.join(root, GENERATED, file)
  const read = (file: string) => readFile(generated(file), 'utf8')
  const alterUsedReviewToken = async (excludedPage?: string) => {
    const tokens = (await readdir(generated('tokens')))
      .filter((file) => file.endsWith('.css'))
      .map((file) => `tokens/${file}`)
    const modules = (await readdir(generated('components'), { recursive: true }))
      .filter((file) => file.endsWith('.module.css'))
      .filter((file) => !excludedPage || !file.startsWith(excludedPage + path.sep))
      .map((file) => `components/${file}`)
    const used = new Set(
      (await Promise.all(modules.map(read))).flatMap((text) =>
        [...text.matchAll(/var\(\s*(--[\w-]+)/g)].map((match) => match[1]!)
      )
    )
    for (const file of tokens) {
      const text = await read(file)
      const match = [...text.matchAll(/(--[\w-]+)\s*:\s*([^;}]+)/g)].find(([, name]) =>
        used.has(name!)
      )
      if (!match) continue
      await writeFile(generated(file), text.replace(match[0], `${match[1]}: #123456`))
      return true
    }
    return false
  }
  const writeRetirements = (owned: string[], pageId: string) =>
    writeFile(
      path.join(root, SNAPSHOT, 'retirements.json'),
      JSON.stringify({
        schema: 1,
        retired: [
          {
            pageId,
            name: 'x',
            reason: 'test',
            owned,
            sharedExclusions: []
          }
        ]
      })
    )

  describe('when an automated sync finds a stale file', () => {
    it('should report it and keep it', async () => {
      await mkdir(path.dirname(generated('icons/Gone.tsx')), { recursive: true })
      await writeFile(generated('icons/Gone.tsx'), 'old')

      const report = await runSync(app, { source: 'snapshot' })

      expect(report.diff.removed).toEqual(['icons/Gone.tsx'])
      expect(await read('icons/Gone.tsx')).toBe('old')
    })
  })

  describe('when an automated sync would rewrite output it does not own', () => {
    it('should refuse and write nothing', async () => {
      const manifest = await read('components/manifest.ts')
      const file = (/"file": "([^"]+)"/.exec(manifest) as RegExpExecArray)[1]!
      const target = `components/${file}.tsx`
      await writeFile(generated(target), '// edited by hand')

      await expect(runSync(app, { source: 'snapshot' })).rejects.toThrow(/does not own/)
      expect(await read(target)).toBe('// edited by hand')
    })

    it('should let a developer regenerate it', async () => {
      const manifest = await read('components/manifest.ts')
      const file = (/"file": "([^"]+)"/.exec(manifest) as RegExpExecArray)[1]!
      const target = `components/${file}.tsx`
      await writeFile(generated(target), '// edited by hand')

      await runSync(app, { source: 'snapshot', developer: true })

      expect(await read(target)).not.toBe('// edited by hand')
    })
  })

  describe('when an automated sync is pointed at the repository by another route', () => {
    it('should guard a symlink to the generated folder like the folder itself', async () => {
      const link = path.join(root, 'link-to-generated')
      await symlink(path.join(root, GENERATED), link)
      const manifest = await read('components/manifest.ts')
      const file = (/"file": "([^"]+)"/.exec(manifest) as RegExpExecArray)[1]!
      await writeFile(generated(`components/${file}.tsx`), '// edited by hand')

      await expect(runSync(app, { source: 'snapshot', outDir: link })).rejects.toThrow(
        /does not own/
      )
    })

    it('should refuse any other folder inside the repository', async () => {
      await expect(
        runSync(app, { source: 'snapshot', outDir: path.join(root, 'components') })
      ).rejects.toThrow(/only write the generated folder/)
    })

    it('should reject a scratch folder outside the repository', async () => {
      const scratch = await mkdtemp(path.join(os.tmpdir(), 'dcl-scratch-'))
      try {
        await expect(runSync(app, { source: 'snapshot', outDir: scratch })).rejects.toThrow(
          'outDir must stay inside'
        )
      } finally {
        await rm(scratch, { recursive: true, force: true })
      }
    })
  })

  describe('when an automated sync runs beside a promoted library it does not stage', () => {
    const promotion = () => path.join(root, 'components/promotion.json')
    beforeEach(async () => {
      await mkdir(path.join(root, 'components'), { recursive: true })
      await writeFile(promotion(), JSON.stringify({ schema: 1, files: {}, components: [] }))
      await writeFile(path.join(root, 'index.ts'), 'export {}\n')
    })

    it('should inherit the unstaged library and leave its bytes unchanged', async () => {
      const before = await readFile(promotion())

      await runSync(app, { source: 'snapshot' })

      expect(await readFile(promotion())).toEqual(before)
    })

    it('should still reject a Review token change that affects an unselected page', async () => {
      expect(await alterUsedReviewToken()).toBe(true)

      await expect(runSync(app, { source: 'snapshot' })).rejects.toThrow(
        /shared input change affects an unselected page/
      )
    })
  })

  describe('when the capturedPageIds API selects a page beside a populated library', () => {
    const selectedPageId = '10:5'
    type SavedBytes = Awaited<ReturnType<typeof fixtureBytes>>
    const expectSameBytes = (actual: SavedBytes, expected: SavedBytes) => {
      expect(actual.map(([file]) => file)).toEqual(expected.map(([file]) => file))
      for (const [index, entry] of actual.entries())
        expect((entry[1] as Buffer).equals(expected[index]![1] as Buffer), String(entry[0])).toBe(
          true
        )
    }
    const library = () => path.join(root, 'components')
    const frozenOutputs = async () =>
      (await fixtureBytes(path.join(root, GENERATED))).filter(([file]) => {
        const relative = String(file)
        return (
          relative.startsWith('components' + path.sep) &&
          relative.split(path.sep).length > 2 &&
          !relative.startsWith(path.join('components', 'demo-field') + path.sep)
        )
      })
    let libraryBefore: Awaited<ReturnType<typeof fixtureBytes>>
    let frozenBefore: Awaited<ReturnType<typeof fixtureBytes>>
    beforeEach(async () => {
      const stateFile = path.join(root, SNAPSHOT, 'sync-state.json')
      const state = JSON.parse(await readFile(stateFile, 'utf8'))
      for (const page of state.pages) page.ready = page.id === selectedPageId
      await writeFile(stateFile, JSON.stringify(state))
      await cp(generated(''), path.join(library(), 'src/generated'), { recursive: true })
      await mkdir(path.join(library(), 'src/components'), { recursive: true })
      await writeFile(
        path.join(library(), 'src/components/figma-module-runtime.tsx'),
        '// saved library runtime\n'
      )
      await writeFile(path.join(library(), 'src/index.ts'), 'export {}\n')
      await writeFile(
        path.join(library(), 'promotion.json'),
        JSON.stringify({
          schema: 1,
          components: [],
          files: {}
        })
      )
      libraryBefore = await fixtureBytes(library())
      frozenBefore = await frozenOutputs()
      expect(frozenBefore.length).toBeGreaterThan(0)
      const captureFile = path.join(root, SNAPSHOT, 'components/Demo Field.json')
      const page = JSON.parse(await readFile(captureFile, 'utf8'))
      expect(page.page.id).toBe(selectedPageId)
      const selectedStyle = (await readdir(generated('components/demo-field'))).find((file) =>
        file.endsWith('.module.css')
      )
      expect(selectedStyle).toBeDefined()
      await writeFile(
        generated(path.join('components/demo-field', selectedStyle!)),
        '/* stale output belonging to the captured page */\n'
      )
    })
    it('should update the selected page through stageOutput and preserve library and frozen bytes', async () => {
      const result = await runSync(app, {
        source: 'snapshot',
        capturedPageIds: [selectedPageId]
      })
      expect(result.diff.changed.some((file) => file.startsWith('components/demo-field/'))).toBe(
        true
      )
      expectSameBytes(await fixtureBytes(library()), libraryBefore)
      expectSameBytes(await frozenOutputs(), frozenBefore)
    })
    describe('and a Review token used by an unselected page differs', () => {
      let generatedBefore: Awaited<ReturnType<typeof fixtureBytes>>
      beforeEach(async () => {
        expect(await alterUsedReviewToken('demo-field')).toBe(true)
        generatedBefore = await fixtureBytes(path.join(root, GENERATED))
      })
      it('should reject the frozen dependency through the API without publishing output', async () => {
        await expect(
          runSync(app, {
            source: 'snapshot',
            capturedPageIds: [selectedPageId]
          })
        ).rejects.toThrow(/shared input change affects an unselected page/)
        expectSameBytes(await fixtureBytes(path.join(root, GENERATED)), generatedBefore)
        expectSameBytes(await fixtureBytes(library()), libraryBefore)
      })
    })
  })

  describe('when applying retirements', () => {
    it('should refuse without the developer flag', async () => {
      await expect(runSync(app, { source: 'snapshot', applyRetirements: true })).rejects.toThrow(
        /developer/
      )
    })

    it('should refuse a live source for retirement cleanup before reading it', async () => {
      await expect(
        runSync(app, { source: 'rest', developer: true, applyRetirements: true })
      ).rejects.toThrow('saved snapshots only')
    })
    it('should block an automated sync while a retirement is pending', async () => {
      const manifest = await read('components/manifest.ts')
      const entry = JSON.parse(manifest.slice(manifest.indexOf('= [') + 2))[0] as {
        file: string
        page: string
      }
      const saved = JSON.parse(
        await readFile(
          path.join(root, SNAPSHOT, `components/${encodePageFileName(entry.page)}`),
          'utf8'
        )
      )
      await writeRetirements([`${GENERATED}/components/${entry.file}.tsx`], saved.page.id)

      await expect(runSync(app, { source: 'snapshot' })).rejects.toThrow(/pending/)
      await expect(runSync(app, { source: 'snapshot', developer: true })).rejects.toThrow(/pending/)
    })

    it('should reject an incomplete retirement without changing published output', async () => {
      await mkdir(path.join(root, 'components'), { recursive: true })
      await writeFile(
        path.join(root, 'package.json'),
        JSON.stringify({ name: 'fixture', version: '0.1.0' })
      )
      await writeFile(
        path.join(root, 'pnpm-lock.yaml'),
        JSON.stringify({ version: '0.1.0', packages: { '': { version: '0.1.0' } } })
      )
      await initializeFixtureGit(root)
      const manifest = await read('components/manifest.ts')
      const entry = JSON.parse(manifest.slice(manifest.indexOf('= [') + 2))[0] as {
        file: string
        page: string
      }
      const saved = JSON.parse(
        await readFile(
          path.join(root, SNAPSHOT, `components/${encodePageFileName(entry.page)}`),
          'utf8'
        )
      )
      const owned = `${GENERATED}/components/${entry.file}.tsx`
      await writeRetirements([owned], saved.page.id)

      await expect(
        runSync(app, { source: 'snapshot', developer: true, applyRetirements: true })
      ).rejects.toThrow('inconsistent')
      expect(await readFile(path.join(root, owned), 'utf8')).toContain('export')
      expect(await read('components/manifest.ts')).toContain(entry.file)
    })
  })
})
