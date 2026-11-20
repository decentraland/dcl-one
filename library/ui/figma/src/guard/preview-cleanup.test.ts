import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import * as fsSafe from './fs-safe.js'
import { stageWorkspace } from './workspace-stage.js'
import { checkScope } from './check-scope.js'
import { copyManagedTree } from './copy.js'
import { generateAll } from '../generators/index.js'
import { writeFiles } from '../sync.js'
import { writeReviewManifest } from '../review-manifest.js'
import { previewCleanupPlan } from './preview-cleanup.js'
import * as previewPlans from './preview-cleanup.js'
import { createHash } from 'node:crypto'
import { readFile, rm, symlink, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createApp, runSync } from '../app.js'
import { pinTestCapture } from '../../test/snapshot.js'
import { fixtureCommit } from './git-fixture.js'
import { fixtureBytes, lifecycleFixture, put, json, GEN, SNAP } from './lifecycle-fixture.js'
import { listFiles } from './fs-safe.js'
let root: string
const read = (file: string) => readFile(path.join(root, file), 'utf8')
async function seal() {
  await pinTestCapture(path.join(root, 'figma'))
  const pin = await json(root, 'figma/test-capture.lock.json')
  const lock = await json(root, 'upstream.lock.json')
  lock.sources[0].snapshot = pin.sources[0].snapshot
  await put(root, 'upstream.lock.json', lock)
  const config = await json(root, 'figma/figma.config.json')
  config.snapshotLock = '../upstream.lock.json'
  await put(root, 'figma/figma.config.json', config)
}
async function select(ids: string[]) {
  const inventory = await json(root, `${SNAP}/generation-inventory.json`)
  inventory.review = [{ pageId: '1:1', componentIds: ids }]
  inventory.promotion = [{ pageId: '1:1', componentIds: ids }]
  await put(root, `${SNAP}/generation-inventory.json`, inventory)
  await seal()
}
const cleanup = async () =>
  runSync(await createApp(path.join(root, 'figma')), {
    source: 'snapshot',
    developer: true,
    prunePreview: true
  })
describe('preview cleanup', () => {
  beforeEach(async () => {
    ;({ root } = await lifecycleFixture())
    const raw = await json(root, `${SNAP}/components/Page.json`)
    raw.components[1].base.tree.svg = raw.components[1].base.tree.svg.replaceAll('h20', 'h19')
    await put(root, `${SNAP}/components/Page.json`, raw)
    await runSync(
      await createApp(path.join(root, 'figma'), undefined, { unverifiedSnapshot: true }),
      { source: 'snapshot', developer: true }
    )
    await pinTestCapture(path.join(root, 'figma'))
    const pin = await json(root, 'figma/test-capture.lock.json')
    pin.sources[0].files = {}
    for (const file of await listFiles(path.join(root, 'figma/review/src/generated'))) {
      const relative = path.relative(root, file).split(path.sep).join('/')
      pin.sources[0].files[relative] = {
        sha256: createHash('sha256')
          .update(await readFile(file))
          .digest('hex')
      }
    }
    await put(root, 'upstream.lock.json', pin)
    await seal()
    await fixtureCommit(root, 'test: pin generated preview baseline')
    await select(['2:1'])
  })
  afterEach(async () => {
    vi.restoreAllMocks()
    await rm(root, { recursive: true, force: true })
  })
  describe('when an explicitly requested developer cleanup follows a ready selection change', () => {
    it('should remove only excluded preview derivatives while preserving raw identities, public exports and retirement records', async () => {
      const source = await read(`${SNAP}/components/Page.json`),
        retirements = await read('figma/ledger/retirements.json'),
        publicIndex = await read('index.ts')
      const result = await cleanup()
      expect(result.pruned).toContain(`${GEN}/page/Second.tsx`)
      expect(result.pruned).toContain(`${GEN}/page/Second.module.css`)
      expect(result.pruned?.filter((file) => file.includes('/assets/'))).toHaveLength(1)
      expect(await read(`${SNAP}/components/Page.json`)).toBe(source)
      expect(await read('figma/ledger/retirements.json')).toBe(retirements)
      expect(await read('index.ts')).toBe(publicIndex)
      expect(await read('components/page/Second.tsx')).toContain('export const Second')
      expect(await json(root, 'figma/review/src/review/catalog.json')).toMatchObject({
        components: [{ name: 'First' }]
      })
      await expect(read(`${GEN}/page/Second.tsx`)).rejects.toMatchObject({ code: 'ENOENT' })
    })
    describe('and the cleanup already ran', () => {
      beforeEach(async () => {
        await cleanup()
      })
      it('should prune nothing on a second run', async () => {
        expect((await cleanup()).pruned).toEqual([])
      })
      describe('and a later genuine generation selects the component again', () => {
        beforeEach(async () => {
          await select(['2:1', '2:2'])
          await runSync(await createApp(path.join(root, 'figma')), {
            source: 'snapshot',
            developer: true
          })
        })
        it('should restore the same identity without a retirement record', async () => {
          expect(await read(`${GEN}/page/Second.tsx`)).toContain('export const Second')
          expect(await json(root, 'figma/ledger/retirements.json')).toEqual({
            schema: 1,
            retired: []
          })
        })
      })
    })
    describe('and an ordinary automated sync runs instead', () => {
      it('should leave every byte in place', async () => {
        const before = await fixtureBytes(root)
        await expect(
          runSync(await createApp(path.join(root, 'figma')), {
            source: 'snapshot',
            capturedPageIds: ['1:1']
          })
        ).rejects.toThrow(/removed.*without a retirement|sync would change output/)
        expect(await read(`${GEN}/page/Second.tsx`)).toContain('export const Second')
        expect(await fixtureBytes(root)).toEqual(before)
      })
    })
    describe('and an automated call asks for the developer capability', () => {
      it('should reject it before writing', async () => {
        const before = await fixtureBytes(root)
        await expect(
          runSync(await createApp(path.join(root, 'figma')), {
            source: 'snapshot',
            prunePreview: true
          })
        ).rejects.toThrow(/developer action/)
        expect(await fixtureBytes(root)).toEqual(before)
      })
    })
    describe('and the call combines selection cleanup with permanent identity retirement', () => {
      it('should reject them as separate operations', async () => {
        const before = await fixtureBytes(root)
        await expect(
          runSync(await createApp(path.join(root, 'figma')), {
            source: 'snapshot',
            developer: true,
            prunePreview: true,
            applyRetirements: true
          })
        ).rejects.toThrow(/separate operations/)
        expect(await fixtureBytes(root)).toEqual(before)
      })
    })
    describe('and the stale output was edited', () => {
      beforeEach(async () => {
        await put(
          root,
          `${GEN}/page/Second.tsx`,
          (await read(`${GEN}/page/Second.tsx`)) + '\n// local edit\n'
        )
      })
      it('should reject it and leave every base byte unchanged', async () => {
        const before = await fixtureBytes(root)
        await expect(cleanup()).rejects.toThrow(/edited|provenance/)
        expect(await fixtureBytes(root)).toEqual(before)
      })
    })
    describe('and an untracked stale module exists', () => {
      beforeEach(async () => {
        await put(
          root,
          `${GEN}/page/Unknown.tsx`,
          '// Generated by dcl-figma from unknown input\nexport const Unknown=1\n'
        )
      })
      it('should reject it without fabricating an ownership record', async () => {
        const before = await fixtureBytes(root)
        await expect(cleanup()).rejects.toThrow(/committed provenance/)
        expect(await fixtureBytes(root)).toEqual(before)
      })
    })
    describe('and surviving external code imports a stale derivative', () => {
      beforeEach(async () => {
        await put(
          root,
          'consumer.ts',
          "export { Second } from './figma/review/src/generated/components/page/Second'\n"
        )
      })
      it('should reject the cleanup', async () => {
        const before = await fixtureBytes(root)
        await expect(cleanup()).rejects.toThrow(/surviving consumer/)
        expect(await fixtureBytes(root)).toEqual(before)
      })
    })
    describe('and the capture was edited', () => {
      beforeEach(async () => {
        await put(root, `${SNAP}/pages.json`, { pages: [] })
      })
      it('should refuse it before staging or writing', async () => {
        const before = await fixtureBytes(root)
        await expect(cleanup()).rejects.toThrow(/capture|digest|fetch-source/)
        expect(await fixtureBytes(root)).toEqual(before)
      })
    })
    describe('and the output is a symlink', () => {
      let saved: Buffer
      beforeEach(async () => {
        const target = path.join(root, `${GEN}/page/Second.tsx`)
        saved = await readFile(target)
        await writeFile(path.join(root, 'saved.tsx'), saved)
        await rm(target)
        await symlink(path.join(root, 'saved.tsx'), target)
      })
      it('should reject it before touching its target or the base tree', async () => {
        await expect(cleanup()).rejects.toThrow(/symlink/)
        expect(await readFile(path.join(root, 'saved.tsx'))).toEqual(saved)
      })
    })
  })

  async function preparedStage() {
    const stage = await stageWorkspace(root, path.join(root, 'figma/figma.config.json'))
    try {
      await copyManagedTree(
        path.join(root, 'upstream.lock.json'),
        path.join(stage.stagedDir, 'upstream.lock.json')
      )
      const app = await createApp(path.join(stage.stagedDir, 'figma'))
      const fresh = await generateAll(app.createSource('snapshot'), {}, app.config)
      await writeFiles(app.resolveOutDir(), fresh.files)
      await writeReviewManifest(app)
      const plan = await previewCleanupPlan(root, stage.stagedDir)
      for (const file of plan.removed) await rm(path.join(stage.stagedDir, file))
      return stage
    } catch (error) {
      await stage.cleanup()
      throw error
    }
  }
  describe('when validating an explicit preview cleanup transition', () => {
    describe('and a cleaned stage was prepared', () => {
      let stage: Awaited<ReturnType<typeof preparedStage>>
      // Only a stage this test prepared is cleaned up, so a failed setup is not masked by a second cleanup.
      let prepared: typeof stage | undefined
      beforeEach(async () => {
        prepared = undefined
        stage = prepared = await preparedStage()
      })
      afterEach(async () => {
        await prepared?.cleanup()
      })
      it.each(['automated', 'partial'] as const)(
        'should refuse the capability in %s mode',
        async (mode) => {
          const result = await checkScope({
            baseDir: root,
            stagedDir: stage.stagedDir,
            capturedPageIds: [],
            runKind: mode === 'automated' ? 'automated' : 'developer',
            stage: mode === 'partial' ? 'partial' : 'complete',
            retirements: { schema: 1, retired: [] },
            prunePreview: true
          })
          expect(result.ok).toBe(false)
          expect(result.violations.some((v) => v.cause.includes('complete developer stage'))).toBe(
            true
          )
        }
      )
      describe.each([
        'components/page/Second.tsx',
        `${SNAP}/components/Page.json`,
        'upstream.lock.json',
        'figma/ledger/retirements.json'
      ])('and the stage mutates the protected input %s', (file) => {
        beforeEach(async () => {
          await writeFile(
            path.join(stage.stagedDir, file),
            file === 'figma/ledger/retirements.json'
              ? JSON.stringify({
                  schema: 1,
                  retired: [
                    {
                      pageId: '1:1',
                      componentId: '2:2',
                      name: 'Second',
                      reason: 'forged cleanup retirement',
                      owned: [`${GEN}/page/Second.tsx`],
                      sharedExclusions: []
                    }
                  ]
                })
              : '{}'
          )
        })
        it('should refuse the mutation', async () => {
          const result = await checkScope({
            baseDir: root,
            stagedDir: stage.stagedDir,
            capturedPageIds: [],
            runKind: 'developer',
            stage: 'complete',
            retirements: { schema: 1, retired: [] },
            prunePreview: true
          })
          expect(result.ok).toBe(false)
          expect(
            result.violations.some((v) => v.cause.includes('Preview cleanup validation failed'))
          ).toBe(true)
        })
      })
      describe('and the catalog is forged', () => {
        beforeEach(async () => {
          await put(stage.stagedDir, 'figma/review/src/review/catalog.json', { components: [] })
        })
        it('should refuse it although the files otherwise match the selected generation', async () => {
          await expect(previewCleanupPlan(root, stage.stagedDir, true)).rejects.toThrow(
            /freshly derived Review catalog/
          )
        })
      })
      describe('and selected output is missing', () => {
        beforeEach(async () => {
          await rm(path.join(stage.stagedDir, 'figma/review/src/generated/tokens/tokens.css'))
        })
        it('should refuse it instead of interpreting it as another authorized exclusion', async () => {
          await expect(previewCleanupPlan(root, stage.stagedDir, true)).rejects.toThrow(
            /freshly synchronized output/
          )
        })
      })
    })
    describe('and copying its stage fails', () => {
      beforeEach(() => {
        const original = fsSafe.atomicWrite
        vi.spyOn(fsSafe, 'atomicWrite').mockImplementation(async (file, ...args) => {
          if (!file.startsWith(root + path.sep) && file.endsWith('/components/index.ts'))
            throw Object.assign(new Error('simulated stage I/O failure'), { code: 'EIO' })
          return original(file, ...args)
        })
      })
      it('should leave the original tree unchanged', async () => {
        const before = await fixtureBytes(root)
        await expect(cleanup()).rejects.toMatchObject({
          code: 'EIO',
          message: 'simulated stage I/O failure'
        })
        expect(await fixtureBytes(root)).toEqual(before)
      })
    })
  })
  describe('when using the actual maintainer CLI', () => {
    const execute = promisify(execFile)
    const cli = path.resolve(import.meta.dirname, '../cli.ts')
    const run = (...args: string[]) =>
      execute(process.execPath, ['--import', import.meta.resolve('tsx'), cli, ...args], {
        cwd: path.join(root, 'figma')
      })
    describe('and a developer sync asks to prune the preview', () => {
      it('should expose preview pruning only through explicit developer sync', async () => {
        const result = await run('sync', '--developer', '--prune-preview')
        expect(JSON.parse(result.stdout).pruned).toContain(`${GEN}/page/Second.tsx`)
        expect(await read('components/page/Second.tsx')).toContain('export const Second')
      })
    })
    describe('and a read-only command gets the flag', () => {
      it('should refuse the flag without touching any byte', async () => {
        const before = await fixtureBytes(root)
        await expect(run('diff', '--prune-preview')).rejects.toMatchObject({
          code: 1,
          stderr: expect.stringContaining('applies only to sync')
        })
        expect(await fixtureBytes(root)).toEqual(before)
      })
    })
  })

  describe('when an external consumer resolves Preview output through a module alias', () => {
    describe('and real TypeScript paths resolve the alias', () => {
      beforeEach(async () => {
        await put(root, 'tsconfig.json', {
          compilerOptions: {
            module: 'ESNext',
            moduleResolution: 'Bundler',
            baseUrl: '.',
            paths: { '@preview/*': ['figma/review/src/generated/components/*'] }
          }
        })
        await put(root, 'consumer.ts', "export {Second} from '@preview/page/Second'\n")
      })
      it('should refuse deleting the imported module', async () => {
        const before = await fixtureBytes(root)
        await expect(cleanup()).rejects.toThrow(/surviving consumer/)
        expect(await fixtureBytes(root)).toEqual(before)
      })
    })
    describe('and an alias to an excluded component cannot be resolved', () => {
      beforeEach(async () => {
        await put(root, 'consumer.ts', "export {Second} from '@unknown/Second'\n")
      })
      it('should fail closed', async () => {
        const before = await fixtureBytes(root)
        await expect(cleanup()).rejects.toThrow(/unresolved module/)
        expect(await fixtureBytes(root)).toEqual(before)
      })
    })
  })

  describe('when a prior publication stopped after one stale derivative left', () => {
    it('should finish the exact remaining cleanup without retiring the source identity', async () => {
      const css = await read(`${GEN}/page/Second.module.css`)
      const asset = css.match(/\.\.\/assets\/([a-f0-9]+\.svg)/)?.[1]
      expect(asset).toBeDefined()
      await rm(path.join(root, GEN, 'assets', asset!))
      const result = await cleanup()
      expect(result.pruned).toContain(`${GEN}/page/Second.tsx`)
      expect((await json(root, `${SNAP}/components/Page.json`)).components).toHaveLength(2)
      expect(await read('components/page/Second.tsx')).toContain('export const Second')
      expect((await cleanup()).pruned).toEqual([])
    })
  })

  describe('when the new ready selection is empty', () => {
    beforeEach(async () => {
      await select([])
    })
    it('should preserve all preview and source bytes instead of interpreting a blank selection as teardown', async () => {
      const before = await fixtureBytes(root)
      await expect(cleanup()).rejects.toThrow(/empty ready component generation/)
      expect(await fixtureBytes(root)).toEqual(before)
    })
  })

  describe('when developer output overrides target another directory', () => {
    it('should refuse Preview cleanup without pruning the canonical output', async () => {
      const before = await fixtureBytes(root)
      await expect(
        runSync(await createApp(path.join(root, 'figma')), {
          source: 'snapshot',
          developer: true,
          prunePreview: true,
          outDir: '../other-preview'
        })
      ).rejects.toThrow(/requires the repository generated directory/)
      expect(await fixtureBytes(root)).toEqual(before)
    })
  })

  describe('when a dynamic import uses a stored module specifier', () => {
    describe.each([
      "const modulePath='./figma/review/src/generated/components/page/Second.js'; export const load=()=>import(modulePath)",
      "const modulePath='@preview/page/Second.js'; export const load=()=>import(modulePath)",
      "const original='@preview/page/Second.js'; const modulePath=(original as string); export const load=()=>import(modulePath)",
      "let modulePath='./components/page/First.js'; modulePath='@preview/page/Second.js'; export const load=()=>import(modulePath)",
      "const paths={value:'@preview/page/Second.js'}; export const load=()=>import(paths.value)",
      "const paths={value:'@preview/page/Second.js'}; export const load=()=>import(paths['value'])"
    ])('and a surviving consumer loads excluded output: %s', (consumer) => {
      beforeEach(async () => {
        await put(root, 'tsconfig.json', {
          compilerOptions: {
            module: 'ESNext',
            moduleResolution: 'Bundler',
            baseUrl: '.',
            paths: { '@preview/*': ['figma/review/src/generated/components/*'] }
          }
        })
        await put(root, 'consumer.ts', consumer)
      })
      it('should preserve every byte', async () => {
        const before = await fixtureBytes(root)
        await expect(cleanup()).rejects.toThrow(/surviving consumer/)
        expect(await fixtureBytes(root)).toEqual(before)
      })
    })
    describe('and an unresolved alias names an excluded module', () => {
      beforeEach(async () => {
        await put(
          root,
          'consumer.ts',
          "const modulePath='@unknown/Second.js'; export const load=()=>import(modulePath)"
        )
      })
      it('should refuse the cleanup', async () => {
        const before = await fixtureBytes(root)
        await expect(cleanup()).rejects.toThrow(/unresolved module/)
        expect(await fixtureBytes(root)).toEqual(before)
      })
    })
    describe('and an unrelated public component is loaded through an emitted JavaScript path', () => {
      beforeEach(async () => {
        await put(
          root,
          'consumer.ts',
          "const modulePath='./components/page/Second.js'; export const load=()=>import(modulePath)"
        )
      })
      it('should preserve the public component and prune the preview derivative', async () => {
        expect((await cleanup()).pruned).toContain(`${GEN}/page/Second.tsx`)
        expect(await read('components/page/Second.tsx')).toContain('export const Second')
      })
    })
  })

  describe('when review-round edge cases affect cleanup', () => {
    describe('and unrelated relative fixture strings name an excluded component', () => {
      beforeEach(async () => {
        await put(
          root,
          'fixture.ts',
          "export const cases=[{from:'./Second'},{from:'../Second.js'}]"
        )
      })
      it('should ignore them', async () => {
        expect((await cleanup()).pruned).toContain(`${GEN}/page/Second.tsx`)
      })
    })
    describe('and a dynamic import reads a lexically shadowed binding', () => {
      beforeEach(async () => {
        await put(
          root,
          'consumer.ts',
          "const modulePath='./Second'; export function load(){const modulePath='./components/page/Second.js';return import(modulePath)}"
        )
      })
      it('should respect the shadowing binding', async () => {
        expect((await cleanup()).pruned).toContain(`${GEN}/page/Second.tsx`)
      })
    })
    describe('and a tracked consumer is missing from the checkout', () => {
      beforeEach(async () => {
        await put(root, 'outside.ts', 'export const value=1')
        await fixtureCommit(root, 'test: track an external consumer')
        await promisify(execFile)('git', [
          '-C',
          root,
          'update-index',
          '--skip-worktree',
          'outside.ts'
        ])
        await rm(path.join(root, 'outside.ts'))
      })
      it('should require a full checkout rather than omit it', async () => {
        const before = await fixtureBytes(root)
        await expect(cleanup()).rejects.toThrow(/requires a full checkout/)
        expect(await fixtureBytes(root)).toEqual(before)
      })
    })
    describe('and the final verified removal set differs', () => {
      it('should refuse publication', async () => {
        const css = await read(`${GEN}/page/Second.module.css`)
        const asset = css.match(/\.\.\/assets\/([a-f0-9]+\.svg)/)?.[1]
        expect(asset).toBeDefined()
        const assetFile = path.join(root, GEN, 'assets', asset!)
        const originalAsset = await readFile(assetFile)
        await rm(assetFile)
        const beforeIndex = await read(`${GEN}/index.ts`)
        const beforeCatalog = await read('figma/review/src/review/catalog.json')
        const original = previewPlans.previewCleanupPlan
        let calls = 0
        vi.spyOn(previewPlans, 'previewCleanupPlan').mockImplementation(async (...args) => {
          if (++calls === 3) await writeFile(assetFile, originalAsset)
          return original(...args)
        })
        await expect(cleanup()).rejects.toThrow(/plan changed during validation/)
        expect(await read(`${GEN}/index.ts`)).toBe(beforeIndex)
        expect(await read('figma/review/src/review/catalog.json')).toBe(beforeCatalog)
        expect(await read(`${GEN}/page/Second.tsx`)).toContain('export const Second')
      })
    })
    describe('and the aggregates and catalog are published but stale removals are pending', () => {
      it('should recover on the next cleanup', async () => {
        const original = fsSafe.atomicWrite
        const catalog = path.join(root, 'figma/review/src/review/catalog.json')
        const source = await read(`${SNAP}/components/Page.json`)
        const publicIndex = await read('index.ts')
        const retirement = await read('figma/ledger/retirements.json')
        const failure = vi
          .spyOn(fsSafe, 'atomicWrite')
          .mockImplementation(async (file, ...args) => {
            await original(file, ...args)
            if (file === catalog)
              throw Object.assign(new Error('interrupted after catalog publish'), { code: 'EIO' })
          })
        await expect(cleanup()).rejects.toMatchObject({
          code: 'EIO',
          message: 'interrupted after catalog publish'
        })
        expect(await read(`${GEN}/index.ts`)).not.toContain('./page/Second')
        expect(await read(catalog.replace(root + path.sep, ''))).not.toContain('"name": "Second"')
        expect(await read(`${GEN}/page/Second.tsx`)).toContain('export const Second')
        failure.mockRestore()
        expect((await cleanup()).pruned).toContain(`${GEN}/page/Second.tsx`)
        expect((await cleanup()).pruned).toEqual([])
        expect(await read(`${SNAP}/components/Page.json`)).toBe(source)
        expect(await read('index.ts')).toBe(publicIndex)
        expect(await read('figma/ledger/retirements.json')).toBe(retirement)
        expect(await read('components/page/Second.tsx')).toContain('export const Second')
      })
    })
  })
})
