import { createHash } from 'node:crypto'
import { cp, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { afterAll, afterEach, beforeAll, describe, expect, it } from 'vitest'
import { createApp, runSync } from '../app.js'
import { runtimeFiles, writePromotionManifest } from './manifest.js'
import { promoteReviewed } from './promote.js'

// promoteReviewed against a temporary library built from the frozen fixture capture: sync generates
// the Review previews, the manifest seals them, then each case promotes into hand-written components.
const figmaRoot = path.resolve(import.meta.dirname, '../..')
const fixtures = path.join(figmaRoot, 'test/fixtures')

let template: string
const roots: string[] = []

async function files(directory: string, prefix = ''): Promise<string[]> {
  const entries = await readdir(directory, { withFileTypes: true }).catch(() => [])
  return (
    await Promise.all(
      entries.map((entry) => {
        const relative = prefix ? `${prefix}/${entry.name}` : entry.name
        return entry.isDirectory() ? files(path.join(directory, entry.name), relative) : [relative]
      })
    )
  ).flat()
}

// Every library file a promotion may touch, by content hash.
async function libraryState(root: string): Promise<Record<string, string>> {
  const state: Record<string, string> = {}
  for (const file of [
    'index.ts',
    'package.json',
    ...(await files(path.join(root, 'components'), 'components')),
    ...(await files(path.join(root, 'foundations'), 'foundations'))
  ].sort())
    state[file] = createHash('sha256')
      .update(await readFile(path.join(root, file)))
      .digest('hex')
  return state
}

async function handWritten(root: string, name: string) {
  const directory = path.join(root, 'components', name)
  await mkdir(directory, { recursive: true })
  await writeFile(
    path.join(directory, `${name}.tsx`),
    `export function ${name}() { return null }\nexport type ${name}Props = object\nexport type ${name}Variant = 'a'\n`
  )
  await writeFile(path.join(directory, `${name}.test.tsx`), `// hand-written ${name} test\n`)
  await writeFile(path.join(directory, `${name}.figma.ts`), `// component=${name}\n`)
  const entry = path.join(root, 'index.ts')
  await writeFile(
    entry,
    (await readFile(entry, 'utf8')) +
      `export { ${name} } from './components/${name}/${name}'\n` +
      `export type { ${name}Props, ${name}Variant } from './components/${name}/${name}'\n`
  )
}

async function library(include: string[] | undefined, ...names: string[]) {
  const root = await mkdtemp(path.join(os.tmpdir(), 'promote-library-'))
  roots.push(root)
  await cp(template, root, { recursive: true })
  const configFile = path.join(root, 'figma/figma.config.json')
  const config = JSON.parse(await readFile(configFile, 'utf8'))
  if (include) config.promotion = { include }
  else delete config.promotion
  await writeFile(configFile, JSON.stringify(config, null, 2))
  for (const name of names) await handWritten(root, name)
  return { root, app: await createApp(path.join(root, 'figma')) }
}

beforeAll(async () => {
  template = await mkdtemp(path.join(os.tmpdir(), 'promote-template-'))
  const figma = path.join(template, 'figma')
  const review = path.join(figma, 'review')
  await cp(path.join(fixtures, 'figma-snapshot'), path.join(figma, 'figma-snapshot'), {
    recursive: true
  })
  const config = JSON.parse(await readFile(path.join(fixtures, 'figma.config.json'), 'utf8'))
  await writeFile(
    path.join(figma, 'figma.config.json'),
    JSON.stringify({
      ...config,
      outDir: 'review/src/generated',
      contract: { ...config.contract, mappingRoot: '../components' }
    })
  )
  await mkdir(path.join(review, 'src/components'), { recursive: true })
  for (const file of runtimeFiles)
    await cp(
      path.join(figmaRoot, 'review/src/components', file),
      path.join(review, 'src/components', file)
    )
  await mkdir(path.join(review, 'public/dcl-ui-review'), { recursive: true })
  await writeFile(path.join(review, 'public/dcl-ui-review/evidence.txt'), 'evidence\n')
  await mkdir(path.join(review, 'src/review'), { recursive: true })
  await writeFile(path.join(review, 'src/review/catalog.json'), '{}\n')
  await writeFile(path.join(review, 'src/review/references.json'), '{}\n')
  await writeFile(path.join(template, 'index.ts'), "export * from './foundations'\n")
  await writeFile(
    path.join(template, 'package.json'),
    JSON.stringify({ name: 'fixture-library', version: '0.1.0' }, null, 2) + '\n'
  )
  await mkdir(path.join(template, 'components'), { recursive: true })
  await mkdir(path.join(template, 'foundations'), { recursive: true })

  const app = await createApp(figma)
  // Setup only: a developer regeneration of the starting Review (automated runs are scope-checked).
  await runSync(app, { source: 'snapshot', developer: true })
  // A preview that registers a sibling, so promotion pulls NavMenuItem in as a dependency only.
  const tab = path.join(review, 'src/generated/components/nav-bar/NavMenuTab.tsx')
  await writeFile(tab, `import '../nav-bar/NavMenuItem'\n${await readFile(tab, 'utf8')}`)
  await writePromotionManifest(app)
}, 120_000)

afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })))
})

afterAll(async () => {
  await rm(template, { recursive: true, force: true })
})

describe('promoteReviewed over a hand-written component', () => {
  describe('when promotion.include names the synced component', () => {
    // Promotes twice over a real Git-backed library tree, so it gets the setup budget, not the 5 s default.
    it('should replace only that hand-written directory and its exports, and be a no-op on a second run', async () => {
      const { root, app } = await library(['TabsBar'], 'TabsBar', 'Tabs')
      const tabs = await readFile(path.join(root, 'components/Tabs/Tabs.tsx'), 'utf8')

      const result = await promoteReviewed(app)

      expect(result).toMatchObject({ components: ['TabsBar'], superseded: ['TabsBar'] })
      expect(await files(path.join(root, 'components/TabsBar'))).toEqual([
        'TabsBar.figma.ts',
        'TabsBar.module.css',
        'TabsBar.module.css.d.ts',
        'TabsBar.tsx'
      ])
      expect(await readFile(path.join(root, 'components/TabsBar/TabsBar.tsx'), 'utf8')).not.toBe(
        "export function TabsBar() { return null }\nexport type TabsBarProps = object\nexport type TabsBarVariant = 'a'\n"
      )
      const entry = await readFile(path.join(root, 'index.ts'), 'utf8')
      expect(entry).not.toContain('TabsBarVariant')
      expect(entry.match(/export \{ TabsBar \}/g)).toHaveLength(1)
      expect(entry).toContain("export { Tabs } from './components/Tabs/Tabs'")
      expect(await readFile(path.join(root, 'components/Tabs/Tabs.tsx'), 'utf8')).toBe(tabs)
      expect(await readFile(path.join(root, 'components/Tabs/Tabs.test.tsx'), 'utf8')).toContain(
        'hand-written Tabs'
      )

      const after = await libraryState(root)
      expect(await promoteReviewed(app)).toMatchObject({ changed: false })
      expect(await libraryState(root)).toEqual(after)
    }, 120_000)
  })

  describe('when the synced component is pulled in only as a preview dependency', () => {
    it('should fail and leave the library untouched', async () => {
      const { root, app } = await library(['NavMenuTab'], 'NavMenuItem')
      const before = await libraryState(root)

      await expect(promoteReviewed(app)).rejects.toThrow(
        'components/NavMenuItem is a hand-written component; promotion replaces it only when promotion.include names NavMenuItem'
      )
      expect(await libraryState(root)).toEqual(before)
    })
  })

  describe('when promotion.include is absent and every eligible component is selected', () => {
    it('should fail and leave the library untouched', async () => {
      const { root, app } = await library(undefined, 'TabsBar')
      const before = await libraryState(root)

      await expect(promoteReviewed(app)).rejects.toThrow(
        'components/TabsBar is a hand-written component'
      )
      expect(await libraryState(root)).toEqual(before)
    })
  })

  describe('when the synced replacement is pending or ineligible', () => {
    it('should fail with the reasons and leave the library untouched', async () => {
      const { root, app } = await library(['ProgressBar'], 'ProgressBar')
      const before = await libraryState(root)

      await expect(promoteReviewed(app)).rejects.toThrow(
        'ProgressBar is excluded from promotion: Pending visual review'
      )
      expect(await libraryState(root)).toEqual(before)
    })
  })

  describe('when the promoted component keeps its text overrides private', () => {
    it('should promote the suppression unchanged and stay a no-op after a fresh sync and seal', async () => {
      const { root } = await library(['TitleBar'])
      const configFile = path.join(root, 'figma/figma.config.json')
      const config = JSON.parse(await readFile(configFile, 'utf8'))
      config.components = {
        ...config.components,
        TitleBar: { ...config.components?.TitleBar, exposeTextSlots: false }
      }
      await writeFile(configFile, JSON.stringify(config, null, 2))
      const app = await createApp(path.join(root, 'figma'))
      await runSync(app, { source: 'snapshot', developer: true })
      await writePromotionManifest(app)

      expect(await promoteReviewed(app)).toMatchObject({ components: ['TitleBar'] })
      const promoted = await readFile(path.join(root, 'components/TitleBar/TitleBar.tsx'), 'utf8')
      expect(promoted).toMatch(/^const titleBarSuppressedProps = \['label', /m)
      expect(promoted).toContain('suppressed={titleBarSuppressedProps}')
      expect(promoted).toContain('attributes={rest}')
      expect(promoted).not.toContain('{...rest}')
      expect(promoted).not.toMatch(/\b_label\d*\b/)

      const after = await libraryState(root)
      await runSync(app, { source: 'snapshot', developer: true })
      await writePromotionManifest(app)
      expect(await promoteReviewed(app)).toMatchObject({ changed: false })
      expect(await libraryState(root)).toEqual(after)
    }, 120_000)
  })

  describe('when a hand-written component has a synced name but is not selected', () => {
    it('should keep it and its exports while promoting the selected component', async () => {
      const { root, app } = await library(['TitleBar'], 'TabsBar')
      const handWrittenFiles = await files(path.join(root, 'components/TabsBar'))

      const result = await promoteReviewed(app)

      expect(result).toMatchObject({ components: ['TitleBar'], superseded: [] })
      expect(await files(path.join(root, 'components/TabsBar'))).toEqual(handWrittenFiles)
      expect(await readFile(path.join(root, 'index.ts'), 'utf8')).toContain('TabsBarVariant')
    })
  })
})
