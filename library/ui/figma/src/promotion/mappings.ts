import { mapVariants } from '../mappers/variants.js'
import { expandVariants } from '../capture/expand-variants.js'
import type { FigmaConfig } from '../config.js'
import type { ComponentSpec } from '../ir/components.js'
import type { RawComponent } from '../ir/raw-components.js'

function promotionMapping(
  spec: ComponentSpec,
  raw: RawComponent,
  file: string,
  config: FigmaConfig
) {
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
    const values: Record<string, string> = {}
    for (const variant of variants) {
      const view = mapped.variants.find((view) => view.figmaId === variant.id)
      if (!view) continue
      const input = variant.props[prop.source]
      const output =
        view.values[prop.name] ??
        view.booleans[prop.name] ??
        view.flags[prop.name as keyof typeof view.flags]
      if (input === undefined || output === undefined) continue
      values[String(input)] =
        typeof output === 'boolean' ? (output ? ` ${prop.name}` : '') : ` ${prop.name}="${output}"`
    }
    for (const [input, attribute] of Object.entries(values)) {
      const sourceVariant = variants.find((item) => String(item.props[prop.source]) === input)
      const view = mapped.variants.find((item) => item.figmaId === sourceVariant?.id)
      if (view?.flags.disabled && !attribute.includes(' disabled')) values[input] += ' disabled'
      if (view?.interaction && view.interaction !== 'base')
        values[input] += ` forceState="${view.interaction}"`
    }
    const options = raw.properties[prop.source]!.options ?? []
    if (options.some((option) => !(option in values)))
      throw new Error(`Cannot promote incomplete property mapping: ${spec.name}.${prop.source}`)
    const local = `property${index}`
    locals.push(
      `const ${local} = instance.getEnum(${JSON.stringify(prop.source)}, { ${Object.entries(values)
        .map(([key, value]) => `${JSON.stringify(key)}: '${value}'`)
        .join(', ')} })`
    )
    attributes.push('${' + local + '}')
    read.add(prop.source)
  }
  const states = Object.keys(raw.properties ?? {}).filter((name) => !read.has(name))
  return `// url=<DCL_FIGMA_FILE>?node-id=${spec.figmaId.replace(':', '-')}
// source=components/${file}/${file}.tsx
// component=${spec.name}
// contract=1
${states.length ? '// states=' + states.join(',') : ''}
import figma from 'figma'
${locals.length ? 'const instance = figma.selectedInstance' : ''}
${locals.join('\n')}
export default {
  example: figma.tsx\`<${spec.name}${attributes.join('')} />\`,
  imports: ["import { ${spec.name} } from '@dcl/one-ui'"],
  id: '${spec.name}',
  metadata: { nestable: true }
}
`
}
export { promotionMapping }
