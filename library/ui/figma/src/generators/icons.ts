import type { IconSpec } from '../ir/schemas.js'
import { assertIdentifier, pascalCase, uniqueName } from './naming.js'
import { renderValue, header, type Renderable } from './render.js'
import { optimizeIcon, type OptimizedIcon } from './svg-to-jsx.js'
import type { GeneratedFile, GeneratorContext } from './types.js'

type IconMeta = {
  component: string
  name: string
  category: string
  section: string
  width: number
  height: number
  monochrome: boolean
  hasActiveState: boolean
}

type SkippedIcon = { name: string; reason: string }

const ACTIVE_ON = 'active=true'
const ACTIVE_OFF = 'active=false'

const TYPES_TS = () => `${header()}import type { SVGProps } from 'react'

export type IconProps = Omit<SVGProps<SVGSVGElement>, 'width' | 'height'> & {
  size?: number | string
}

export type ActiveIconProps = IconProps & { active?: boolean }
`

function groupByName(icons: IconSpec[]): Map<string, IconSpec[]> {
  const groups = new Map<string, IconSpec[]>()
  for (const icon of icons) {
    const list = groups.get(icon.name) ?? []
    list.push(icon)
    groups.set(icon.name, list)
  }
  return groups
}

function svgElement(icon: IconSpec, optimized: OptimizedIcon, depth: number): string {
  const pad = '  '.repeat(depth)
  const size = (value: number) => `size ?? ${value}`
  return [
    `${pad}<svg`,
    `${pad}  width={${size(icon.width)}}`,
    `${pad}  height={${size(icon.height)}}`,
    `${pad}  viewBox="${optimized.viewBox}"`,
    `${pad}  fill="none"`,
    `${pad}  xmlns="http://www.w3.org/2000/svg"`,
    `${pad}  {...props}`,
    `${pad}>`,
    optimized.jsx,
    `${pad}</svg>`
  ].join('\n')
}

function componentSource(
  name: string,
  variants: { icon: IconSpec; optimized: OptimizedIcon }[]
): string {
  const active = variants.find((v) => v.icon.variant === ACTIVE_ON)
  const idle = variants.find((v) => v.icon.variant === ACTIVE_OFF)

  if (active && idle) {
    return `export const ${name} = ({ size, active = false, ...props }: ActiveIconProps) =>
  active ? (
${svgElement(active.icon, active.optimized, 2)}
  ) : (
${svgElement(idle.icon, idle.optimized, 2)}
  )
`
  }
  const [only] = variants
  if (variants.length !== 1 || !only || only.icon.variant !== null) {
    throw new Error(
      `Unsupported variants for ${name}: ${variants.map((v) => v.icon.variant).join(', ')}`
    )
  }
  return `export const ${name} = ({ size, ...props }: IconProps) => (
${svgElement(only.icon, only.optimized, 1)}
)
`
}

function generateIcons(icons: IconSpec[], context: GeneratorContext) {
  void context
  const skipped: SkippedIcon[] = []
  const usable = icons.filter((icon) => {
    if (icon.svg.includes('<image')) {
      skipped.push({ name: icon.name, reason: 'embeds a raster image' })
      return false
    }
    return true
  })

  const usedNames = new Set<string>()
  const meta: IconMeta[] = []
  const byCategory = new Map<string, string[]>()

  for (const [name, variants] of groupByName(usable)) {
    const isSingle = variants.length === 1 && variants[0]!.variant === null
    const isActivePair =
      variants.length === 2 &&
      variants.some((icon) => icon.variant === ACTIVE_ON) &&
      variants.some((icon) => icon.variant === ACTIVE_OFF)
    if (!isSingle && !isActivePair) {
      skipped.push({
        name,
        reason: `unsupported or ambiguous variants: ${variants.map((icon) => `${icon.nodeId} (${icon.variant ?? 'no variant'})`).join(', ')}`
      })
      continue
    }
    const component = uniqueName(assertIdentifier(pascalCase(name)), usedNames)
    const optimized = variants.map((icon) => ({
      icon,
      optimized: optimizeIcon(icon.svg, component, variants.length > 1 ? 3 : 2)
    }))
    const first = variants[0]!
    assertIdentifier(first.category)
    const source = componentSource(component, optimized)
    const bucket = byCategory.get(first.category) ?? []
    bucket.push(source)
    byCategory.set(first.category, bucket)
    meta.push({
      component,
      name,
      category: first.category,
      section: first.section,
      width: first.width,
      height: first.height,
      monochrome: optimized.every((v) => v.optimized.monochrome),
      hasActiveState: variants.some((icon) => icon.variant === ACTIVE_ON)
    })
  }

  const files: GeneratedFile[] = [{ path: 'icons/types.ts', content: TYPES_TS() }]

  for (const [category, sources] of byCategory) {
    const uses = sources.join('\n')
    const imports = [
      uses.includes('ActiveIconProps') ? 'ActiveIconProps' : null,
      uses.includes(': IconProps') ? 'IconProps' : null
    ].filter(Boolean)
    files.push({
      path: `icons/${category}.tsx`,
      content: `${header()}import type { ${imports.join(', ')} } from './types'\n\n${uses}`
    })
  }

  const categories = [...byCategory.keys()]
  const componentNames = meta.map((m) => m.component)
  files.push({
    path: 'icons/iconMeta.ts',
    content: [
      header(),
      `export type IconMeta = {
  component: string
  name: string
  category: string
  section: string
  width: number
  height: number
  monochrome: boolean
  hasActiveState: boolean
}
`,
      `export const iconMeta: IconMeta[] = ${renderValue(meta as unknown as Renderable)}\n`,
      `export const skippedIcons: { name: string; reason: string }[] = ${renderValue(skipped as unknown as Renderable)}\n`
    ].join('\n')
  })
  files.push({
    path: 'icons/index.ts',
    content: [
      header(),
      `import type { ComponentType } from 'react'`,
      ...categories.map((category) => `import * as ${category} from './${category}'`),
      `import type { ActiveIconProps } from './types'\n`,
      ...categories.map((category) => `export * from './${category}'`),
      `export * from './iconMeta'`,
      `export type * from './types'\n`,
      `export const iconComponents: Record<string, ComponentType<ActiveIconProps>> = {`,
      ...categories.flatMap((category) =>
        meta
          .filter((m) => m.category === category)
          .map((m) => `  ${m.component}: ${category}.${m.component},`)
      ),
      `}\n`,
      `export const iconComponentNames = ${renderValue(componentNames)}\n`
    ].join('\n')
  })

  return { files, meta, skipped }
}

export { generateIcons }
export type { IconMeta, SkippedIcon }
