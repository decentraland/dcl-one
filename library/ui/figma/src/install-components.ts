import { posix } from 'node:path'
import { componentRegistryEntries } from '../component-generation.mjs'
import postcss from 'postcss'
import postcssModules from 'postcss-modules'
import type { FigmaConfig } from './config.js'
import { expandVariants } from './capture/expand-variants.js'
import { generateComponents } from './generators/components.js'
import { generateFoundations } from './generators/tokens.js'
import { assertIdentifier } from './generators/naming.js'
import type { ComponentSpec, ViewNode } from './ir/components.js'
import type { RawComponent } from './ir/raw-components.js'
import { mapVariants } from './mappers/variants.js'
import { configuredPages, loadPageSnapshot } from './page-identity.js'
import { promotionExclusion, runtimeFiles } from './promotion/eligibility.js'
import type { FigmaSource } from './sources/types.js'
import { loadSpecsFromSource } from './specs.js'

type ComponentPlanContext = {
  source: FigmaSource
  config: FigmaConfig
  // Public entries are names or { name, limits }; internal entries are names.
  registry: { components: readonly unknown[]; internal?: readonly string[] }
  runtime: ReadonlyMap<string, Buffer>
}
type ComponentInstallPlan = {
  components: string[]
  files: Map<string, Buffer>
}

const literal = (value: unknown) => JSON.stringify(value)

function mappingLiteral(value: string): string {
  if (/[\\\r\n\u2028\u2029]/.test(value))
    throw new Error('Unsupported escaped string in component mapping')
  if (!value.includes("'")) return "'" + value + "'"
  if (!value.includes('"')) return '"' + value + '"'
  throw new Error('Unsupported quoted string in component mapping')
}

function references(node: ViewNode): string[] {
  return [...(node.ref ? [node.ref.component] : []), ...(node.children ?? []).flatMap(references)]
}

function relocate(content: string): string {
  return content
    .replace(/from '\.\.\/\.\.\/\.\.\/components\/(figma-[a-z-]+)'/g, `from '../runtime/$1'`)
    .replace(
      `import type { MatrixCase } from '../../../components/StateMatrix'`,
      `type MatrixCase = { key: string; props: Record<string, unknown>; states: ('base' | ForcedState)[] }`
    )
    .replace(/^import '\.\.\/[^/]+\/([A-Za-z0-9_$]+)'$/gm, `import '../$1/$1'`)
    .replace(
      /from '\.\.\/\.\.\/tokens\/text-styles\.module\.css'/g,
      `from '../../foundations/typography/text-styles.module.css'`
    )
}

async function cssDeclarations(
  files: ReadonlyMap<string, Buffer>,
  foundations: string
): Promise<Map<string, Buffer>> {
  const stylesheets = new Map(
    [...files]
      .filter(([file]) => file.endsWith('.module.css'))
      .map(([file, bytes]) => ['/components/' + file, bytes.toString('utf8')])
  )
  stylesheets.set('/foundations/typography/text-styles.module.css', foundations)
  const cache = new Map<string, Record<string, string>>()
  const load = async (file: string, ancestors: string[] = []): Promise<Record<string, string>> => {
    if (ancestors.includes(file)) throw new Error('Recursive CSS module composition: ' + file)
    const cached = cache.get(file)
    if (cached) return cached
    const content = stylesheets.get(file)
    if (content === undefined) throw new Error('Unknown CSS module input: ' + file)
    let exports: Record<string, string> = {}
    class DeclarationLoader {
      fetch(specifier: string, relativeTo: string): Promise<Record<string, string>> {
        if (!specifier.startsWith('.'))
          throw new Error('Unsupported CSS module import: ' + specifier)
        return load(posix.resolve(posix.dirname(relativeTo), specifier), [...ancestors, file])
      }
    }
    await postcss([
      postcssModules({
        Loader: DeclarationLoader,
        getJSON: (_file, tokens) => {
          exports = tokens
        }
      })
    ]).process(content, { from: file })
    cache.set(file, exports)
    return exports
  }
  const declarations = new Map<string, Buffer>()
  for (const [file] of files) {
    if (!file.endsWith('.module.css')) continue
    const names = Object.keys(await load('/components/' + file))
    declarations.set(
      file + '.d.ts',
      Buffer.from(
        `declare const styles: {\n${names.map((name) => `  readonly ${literal(name)}: string`).join('\n')}\n}\nexport default styles\n`
      )
    )
  }
  return declarations
}

function componentMapping(spec: ComponentSpec, raw: RawComponent, config: FigmaConfig): string {
  if (!/^\d+:\d+$/.test(spec.figmaId)) throw new Error('Invalid captured component id')
  const variants = [raw.base, ...(raw.others ?? [])]
  const mapped = mapVariants(raw, expandVariants(raw), {
    wordAliases: Object.fromEntries(
      Object.entries(config.variantAliases).map(([key, value]) => [key.toLowerCase(), value])
    ),
    ...config.components[raw.name]
  })
  const locals: string[] = []
  const attributes: string[] = []
  const read = new Set<string>()
  for (const [index, prop] of spec.props.entries()) {
    if (!raw.properties?.[prop.source]) continue
    assertIdentifier(prop.name)
    const values: Record<string, string> = {}
    for (const variant of variants) {
      const view = mapped.variants.find((entry) => entry.figmaId === variant.id)
      if (!view) continue
      const input = variant.props[prop.source]
      const output =
        view.values[prop.name] ??
        view.booleans[prop.name] ??
        view.flags[prop.name as keyof typeof view.flags]
      if (input === undefined || output === undefined) continue
      let attribute = ` ${prop.name}={${literal(output)}}`
      if (view.flags.disabled && prop.name !== 'disabled') attribute += ' disabled'
      if (view.interaction && view.interaction !== 'base')
        attribute += ` forceState={${literal(view.interaction)}}`
      values[String(input)] = attribute
    }
    if (
      (raw.properties[prop.source]!.options ?? []).some((option) => !Object.hasOwn(values, option))
    )
      throw new Error(`Incomplete component mapping: ${spec.name}.${prop.source}`)
    const local = `property${index}`
    locals.push(
      `const ${local} = instance.getEnum(${mappingLiteral(prop.source)}, { ${Object.entries(values)
        .map(([key, value]) => `${mappingLiteral(key)}: ${mappingLiteral(value)}`)
        .join(', ')} })`
    )
    attributes.push('${' + local + '}')
    read.add(prop.source)
  }
  const states = Object.keys(raw.properties ?? {}).filter((name) => !read.has(name))
  if (states.some((name) => /[\r\n\u2028\u2029,]/.test(name)))
    throw new Error('Unsupported state name in component mapping')
  const stateComment = states.length ? '// states=' + states.join(',') : ''
  return `// url=<DCL_FIGMA_FILE>?node-id=${spec.figmaId.replace(':', '-')}
// source=components/${spec.name}/${spec.name}.tsx
// component=${spec.name}
// contract=1
${stateComment}
import figma from 'figma'
${locals.length ? 'const instance = figma.selectedInstance' : ''}
${locals.join('\n')}
export default {
  example: figma.tsx\`<${spec.name}${attributes.join('')} />\`,
  imports: [${literal(`import { ${spec.name} } from '@dcl/one-ui'`)}],
  id: ${literal(spec.name)},
  metadata: { nestable: true }
}
`
}

type AcceptedLimit = { id: string; text: string }

// The registry the install writes back: public names keep their accepted limits, in the { name, limits } form.
function registryBytes(
  components: readonly string[],
  publicNames: ReadonlySet<string>,
  limits: Readonly<Record<string, readonly AcceptedLimit[]>> = {}
): Buffer {
  const visibility = {
    components: components
      .filter((name) => publicNames.has(name))
      .map((name) => (Object.hasOwn(limits, name) ? { name, limits: limits[name] } : name)),
    ...(components.some((name) => !publicNames.has(name))
      ? { internal: components.filter((name) => !publicNames.has(name)) }
      : {})
  }
  return Buffer.from(JSON.stringify(visibility, null, 2) + '\n')
}

/** Consumes verified inputs supplied by runInstall; it never verifies, publishes or writes files. */
async function planComponents(context: ComponentPlanContext): Promise<ComponentInstallPlan> {
  const { source, config, registry, runtime } = context
  if (config.imageFills !== 'placeholder')
    throw new Error('Public component installation requires the placeholder raster policy')
  let entries: { components: string[]; internal: string[]; limits: Record<string, AcceptedLimit[]> }
  try {
    entries = componentRegistryEntries(registry)
  } catch (error) {
    throw new Error(
      'Component registry must contain public and internal name lists only: ' +
        (error as Error).message,
      { cause: error }
    )
  }
  const requested = [...entries.components, ...entries.internal]
  if (
    requested.some((name) => typeof name !== 'string') ||
    new Set(requested).size !== requested.length
  )
    throw new Error('Component registry must contain unique component names only')
  for (const name of requested) assertIdentifier(name)
  const publicNames = new Set(entries.components)
  if (!requested.length)
    return { components: [], files: new Map([['promotion.json', registryBytes([], publicNames)]]) }
  for (const file of runtimeFiles)
    if (!runtime.get(file)?.length) throw new Error(`Missing locked runtime input: ${file}`)
  const loaded = await loadSpecsFromSource(source, config, 'promotion')
  if (loaded.nameCollisions.length)
    throw new Error('Ambiguous component names in the pinned source')
  const available = new Map(loaded.specs.map((spec) => [spec.name, spec]))
  const selected = new Map<string, ComponentSpec>()
  const queue = [...requested]
  while (queue.length) {
    const name = queue.shift()!
    if (selected.has(name)) continue
    assertIdentifier(name)
    const spec = available.get(name)
    if (!spec) throw new Error(`Unknown component or missing dependency: ${name}`)
    if (promotionExclusion(spec).length) throw new Error(`Component is not eligible: ${name}`)
    selected.set(name, spec)
    queue.push(...spec.views.flatMap((view) => references(view.root)))
  }
  const visiting = new Set<string>()
  const visited = new Set<string>()
  const visit = (name: string) => {
    if (visiting.has(name)) throw new Error(`Recursive component dependency: ${name}`)
    if (visited.has(name)) return
    visiting.add(name)
    for (const dependency of selected.get(name)!.views.flatMap((view) => references(view.root)))
      visit(dependency)
    visiting.delete(name)
    visited.add(name)
  }
  for (const name of selected.keys()) visit(name)
  const specs = [...selected.values()].sort((a, b) => a.name.localeCompare(b.name, 'en'))
  const output = generateComponents(specs, { imageFills: config.imageFills })
  const generated = new Map(output.files.map((file) => [file.path, file.content]))
  const files = new Map<string, Buffer>()
  const assets = new Set<string>()
  for (const spec of specs) {
    const entry = output.manifest.find((item) => item.name === spec.name)!
    for (const extension of ['.tsx', '.module.css']) {
      const content = generated.get(`components/${entry.file}${extension}`)
      if (content === undefined) throw new Error(`Missing generated component: ${spec.name}`)
      if (extension === '.module.css')
        for (const match of content.matchAll(/url\(\x22\.\.\/assets\/([^\x22)]+)\x22\)/g))
          assets.add(match[1]!)
      files.set(`${spec.name}/${spec.name}${extension}`, Buffer.from(relocate(content)))
    }
    if (!publicNames.has(spec.name)) continue
    const pages = configuredPages(config).filter((page) => page.name === spec.page)
    if (pages.length !== 1) throw new Error(`Ambiguous component page: ${spec.page}`)
    const page = await loadPageSnapshot(source, pages[0]!)
    const raw = page?.components.find((item) => item.id === spec.figmaId)
    if (!raw) throw new Error(`Missing captured component mapping: ${spec.name}`)
    files.set(
      `${spec.name}/${spec.name}.figma.ts`,
      Buffer.from(componentMapping(spec, raw, config))
    )
  }
  for (const name of [...assets].sort()) {
    if (!/^[a-f0-9]{16}\.svg$/.test(name)) throw new Error('Unsafe generated asset path')
    const content = generated.get(`components/assets/${name}`)
    if (content === undefined) throw new Error(`Missing referenced component asset: ${name}`)
    files.set(`assets/${name}`, Buffer.from(content))
  }
  for (const file of runtimeFiles)
    files.set(
      `runtime/${file}`,
      Buffer.from(
        runtime
          .get(file)!
          .toString('utf8')
          .replaceAll(`from '../generated/icons'`, `from '../../foundations/icons'`)
      )
    )
  const [collections, textStyles, gradients] = await Promise.all([
    source.getTokenCollections(),
    source.getTextStyles(),
    source.getGradients()
  ])
  const foundations = generateFoundations({
    collections,
    textStyles,
    gradients,
    excludedPrimitives: config.excludedPrimitives
  })
  const textStyleModule = foundations.find(
    (file) => file.path === 'typography/text-styles.module.css'
  )
  if (!textStyleModule) throw new Error('Missing generated typography CSS module')
  for (const [file, bytes] of await cssDeclarations(files, textStyleModule.content))
    files.set(file, bytes)
  const components = specs.map((spec) => spec.name)
  files.set('promotion.json', registryBytes(components, publicNames, entries.limits))
  return { components, files }
}

export { planComponents, cssDeclarations }
export type { ComponentPlanContext, ComponentInstallPlan }
