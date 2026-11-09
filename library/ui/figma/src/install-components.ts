import type { FigmaConfig } from './config.js'
import { expandVariants } from './capture/expand-variants.js'
import { generateComponents } from './generators/components.js'
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
  registry: { components: readonly string[] }
  runtime: ReadonlyMap<string, Buffer>
}
type ComponentInstallPlan = { components: string[]; files: Map<string, Buffer> }

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

function cssDeclaration(content: string): string {
  const names = [...new Set([...content.matchAll(/\.([a-zA-Z_][\w-]*)/g)].map((m) => m[1]!))]
  return `declare const styles: {\n${names.map((name) => `  readonly ${literal(name)}: string`).join('\n')}\n}\nexport default styles\n`
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

/** Consumes verified inputs supplied by runInstall; it never verifies, publishes or writes files. */
async function planComponents(context: ComponentPlanContext): Promise<ComponentInstallPlan> {
  const { source, config, registry, runtime } = context
  if (config.imageFills !== 'placeholder')
    throw new Error('Public component installation requires the placeholder raster policy')
  if (
    Object.keys(registry).length !== 1 ||
    !Array.isArray(registry.components) ||
    !registry.components.length ||
    registry.components.some((name) => typeof name !== 'string') ||
    new Set(registry.components).size !== registry.components.length
  )
    throw new Error('Component registry must contain unique component names only')
  for (const name of registry.components) assertIdentifier(name)
  for (const file of runtimeFiles)
    if (!runtime.get(file)?.length) throw new Error(`Missing locked runtime input: ${file}`)
  const loaded = await loadSpecsFromSource(source, config, 'promotion')
  if (loaded.nameCollisions.length)
    throw new Error('Ambiguous component names in the pinned source')
  const available = new Map(loaded.specs.map((spec) => [spec.name, spec]))
  const selected = new Map<string, ComponentSpec>()
  const queue = [...registry.components]
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
  for (const [file, bytes] of [...files])
    if (file.endsWith('.module.css'))
      files.set(file + '.d.ts', Buffer.from(cssDeclaration(bytes.toString('utf8'))))
  const components = specs.map((spec) => spec.name)
  files.set('promotion.json', Buffer.from(JSON.stringify({ components }, null, 2) + '\n'))
  return { components, files }
}

export { planComponents }
export type { ComponentPlanContext, ComponentInstallPlan }
