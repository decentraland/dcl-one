import { cp, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import type { App } from '../app.js'
import { promotionMapping } from './mappings.js'
import { generateComponents } from '../generators/components.js'
import { generateIcons } from '../generators/icons.js'
import { generateFoundations } from '../generators/tokens.js'
import type { ComponentSpec } from '../ir/components.js'
import { readMaterialized } from '../lfs.js'
import { runtimeFiles, verifyPromotionManifest, type PromotionManifest } from './manifest.js'

// Promotion copies reviewed output into the shared library (library/ui):
//   components/<Name>/<Name>.tsx|.module.css|.figma.ts   reviewed previews, one directory each
//   components/assets/                                    SVG assets their stylesheets reference
//   components/runtime/                                   the reviewed FigmaView runtime
//   foundations/<family>/, foundations/icons/             tokens and icons from the verified snapshot
// It never reads live Figma and never regenerates a component design.
const REVIEW_GENERATED = 'figma/review/src/generated'
const REVIEW_RUNTIME = 'figma/review/src/components'
const RECORD = 'components/promotion.json'
const PROMOTED_BLOCK = /\n?\/\/ BEGIN PROMOTED COMPONENTS[\s\S]*?\/\/ END PROMOTED COMPONENTS\n?/g

type Output = ReturnType<typeof generateComponents>

const readText = async (file: string) => (await readMaterialized(file)).toString('utf8')

// Exact class-name typings, so a typo in a generated className fails typechecking.
async function declareCssModules(directory: string): Promise<void> {
  for (const item of await readdir(directory, { withFileTypes: true })) {
    const file = path.join(directory, item.name)
    if (item.isDirectory()) await declareCssModules(file)
    else if (item.name.endsWith('.module.css')) {
      const content = await readFile(file, 'utf8')
      const names = [
        ...new Set([...content.matchAll(/\.([a-zA-Z_][\w-]*)/g)].map((match) => match[1]!))
      ]
      await writeFile(
        `${file}.d.ts`,
        `declare const styles: {\n${names.map((name) => `  readonly ${JSON.stringify(name)}: string`).join('\n')}\n}\nexport default styles\n`
      )
    }
  }
}

async function listFiles(directory: string, prefix = ''): Promise<string[]> {
  const entries = await readdir(directory, { withFileTypes: true }).catch(() => [])
  return (
    await Promise.all(
      entries.map((entry) => {
        const relative = prefix ? `${prefix}/${entry.name}` : entry.name
        return entry.isDirectory()
          ? listFiles(path.join(directory, entry.name), relative)
          : [relative]
      })
    )
  ).flat()
}

// components/<Name>/..., components/assets/..., components/runtime/... -> its top two segments.
const promotedDir = (file: string) => file.split('/').slice(0, 2).join('/')

/** Generated previews import sibling components for registration; promotion must carry them too. */
function dependencies(content: string): string[] {
  return [...content.matchAll(/^import '\.\.\/[^/]+\/([A-Za-z0-9]+)'$/gm)].map((match) => match[1]!)
}

async function selectComponents(
  app: App,
  manifest: PromotionManifest,
  output: Output,
  root: string
): Promise<ComponentSpec[]> {
  const eligible = new Map(manifest.components.map((spec) => [spec.name, spec]))
  const include = app.config.promotion?.include ?? [...eligible.keys()]
  const selected = new Map<string, ComponentSpec>()
  const queue = [...include]
  while (queue.length) {
    const name = queue.shift()!
    if (selected.has(name)) continue
    const spec = eligible.get(name)
    if (!spec) {
      const excluded = manifest.excluded.find((item) => item.name === name)
      throw new Error(
        excluded
          ? `${name} is excluded from promotion: ${excluded.reasons.join('; ')}`
          : `${name} is not a reviewed component eligible for promotion`
      )
    }
    selected.set(name, spec)
    const entry = output.manifest.find((item) => item.name === name)!
    queue.push(
      ...dependencies(
        await readText(path.join(root, REVIEW_GENERATED, `components/${entry.file}.tsx`))
      )
    )
  }
  return [...selected.values()]
}

/** Rewrites a reviewed preview (TSX or CSS Module) for its components/<Name>/ directory. */
function relocate(content: string): string {
  return (
    content
      .replace(/from '\.\.\/\.\.\/\.\.\/components\/(figma-[a-z-]+)'/g, "from '../runtime/$1'")
      .replace(
        "import type { MatrixCase } from '../../../components/StateMatrix'",
        "type MatrixCase = { key: string; props: Record<string, unknown>; states: ('base' | ForcedState)[] }"
      )
      .replace(/^import '\.\.\/[^/]+\/([A-Za-z0-9]+)'$/gm, "import '../$1/$1'")
      // Text styles live in generated/tokens in Review and in foundations/typography in the library.
      .replace(
        /from '\.\.\/\.\.\/tokens\/text-styles\.module\.css'/g,
        "from '../../foundations/typography/text-styles.module.css'"
      )
  )
}

// The hand-written directories a promotion supersedes: components/<dir>/ exported from the public entry
// under a promoted name. Promotion output (runtime, assets, the promoted block) is never a candidate.
function supersededDirectories(entry: string, names: string[]): string[] {
  const directories = new Set<string>()
  for (const match of entry
    .replace(PROMOTED_BLOCK, '\n')
    .matchAll(/^export (?:type )?\{([^}]*)\} from '\.\/components\/([^/']+)\/[^']*'$/gm)) {
    const directory = match[2]!
    if (directory === 'runtime' || directory === 'assets') continue
    const exported = match[1]!.split(',').map((name) => name.trim())
    if (exported.some((name) => names.includes(name))) directories.add(directory)
  }
  return [...directories].sort()
}

// Fails closed: a synced component replaces a hand-written one only when promotion.include names it
// explicitly, never through the implicit all-eligible default or as a preview dependency.
const supersession = (entry: string, selected: string[], include: string[] | undefined) =>
  supersededDirectories(
    entry,
    selected.filter((name) => include?.includes(name))
  )

/** The public entry without superseded hand-written exports, with one promoted block at the end. */
function publicEntry(current: string, superseded: string[], promoted: string[]): string {
  return (
    current
      .replace(PROMOTED_BLOCK, '\n')
      .split('\n')
      .filter(
        (line) => !superseded.some((directory) => line.includes(`from './components/${directory}/`))
      )
      .join('\n')
      .trimEnd() +
    '\n\n// BEGIN PROMOTED COMPONENTS\n' +
    [...promoted].sort().join('\n') +
    '\n// END PROMOTED COMPONENTS\n'
  )
}

async function promoteReviewed(app: App) {
  const manifest = await verifyPromotionManifest(app)
  if (!manifest.components.length)
    throw new Error('No reviewed components are eligible for promotion')
  const root = path.resolve(app.cwd, '..')
  const output = generateComponents(manifest.components, {})
  const selected = await selectComponents(app, manifest, output, root)
  const names = selected.map((spec) => spec.name).sort()
  const previous = await readFile(path.join(root, RECORD), 'utf8').catch(() => undefined)
  const entryFile = path.join(root, 'index.ts')
  const currentEntry = await readFile(entryFile, 'utf8')
  const superseded = supersession(currentEntry, names, app.config.promotion?.include)

  const temporary = await mkdtemp(path.join(os.tmpdir(), 'figma-promote-'))
  try {
    const staged = (relative: string) => path.join(temporary, relative)
    const write = async (relative: string, content: string | Buffer) => {
      await mkdir(path.dirname(staged(relative)), { recursive: true })
      await writeFile(staged(relative), content)
    }

    const previousNames: string[] = previous ? JSON.parse(previous).components : []
    const assets = new Set<string>()
    for (const spec of selected) {
      const entry = output.manifest.find((item) => item.name === spec.name)!
      const base = path.basename(entry.file)
      const existing = await readdir(path.join(root, 'components', base)).catch(() => undefined)
      if (existing && !previousNames.includes(spec.name) && !superseded.includes(base))
        throw new Error(
          `components/${base} is a hand-written component; promotion replaces it only when promotion.include names ${spec.name}`
        )
      for (const extension of ['.tsx', '.module.css']) {
        const content = await readText(
          path.join(root, REVIEW_GENERATED, `components/${entry.file}${extension}`)
        )
        if (extension === '.module.css')
          for (const match of content.matchAll(/url\("\.\.\/assets\/([^")]+)"\)/g))
            assets.add(match[1]!)
        await write(`components/${base}/${base}${extension}`, relocate(content))
      }
      const page = await app.createSource('snapshot').getComponentPage(spec.page)
      const raw = page?.components.find((item) => item.id === spec.figmaId)
      if (!raw) throw new Error(`Missing reviewed component: ${spec.name}`)
      await write(
        `components/${base}/${base}.figma.ts`,
        promotionMapping(spec, raw, base, app.config)
      )
    }
    for (const asset of assets)
      await write(
        `components/assets/${asset}`,
        await readMaterialized(path.join(root, REVIEW_GENERATED, 'components/assets', asset))
      )
    for (const file of runtimeFiles)
      await write(
        `components/runtime/${file}`,
        (await readText(path.join(root, REVIEW_RUNTIME, file))).replace(
          "from '../generated/icons'",
          "from '../../foundations/icons'"
        )
      )

    const source = app.createSource('snapshot')
    const [collections, textStyles, gradients, icons] = await Promise.all([
      source.getTokenCollections(),
      source.getTextStyles(),
      source.getGradients(),
      source.getIcons()
    ])
    for (const file of generateFoundations({ collections, textStyles, gradients }))
      await write(`foundations/${file.path}`, file.content)
    for (const file of generateIcons(icons, {}).files)
      await write(`foundations/${file.path}`, file.content)

    await declareCssModules(temporary)

    const entry = publicEntry(
      currentEntry,
      superseded,
      selected
        .map((spec) => output.manifest.find((item) => item.name === spec.name)!)
        .map((item) => {
          const base = path.basename(item.file)
          return `export { ${item.name} } from './components/${base}/${base}'\nexport type { ${item.name}Props } from './components/${base}/${base}'`
        })
    )

    // Unchanged only when the record, the public entry and every promoted file are byte-identical
    // to what this run produced, so a promoter change (not just a new seal) re-promotes.
    const record =
      JSON.stringify({ schema: 1, files: manifest.files, components: names }, null, 2) + '\n'
    const stagedFiles = await listFiles(temporary)
    const promotedDirs = [
      ...new Set(stagedFiles.filter((file) => file.startsWith('components/')).map(promotedDir))
    ]
    const existingFiles = (
      await Promise.all(promotedDirs.map((dir) => listFiles(path.join(root, dir), dir)))
    ).flat()
    const sameFiles =
      JSON.stringify(stagedFiles.filter((file) => file.startsWith('components/')).sort()) ===
        JSON.stringify(existingFiles.sort()) &&
      (
        await Promise.all(
          stagedFiles.map(async (file) =>
            (await readFile(staged(file))).equals(
              await readFile(path.join(root, file)).catch(() => Buffer.alloc(0))
            )
          )
        )
      ).every(Boolean)
    if (previous === record && entry === currentEntry && sameFiles)
      return { library: root, changed: false, components: names }

    const packageFile = path.join(root, 'package.json')
    const pkg = JSON.parse(await readFile(packageFile, 'utf8'))
    const [major, minor] = String(pkg.version).split('.').map(Number)
    pkg.version = `${major}.${minor! + 1}.0`

    // Publish only after every reviewed input verified and the whole output was staged.
    for (const name of previousNames) {
      const entryFor = output.manifest.find((item) => item.name === name)
      const directory = entryFor ? path.basename(entryFor.file) : name
      await rm(path.join(root, 'components', directory), { recursive: true, force: true })
    }
    // A superseded hand-written directory goes whole (tests and mapping included), so no stale file
    // survives next to the promoted one.
    for (const directory of superseded)
      await rm(path.join(root, 'components', directory), { recursive: true, force: true })
    for (const generated of ['components/assets', 'components/runtime', 'foundations/icons'])
      await rm(path.join(root, generated), { recursive: true, force: true })
    for (const item of await readdir(temporary))
      await cp(staged(item), path.join(root, item), { recursive: true })
    await writeFile(entryFile, entry)
    await writeFile(packageFile, JSON.stringify(pkg, null, 2) + '\n')
    await writeFile(path.join(root, RECORD), record)
    return {
      components: names,
      excluded: manifest.excluded,
      superseded,
      library: root,
      version: pkg.version
    }
  } finally {
    await rm(temporary, { recursive: true, force: true })
  }
}

export { promoteReviewed, publicEntry, relocate, supersededDirectories, supersession }
