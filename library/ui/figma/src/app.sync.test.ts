import { cp, mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from 'vitest'
import { createApp, runSync, type App } from './app.js'
import { GENERATED, SNAPSHOT } from './guard/ownership.js'
import { RETIREMENTS_PATH } from './guard/retirements.js'
import { fixtureBytes } from './guard/lifecycle-fixture.js'
import { copySnapshot, testConfigFile } from '../test/snapshot.js'
import { initializeFixtureGit } from './guard/git-fixture.js'
import { encodePageFileName } from './capture/page-file-name.js'

// A throwaway repository: figma with the synthetic snapshot, and the generated folder beside it.
describe('runSync guard and retirements', () => {
  let root: string
  let app: App

  // The generated baseline is the same for every case: build it once and give each case its own copy.
  // A full developer sync per test would multiply the suite's runtime, so this one-time build stays a beforeAll.
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
    await runSync(
      await createApp(mcp, path.join(mcp, 'figma.config.json'), { unverifiedSnapshot: true }),
      {
        source: 'snapshot',
        developer: true
      }
    )
  })

  afterAll(async () => {
    await rm(template, { recursive: true, force: true })
  })

  beforeEach(async () => {
    root = await mkdtemp(path.join(os.tmpdir(), 'dcl-app-sync-'))
    await cp(template, root, { recursive: true, verbatimSymlinks: true })
    const mcp = path.join(root, 'figma')
    app = await createApp(mcp, path.join(mcp, 'figma.config.json'), { unverifiedSnapshot: true })
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
  const writeRetirements = async (owned: string[], pageId: string) => {
    await mkdir(path.dirname(path.join(root, RETIREMENTS_PATH)), { recursive: true })
    await writeFile(
      path.join(root, RETIREMENTS_PATH),
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
  }

  const firstManifestFile = async () => {
    const manifest = await read('components/manifest.ts')
    return (/"file": "([^"]+)"/.exec(manifest) as RegExpExecArray)[1]!
  }
  const firstManifestEntry = async () => {
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
    return { entry, pageId: saved.page.id as string }
  }
  // Starts a sync expected to fail; the rejection is handled here so each it() can await it on its own.
  const failingSync = (options: Parameters<typeof runSync>[1]) => {
    const outcome = runSync(app, options)
    outcome.catch(() => undefined)
    return outcome
  }

  describe('when an automated sync finds a stale file', () => {
    let report: Awaited<ReturnType<typeof runSync>>

    beforeEach(async () => {
      await mkdir(path.dirname(generated('icons/Gone.tsx')), { recursive: true })
      await writeFile(generated('icons/Gone.tsx'), 'old')

      report = await runSync(app, { source: 'snapshot' })
    })

    it('should report it as removed', () => {
      expect(report.diff.removed).toEqual(['icons/Gone.tsx'])
    })

    it('should keep it', async () => {
      expect(await read('icons/Gone.tsx')).toBe('old')
    })
  })

  describe('when an automated sync would rewrite output it does not own', () => {
    let target: string

    beforeEach(async () => {
      target = `components/${await firstManifestFile()}.tsx`
      await writeFile(generated(target), '// edited by hand')
    })

    describe('and the sync is automated', () => {
      let outcome: Promise<unknown>

      beforeEach(() => {
        outcome = failingSync({ source: 'snapshot' })
      })

      it('should refuse', async () => {
        await expect(outcome).rejects.toThrow(/does not own/)
      })

      it('should write nothing', async () => {
        await outcome.catch(() => undefined)
        expect(await read(target)).toBe('// edited by hand')
      })
    })

    describe('and a developer runs the sync', () => {
      beforeEach(async () => {
        await runSync(app, { source: 'snapshot', developer: true })
      })

      it('should regenerate it', async () => {
        expect(await read(target)).not.toBe('// edited by hand')
      })
    })
  })

  describe('when an automated sync is pointed at the repository by another route', () => {
    describe('and the route is a symlink to the generated folder', () => {
      let link: string

      beforeEach(async () => {
        link = path.join(root, 'link-to-generated')
        await symlink(path.join(root, GENERATED), link)
        await writeFile(
          generated(`components/${await firstManifestFile()}.tsx`),
          '// edited by hand'
        )
      })

      it('should guard it like the folder itself', async () => {
        await expect(runSync(app, { source: 'snapshot', outDir: link })).rejects.toThrow(
          /does not own/
        )
      })
    })

    describe('and the route is any other folder inside the repository', () => {
      it('should refuse it', async () => {
        await expect(
          runSync(app, { source: 'snapshot', outDir: path.join(root, 'components') })
        ).rejects.toThrow(/only write the generated folder/)
      })
    })

    describe('and the route is a scratch folder outside the repository', () => {
      let scratch: string

      beforeEach(async () => {
        scratch = await mkdtemp(path.join(os.tmpdir(), 'dcl-scratch-'))
      })

      afterEach(async () => {
        await rm(scratch, { recursive: true, force: true })
      })

      it('should reject it', async () => {
        await expect(runSync(app, { source: 'snapshot', outDir: scratch })).rejects.toThrow(
          'outDir must stay inside'
        )
      })
    })
  })

  describe('when an automated sync runs beside a promoted library it does not stage', () => {
    const promotion = () => path.join(root, 'components/promotion.json')
    beforeEach(async () => {
      await mkdir(path.join(root, 'components'), { recursive: true })
      await writeFile(promotion(), JSON.stringify({ schema: 1, files: {}, components: [] }))
      await writeFile(path.join(root, 'index.ts'), 'export {}\n')
    })

    describe('and nothing shared changed', () => {
      let before: Buffer

      beforeEach(async () => {
        before = await readFile(promotion())

        await runSync(app, { source: 'snapshot' })
      })

      it('should inherit the unstaged library and leave its bytes unchanged', async () => {
        expect(await readFile(promotion())).toEqual(before)
      })
    })

    describe('and a Review token change affects an unselected page', () => {
      beforeEach(async () => {
        expect(await alterUsedReviewToken()).toBe(true)
      })

      it('should still reject the sync', async () => {
        await expect(runSync(app, { source: 'snapshot' })).rejects.toThrow(
          /shared input change affects an unselected page/
        )
      })
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

    describe('and only the selected page changed', () => {
      let result: Awaited<ReturnType<typeof runSync>>

      beforeEach(async () => {
        result = await runSync(app, {
          source: 'snapshot',
          capturedPageIds: [selectedPageId]
        })
      })

      it('should update the selected page through stageOutput', () => {
        expect(result.diff.changed.some((file) => file.startsWith('components/demo-field/'))).toBe(
          true
        )
      })

      it('should preserve the library bytes', async () => {
        expectSameBytes(await fixtureBytes(library()), libraryBefore)
      })

      it('should preserve the frozen output bytes', async () => {
        expectSameBytes(await frozenOutputs(), frozenBefore)
      })
    })

    describe('and a Review token used by an unselected page differs', () => {
      let generatedBefore: Awaited<ReturnType<typeof fixtureBytes>>
      let outcome: Promise<unknown>
      beforeEach(async () => {
        expect(await alterUsedReviewToken('demo-field')).toBe(true)
        generatedBefore = await fixtureBytes(path.join(root, GENERATED))
        outcome = failingSync({
          source: 'snapshot',
          capturedPageIds: [selectedPageId]
        })
      })

      it('should reject the frozen dependency through the API', async () => {
        await expect(outcome).rejects.toThrow(/shared input change affects an unselected page/)
      })

      it('should not publish output', async () => {
        await outcome.catch(() => undefined)
        expectSameBytes(await fixtureBytes(path.join(root, GENERATED)), generatedBefore)
      })

      it('should leave the library bytes unchanged', async () => {
        await outcome.catch(() => undefined)
        expectSameBytes(await fixtureBytes(library()), libraryBefore)
      })
    })
  })

  describe('when the config names a snapshotDir other than figma/figma-snapshot', () => {
    let before: string
    let errors: unknown[]

    beforeEach(async () => {
      const other = path.join(root, 'figma/other-snapshot')
      await cp(path.join(root, SNAPSHOT), other, { recursive: true })
      app.config.snapshotDir = 'other-snapshot'
      before = await read('components/manifest.ts')

      errors = []
      for (const options of [
        { source: 'snapshot' as const },
        { source: 'snapshot' as const, developer: true },
        { source: 'snapshot' as const, developer: true, applyRetirements: true }
      ])
        errors.push(
          await runSync(app, options).then(
            () => undefined,
            (error: unknown) => error
          )
        )
    })

    it('should refuse an automated, a developer and a retirement sync', () => {
      expect(errors).toEqual([
        expect.objectContaining({ message: expect.stringContaining('names another snapshotDir') }),
        expect.objectContaining({ message: expect.stringContaining('names another snapshotDir') }),
        expect.objectContaining({ message: expect.stringContaining('names another snapshotDir') })
      ])
    })

    it('should refuse before ownership, retirements or the guard change the manifest', async () => {
      expect(await read('components/manifest.ts')).toBe(before)
    })
  })

  describe('when applying retirements', () => {
    describe('and the developer flag is missing', () => {
      it('should refuse', async () => {
        await expect(runSync(app, { source: 'snapshot', applyRetirements: true })).rejects.toThrow(
          /developer/
        )
      })
    })

    describe('and a retirement is pending', () => {
      beforeEach(async () => {
        const { entry, pageId } = await firstManifestEntry()
        await writeRetirements([`${GENERATED}/components/${entry.file}.tsx`], pageId)
      })

      it('should block an automated sync', async () => {
        await expect(runSync(app, { source: 'snapshot' })).rejects.toThrow(/pending/)
      })

      it('should block a developer sync that does not apply it', async () => {
        await expect(runSync(app, { source: 'snapshot', developer: true })).rejects.toThrow(
          /pending/
        )
      })
    })

    describe('and the retirement is incomplete', () => {
      let entry: { file: string; page: string }
      let owned: string
      let outcome: Promise<unknown>

      beforeEach(async () => {
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
        const first = await firstManifestEntry()
        entry = first.entry
        owned = `${GENERATED}/components/${entry.file}.tsx`
        await writeRetirements([owned], first.pageId)

        outcome = failingSync({ source: 'snapshot', developer: true, applyRetirements: true })
      })

      it('should reject it', async () => {
        await expect(outcome).rejects.toThrow('inconsistent')
      })

      it('should keep the owned output', async () => {
        await outcome.catch(() => undefined)
        expect(await readFile(path.join(root, owned), 'utf8')).toContain('export')
      })

      it('should keep the manifest entry', async () => {
        await outcome.catch(() => undefined)
        expect(await read('components/manifest.ts')).toContain(entry.file)
      })
    })
  })
})
