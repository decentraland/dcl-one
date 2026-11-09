import { cp, mkdir, mkdtemp, readFile, readdir, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { createApp, runSync } from '../app.js'
import { writeReviewManifest } from '../review-manifest.js'
import { runtimeFiles } from '../promotion/eligibility.js'
import { listFiles } from './fs-safe.js'
import { computeOwned } from './apply-retirements.js'
import { emptyRetirements, type Retirements } from './retirements.js'
import { initializeFixtureGit } from './git-fixture.js'
const SNAP = 'figma/figma-snapshot'
const GEN = 'figma/review/src/generated/components'
const LIB = 'components'
const put = async (root: string, file: string, value: unknown) => {
  await mkdir(path.dirname(path.join(root, file)), { recursive: true })
  await writeFile(
    path.join(root, file),
    typeof value === 'string' ? value : JSON.stringify(value, null, 2) + '\n'
  )
}
const json = async (root: string, file: string) =>
  JSON.parse(await readFile(path.join(root, file), 'utf8'))

async function buildLifecycleFixture(splitPages: boolean, duplicateNames: boolean, parent: string) {
  const publicNames: readonly [string, string] = duplicateNames
    ? ['Foo', 'Foo2']
    : ['First', 'Second']
  const [first, second] = publicNames
  // Two live sets may no longer share a generated name, so the duplicate reaches Foo2 through an alias.
  const capturedNames = duplicateNames ? [first, `${first} copy`] : publicNames
  const root = await mkdtemp(path.join(parent, 'retirement-lifecycle-'))
  const svg =
    '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20">' +
    '<path d="M0 0h20v20H0z"/>'.repeat(300) +
    '</svg>'
  await put(root, 'figma/figma.config.json', {
    snapshotDir: 'figma-snapshot',
    outDir: 'review/src/generated',
    pages: { '1:1': { id: '1:1', name: 'Page', kind: 'components' } },
    selection: { readyOnly: true },
    ...(duplicateNames ? { componentNames: { [`${first} copy`]: second } } : {})
  })
  for (const [file, value] of Object.entries({
    'variables.json': {
      collections: ['Primitives', 'Semantic', 'Spacing', 'Layout'].map((name) => ({
        name,
        modes: ['Value'],
        variables: []
      }))
    },
    'text-styles.json': { styles: [] },
    'paint-styles.json': { gradients: [] },
    'icons.json': { icons: [] },
    'pages.json': { pages: [{ id: '1:1', name: 'Page' }] },
    'sync-state.json': { pages: [{ id: '1:1', name: 'Page', ready: true }] },
    'generation-inventory.json': {
      schema: 1,
      review: [{ pageId: '1:1' }],
      promotion: [{ pageId: '1:1' }]
    },
    'retirements.json': emptyRetirements(),
    'components/Page.json': {
      page: { id: '1:1', name: 'Page' },
      components: capturedNames.map((name, index) => ({
        kind: 'component',
        id: `2:${index + 1}`,
        name,
        section: [],
        x: 0,
        y: 0,
        properties: {},
        base: {
          id: `2:${index + 1}`,
          name,
          props: {},
          tree: { type: 'FRAME', name, w: 20, h: 20, svg }
        },
        others: []
      }))
    }
  }))
    await put(root, `${SNAP}/${file}`, value)
  if (splitPages) {
    const config = await json(root, 'figma/figma.config.json')
    config.pages['1:2'] = { id: '1:2', name: 'Other', kind: 'components' }
    await put(root, 'figma/figma.config.json', config)
    const page = await json(root, `${SNAP}/components/Page.json`)
    await put(root, `${SNAP}/components/Other.json`, {
      page: { id: '1:2', name: 'Other' },
      components: [page.components.pop()]
    })
    await put(root, `${SNAP}/components/Page.json`, page)
    const state = await json(root, `${SNAP}/sync-state.json`)
    state.pages.push({ id: '1:2', name: 'Other', ready: false })
    await put(root, `${SNAP}/sync-state.json`, state)
    const inventory = await json(root, `${SNAP}/generation-inventory.json`)
    inventory.review.push({ pageId: '1:2' })
    inventory.promotion.push({ pageId: '1:2' })
    await put(root, `${SNAP}/generation-inventory.json`, inventory)
  }
  const app = await createApp(path.join(root, 'figma'), undefined, { unverifiedSnapshot: true })
  await runSync(app, { source: 'snapshot', developer: true })
  await writeReviewManifest(app)
  for (const name of runtimeFiles)
    await put(root, `figma/review/src/components/${name}`, '// fixture runtime\n')
  await cp(path.join(root, GEN), path.join(root, LIB), { recursive: true })
  for (const name of publicNames)
    await put(
      root,
      `${LIB}/${splitPages && name === second ? 'other' : 'page'}/${name}.figma.ts`,
      '// fixture mapping\n'
    )
  await put(
    root,
    'index.ts',
    publicNames
      .flatMap((name, index) => {
        const module = `./components/${splitPages && index === 1 ? 'other' : 'page'}/${name}`
        return [
          `export { ${name} } from '${module}'`,
          `export type { ${name}Props${index === 0 ? `, ${name}Variant` : ''} } from '${module}'`
        ]
      })
      .join('\n') + '\n'
  )
  await put(root, 'package.json', { name: 'fixture-library', version: '0.1.0' })
  await put(root, 'pnpm-lock.yaml', {
    name: 'fixture-library',
    version: '0.1.0',
    lockfileVersion: 3,
    packages: { '': { name: 'fixture-library', version: '0.1.0' } }
  })
  await put(root, 'components/promotion.json', {
    schema: 1,
    components: publicNames,
    files: {}
  })
  const shared = `${GEN}/assets/${(await readdir(path.join(root, GEN, 'assets')))[0]}`
  const records: Retirements = {
    schema: 1,
    retired: [
      {
        pageId: '1:1',
        componentId: '2:1',
        name: first,
        reason: 'Explicit component retirement',
        owned: await computeOwned(root, { pageId: '1:1', componentId: '2:1' }),
        sharedExclusions: [shared, shared.replace(GEN, LIB)]
      }
    ]
  }
  await initializeFixtureGit(root)
  return { root, shared, records }
}

// Building the fixture runs a sync and several Git commands. Each test file builds every variant once and
// hands each test its own copy, Git history included, so tests stay isolated without paying the build.
const templates = new Map<string, ReturnType<typeof buildLifecycleFixture>>()

async function lifecycleFixture(splitPages = false, duplicateNames = false) {
  // Without the run folder from the global setup nothing would remove a shared template: build per test.
  const templateFolder = process.env.DCL_FIXTURE_TEMPLATES
  if (!templateFolder) {
    const built = await buildLifecycleFixture(splitPages, duplicateNames, os.tmpdir())
    return {
      ...built,
      app: await createApp(path.join(built.root, 'figma'), undefined, { unverifiedSnapshot: true })
    }
  }
  const key = `${splitPages}:${duplicateNames}`
  if (!templates.has(key))
    templates.set(key, buildLifecycleFixture(splitPages, duplicateNames, templateFolder))
  const template = await templates.get(key)!
  const root = await mkdtemp(path.join(os.tmpdir(), 'retirement-lifecycle-'))
  await cp(template.root, root, { recursive: true, verbatimSymlinks: true })
  const app = await createApp(path.join(root, 'figma'), undefined, { unverifiedSnapshot: true })
  return { root, app, shared: template.shared, records: structuredClone(template.records) }
}
const fixtureBytes = async (root: string) =>
  Promise.all(
    (await listFiles(root))
      .filter((file) => !path.relative(root, file).startsWith('.git' + path.sep))
      .sort()
      .map(async (file) => [path.relative(root, file), await readFile(file)])
  )
export { lifecycleFixture, fixtureBytes, put, json, SNAP, GEN, LIB }
