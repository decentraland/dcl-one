import { createHash } from 'node:crypto'
import type {
  ComponentSpec,
  CssBlock,
  Semantic,
  StateRule,
  ViewNode,
  ViewSpec
} from '../ir/components.js'
import { viewLoop, viewMotion, type StateExtras } from './motion.js'
import { assertIdentifier, kebabCase, textStyleClass } from './naming.js'
import { inLayer } from './css-layers.js'
import { header } from './render.js'
import { replaceRasterFills } from './image-fills.js'
import { unsafeSvgReason } from './svg-safety.js'
import type { GeneratedFile, GeneratorContext, ImageFills } from './types.js'

type ManifestEntry = {
  name: string
  page: string
  category: string
  figmaName: string
  figmaId: string
  file: string
  props: string[]
  views: number
  needsManual: string[]
  notes: string[]
}

const FLAG_PROPS = ['selected', 'disabled', 'loading']
const quote = (value: string) =>
  `'${value
    .replace(/\\/g, '\\\\')
    .replace(/'/g, "\\'")
    .replace(/\r/g, '\\r')
    .replace(/\n/g, '\\n')
    .replace(/\u2028/g, '\\u2028')
    .replace(/\u2029/g, '\\u2029')}'`
const literal = (value: string) => JSON.stringify(value)
const pad = (depth: number) => '  '.repeat(depth)
const lowerFirst = (name: string) => name.charAt(0).toLowerCase() + name.slice(1)
const pathKey = (path: number[]) => path.join('.')

function cssLines(block: CssBlock, extra: [string, string][] = []): string[] {
  const lines: string[] = []
  if (block.textStyle)
    lines.push(
      `composes: ${textStyleClass(block.textStyle)} from '../../tokens/text-styles.module.css';`
    )
  for (const [property, value] of [...block.decls, ...extra]) lines.push(`${property}: ${value};`)
  if (block.after)
    lines.push(
      '&::after {',
      ...block.after.map(([property, value]) => `  ${property}: ${value};`),
      '}'
    )
  return lines
}

function stateBlock(
  selectors: string[],
  decls: [string, string][],
  starting: [string, string][] = []
): string[] {
  return [
    `${selectors.join(', ')} {`,
    ...decls.map(([property, value]) => `  ${property}: ${value};`),
    ...(starting.length > 0
      ? [
          '  @starting-style {',
          ...starting.map(([property, value]) => `    ${property}: ${value};`),
          '  }'
        ]
      : []),
    '}'
  ]
}

function stateCss(
  rules: StateRule[] | undefined,
  state: 'hover' | 'pressed',
  extras: StateExtras = new Map()
): string[] {
  if (!rules) return []
  const pseudo = state === 'hover' ? ':hover' : ':active'
  return rules.flatMap((rule) => {
    const key = pathKey(rule.path)
    const target = rule.path.length === 0 ? '' : ` [data-n='${key}']`
    const selectors = (suffix: string) => [
      `&${pseudo}:not(:disabled):not([data-interaction-mode='controlled'])${target}${suffix}`,
      `&[data-force-state='${state}']${target}${suffix}`
    ]
    const extra = extras.get(key)
    const decls = [...rule.decls, ...(extra?.decls ?? [])]
    const ring = extras.get(`${key}::after`)
    return [
      ...(decls.length > 0 || extra?.starting
        ? stateBlock(selectors(''), decls, extra?.starting)
        : []),
      ...(rule.after
        ? stateBlock(selectors('::after'), [...rule.after, ...(ring?.decls ?? [])])
        : [])
    ]
  })
}

// Props that opt-in semantics add to a component: link targets and activation handlers.
type SemanticProps = { hrefs: Set<string>; handlers: Set<string> }
type SemanticContext = {
  semantics: Record<string, Semantic>
  props: SemanticProps
  roles: Map<string, NonNullable<ComponentSpec['textRoles']>[number]>
}

function firstSlot(node: ViewNode): string | undefined {
  if (node.kind === 'text') return node.slot
  for (const child of node.children ?? []) {
    const slot = firstSlot(child)
    if (slot) return slot
  }
}

// Semantic href and handler names come from figma.config.json, so they follow the shared identifier rule too.
function semanticProp(template: string, node: ViewNode): string {
  if (!template.includes('{slot}')) return assertIdentifier(template)
  const slot = firstSlot(node)
  if (!slot) throw new Error(`Semantic prop ${template} needs a text slot inside ${node.name}`)
  return assertIdentifier(template.replace('{slot}', slot))
}

// A link around text bound to a role takes exactly the prop name the role declares, never one built from the role.
function roleHref(node: ViewNode, context: SemanticContext): string | undefined {
  const slot = firstSlot(node)
  const role = slot ? context.roles.get(slot) : undefined
  if (!role) return undefined
  if (!role.href)
    throw new Error(
      `Text role ${role.name} sits under the linked action ${node.name}: declare its href in textRoles`
    )
  return role.href
}

const ROOT_SEMANTICS = '@root'

function semanticFor(node: ViewNode, context: SemanticContext): Semantic | undefined {
  if (node.kind !== 'box') return undefined
  return context.semantics[node.path.length === 0 ? ROOT_SEMANTICS : node.name]
}

function hasSemanticDescendant(node: ViewNode, context: SemanticContext): boolean {
  return (node.children ?? []).some(
    (child) => semanticFor(child, context) !== undefined || hasSemanticDescendant(child, context)
  )
}

function emitSemantic(node: ViewNode, context: SemanticContext): string | undefined {
  const config = semanticFor(node, context)
  if (!config) return undefined
  // An action never wraps another semantic node: interactive elements cannot nest, and a name
  // shared by a wrapper and a leaf (a logo frame that also holds the tabs) resolves to the leaf.
  if (config.role === 'action' && hasSemanticDescendant(node, context)) return undefined
  const resolved: Record<string, string | boolean> = { role: config.role }
  if (config.label) resolved.label = config.label
  if (config.href) {
    resolved.href = roleHref(node, context) ?? semanticProp(config.href, node)
    context.props.hrefs.add(resolved.href)
  }
  if (config.onActivate) {
    resolved.onActivate = semanticProp(config.onActivate, node)
    context.props.handlers.add(resolved.onActivate)
  }
  if (config.listItem) resolved.listItem = true
  if (config.fallback) resolved.fallback = config.fallback
  return JSON.stringify(resolved)
}

function emitNode(
  node: ViewNode,
  depth: number,
  classPrefix: string,
  cssRules: string[],
  extraRoot: string[] = [],
  semantic?: SemanticContext
): string {
  const lines: string[] = [`${pad(depth)}{`]
  const inner = pad(depth + 1)
  const extra: [string, string][] =
    node.kind === 'icon' && node.icon?.color ? [['color', node.icon.color]] : []
  lines.push(`${inner}kind: ${quote(node.kind)},`)
  lines.push(`${inner}path: ${quote(pathKey(node.path))},`)
  lines.push(`${inner}name: ${literal(node.name)},`)
  const className = `${classPrefix}n${node.path.join('_') || 'root'}`
  cssRules.push(`.${className} {\n${[...cssLines(node.css, extra), ...extraRoot].join('\n')}\n}`)
  lines.push(`${inner}className: styles.${className},`)
  if (node.textRuns) {
    lines.push(`${inner}textRuns: [`)
    node.textRuns.forEach((run, index) => {
      const runClass = `${className}r${index}`
      cssRules.push(`.${runClass} {\n${cssLines(run.css).join('\n')}\n}`)
      lines.push(`${inner}  { text: ${literal(run.text)}, className: styles.${runClass} },`)
    })
    lines.push(`${inner}],`)
  }
  if (node.text !== undefined) lines.push(`${inner}text: ${literal(node.text)},`)
  if (node.slot) lines.push(`${inner}slot: ${quote(node.slot)},`)
  if (node.icon) {
    const props = JSON.stringify(node.icon.props)
    lines.push(
      `${inner}icon: { component: ${quote(node.icon.component)}, props: ${props}, size: ${node.icon.size} },`
    )
  }
  if (node.ref) {
    lines.push(
      `${inner}ref: { component: ${quote(node.ref.component)}, props: ${JSON.stringify(node.ref.props)} },`
    )
  }
  if (node.glyph) {
    lines.push(
      `${inner}glyph: { name: ${literal(node.glyph.name)}, reason: ${literal(node.glyph.reason)} },`
    )
  }
  const semanticLine = semantic ? emitSemantic(node, semantic) : undefined
  if (semanticLine) lines.push(`${inner}semantic: ${semanticLine},`)
  if (node.children && node.children.length > 0) {
    lines.push(`${inner}children: [`)
    lines.push(
      node.children
        .map((child) => emitNode(child, depth + 2, classPrefix, cssRules, [], semantic))
        .join(',\n')
    )
    lines.push(`${inner}],`)
  }
  lines.push(`${pad(depth)}}`)
  return lines.join('\n')
}

function emitView(
  view: ViewSpec,
  index: number,
  cssRules: string[],
  semantic?: SemanticContext
): string {
  const motion = viewMotion(view)
  const loop = viewLoop(view, `v${index}`)
  const rootExtra = [
    ...motion.rest,
    ...loop.rest,
    ...stateCss(view.hover, 'hover', motion.states.hover),
    ...stateCss(view.pressed, 'pressed', motion.states.pressed)
  ]
  const emitted = emitNode(view.root, 1, `v${index}`, cssRules, rootExtra, semantic).trimStart()
  cssRules.push(...loop.keyframes)
  return `  ${quote(view.key)}: ${emitted}`
}

function propType(prop: ComponentSpec['props'][number]): string {
  return prop.kind === 'boolean' ? 'boolean' : prop.values.map((value) => quote(value)).join(' | ')
}

function caseProps(view: ViewSpec): Record<string, string | boolean> {
  const props: Record<string, string | boolean> = { ...view.values, ...view.booleans }
  for (const [flag, on] of Object.entries(view.flags)) if (on) props[flag] = true
  return props
}

// Sparse axes are a relation, not independent options. Interaction flags keep their existing
// fallback contract; only captured enum and non-flag boolean combinations constrain the API.
// A caller that omits every axis prop gets the defaults, so that selection must be a drawn view: the runtime throws
// for any undrawn one, and a default the generator itself produces must never be one.
function assertDrawnDefaults(spec: ComponentSpec): void {
  const axes = spec.props.filter((prop) => !FLAG_PROPS.includes(prop.name))
  const drawn = spec.views.some((view) =>
    axes.every((prop) =>
      prop.kind === 'boolean'
        ? (view.booleans[prop.name] ?? false) === prop.default
        : (view.values[prop.name] ?? prop.default) === prop.default
    )
  )
  if (!drawn)
    throw new Error(
      `${spec.name}: its default selection (${axes.map((prop) => `${prop.name}=${JSON.stringify(prop.default)}`).join(', ')}) is not a captured view`
    )
}

function variantContract(spec: ComponentSpec): string | undefined {
  const axes = spec.props.filter((prop) => !FLAG_PROPS.includes(prop.name))
  const rows = [
    ...new Set(
      spec.views.map((view) =>
        JSON.stringify(
          axes.map((prop) =>
            prop.kind === 'boolean'
              ? (view.booleans[prop.name] ?? false)
              : (view.values[prop.name] ?? prop.default)
          )
        )
      )
    )
  ].map((row) => JSON.parse(row) as (string | boolean)[])
  const product = axes.reduce(
    (count, prop) => count * (prop.kind === 'boolean' ? 2 : prop.values.length),
    1
  )
  if (rows.length === product || axes.length === 0) return undefined
  return [
    '(',
    ...rows.map(
      (row) =>
        `  | { ${axes
          .map((prop, index) => {
            const value = row[index]!
            return `${prop.name}${value === prop.default ? '?' : ''}: ${typeof value === 'boolean' ? value : literal(value)}`
          })
          .join('; ')} }`
    ),
    ')'
  ].join('\n')
}

function labelNode(node: ViewNode): ViewNode | undefined {
  if (node.kind === 'text' && node.slot === 'label') return node
  for (const child of node.children ?? []) {
    const label = labelNode(child)
    if (label) return label
  }
}

function labelColor(node: ViewNode): string | undefined {
  return labelNode(node)?.css.decls.find(([property]) => property === 'color')?.[1]
}

function buttonControlView(
  view: ViewSpec,
  color: string | undefined,
  restingLabel?: ViewNode
): ViewSpec {
  const labels = new Set<string>()
  const root = (input: ViewNode): ViewNode => {
    let node = input
    if (view.flags.loading && node.name === 'label' && restingLabel)
      node = {
        ...restingLabel,
        path: node.path,
        css: {
          ...restingLabel.css,
          decls: [
            ...restingLabel.css.decls.filter(([property]) => property !== 'opacity'),
            ['opacity', '0']
          ]
        }
      }
    if (view.flags.loading && node.name === 'Spinner') {
      const background = node.css.decls.find(([property]) => property === 'background')?.[1]
      node = {
        ...node,
        css: {
          ...node.css,
          decls: [
            ...node.css.decls.filter(
              ([property]) =>
                !['left', 'top', 'transform', ...(background ? ['background'] : [])].includes(
                  property
                )
            ),
            ...(background
              ? ([
                  ['mask', background],
                  ['background-color', 'currentColor']
                ] as [string, string][])
              : []),
            ['left', '50%'],
            ['top', '50%'],
            ['transform', 'translate(-50%, -50%)']
          ]
        }
      }
    }
    const label = node.kind === 'text' && node.slot === 'label'
    if (label) labels.add(pathKey(node.path))
    const naturalWidth = node.path.length === 0 || label
    return {
      ...node,
      css: {
        ...node.css,
        decls: [
          ...node.css.decls.filter(([property]) => !naturalWidth || property !== 'width'),
          ...(node.path.length === 0 && color ? [['color', color] as [string, string]] : [])
        ]
      },
      ...(node.children ? { children: node.children.map(root) } : {})
    }
  }
  const controlRoot = root(view.root)
  const states = (rules: StateRule[] | undefined): StateRule[] | undefined =>
    rules?.flatMap((rule) => {
      const label = labels.has(pathKey(rule.path))
      const naturalWidth = rule.path.length === 0 || label
      const decls = rule.decls.filter(([property]) => !naturalWidth || property !== 'width')
      const foreground = label ? decls.filter(([property]) => property === 'color') : []
      return [{ ...rule, decls }, ...(foreground.length ? [{ path: [], decls: foreground }] : [])]
    })
  return { ...view, root: controlRoot, hover: states(view.hover), pressed: states(view.pressed) }
}

function emitComponent(
  spec: ComponentSpec,
  context: GeneratorContext,
  folders: Map<string, string>,
  cssRules: string[]
): string {
  const control = spec.control === 'button'
  const variantValues = spec.props.find((prop) => prop.name === 'variant')?.values ?? []
  const actionAliases = control
    ? Object.fromEntries(
        Object.entries({
          tertiary: 'outline',
          quaternary: 'dark',
          inverse: 'primary',
          iconOnly: 'primary'
        }).filter(
          ([alias, target]) => !variantValues.includes(alias) && variantValues.includes(target)
        )
      )
    : {}
  const extraVariants = Object.keys(actionAliases)
  const actionVariant = [
    ...Object.entries(actionAliases).map(
      ([alias, target]) => `variant === ${quote(alias)} ? ${quote(target)} : `
    ),
    'variant'
  ].join('')
  const varName = lowerFirst(spec.name)
  const enums = spec.props.filter((prop) => prop.kind === 'enum')
  const flagProps = spec.props.filter((prop) => FLAG_PROPS.includes(prop.name))
  const booleans = spec.props.filter(
    (prop) => prop.kind === 'boolean' && !FLAG_PROPS.includes(prop.name)
  )
  // Text roles are public typed props even when the positional text overrides are not.
  const roles = new Map((spec.textRoles ?? []).map((role) => [role.name, role]))
  const countRoles = (spec.textRoles ?? []).filter((role) => role.type === 'count')
  const labelRoles = countRoles.map((role) => role.label!)
  const slots = spec.slots.filter(
    (slot) => spec.exposeTextSlots !== false || slot.kind !== 'text' || roles.has(slot.name)
  )
  const omittedTextSlots = spec.slots.filter((slot) => !slots.includes(slot))
  const reactNodeSlots = slots.filter((slot) => !roles.has(slot.name))
  const countContract = countRoles
    .map(
      (role) =>
        `({ ${role.name}?: undefined; ${role.label}?: string } | { ${role.name}: number; ${role.label}: string })`
    )
    .join(' & ')
  assertDrawnDefaults(spec)
  const contract = variantContract(spec)

  const refImports = [
    ...new Set(
      spec.views.flatMap((view) => {
        const found: string[] = []
        const visit = (node: ViewNode) => {
          if (node.ref) found.push(node.ref.component)
          node.children?.forEach(visit)
        }
        visit(view.root)
        return found
      })
    )
  ].filter((name) => name !== spec.name)

  // Components without opt-in semantics emit exactly what they did before.
  const semantic: SemanticContext | undefined = spec.semantics
    ? {
        semantics: spec.semantics,
        props: { hrefs: new Set(), handlers: new Set() },
        roles: new Map((spec.textRoles ?? []).map((role) => [role.name, role]))
      }
    : undefined
  const views = spec.views.map((view, index) => {
    if (!control) return emitView(view, index, cssRules, semantic)
    const color =
      labelColor(view.root) ??
      spec.views
        .filter((candidate) =>
          Object.entries(view.values).every(([axis, value]) => candidate.values[axis] === value)
        )
        .map((candidate) => labelColor(candidate.root))
        .find(Boolean)
    const restingLabel = view.flags.loading
      ? spec.views
          .filter(
            (candidate) =>
              !candidate.flags.loading &&
              !candidate.flags.disabled &&
              Object.entries(view.values).every(([axis, value]) => candidate.values[axis] === value)
          )
          .map((candidate) => labelNode(candidate.root))
          .find(Boolean)
      : undefined
    return emitView(buttonControlView(view, color, restingLabel), index, cssRules, semantic)
  })
  const hrefProps = [...(semantic?.props.hrefs ?? [])].sort()
  const handlerProps = [...(semantic?.props.handlers ?? [])].sort()
  const semanticNames = [...hrefProps, ...handlerProps]

  const typeLines = [
    ...spec.props
      .filter((prop) => !contract || FLAG_PROPS.includes(prop.name))
      .map(
        (prop) =>
          `  ${prop.name}?: ${propType(prop)}${control && prop.name === 'variant' ? extraVariants.map((name) => ` | ${quote(name)}`).join('') : ''}`
      ),
    ...slots
      .map((slot) => {
        const role = roles.get(slot.name)
        return role?.type === 'count' ? null : `  ${slot.name}?: ${role ? 'string' : 'ReactNode'}`
      })
      .filter((line) => line !== null),
    ...omittedTextSlots.map((slot) => `  ${slot.name}?: never`),
    ...hrefProps.map((name) => `  ${name}?: string`),
    ...handlerProps.map((name) => `  ${name}?: () => void`),
    '  forceState?: ForcedState'
  ]
  const destructure = [
    ...spec.props.map(
      (prop) =>
        `${prop.name} = ${prop.kind === 'boolean' ? String(prop.default) : quote(String(prop.default))}`
    ),
    ...slots.map((slot) => slot.name),
    ...labelRoles,
    ...semanticNames,
    ...(control ? ['children', 'leadingIcon', 'trailingIcon', 'iconOnly'] : []),
    'forceState',
    '...rest'
  ]
  // Each binding of the parameter destructure is declared once: a role, a declared href and a semantic prop with the
  // same name would otherwise generate invalid code.
  const bindingNames = destructure.map((entry) => entry.replace(/^\.\.\./, '').split(' = ')[0]!)
  const repeated = bindingNames.find((name, index) => bindingNames.indexOf(name) !== index)
  if (repeated) throw new Error(`${spec.name}: generated prop ${repeated} is declared twice`)
  const selection = [
    `values: { ${enums.map((prop) => (control && prop.name === 'variant' ? `variant: ${actionVariant}` : prop.name)).join(', ')} }`,
    `booleans: { ${booleans.map((prop) => prop.name).join(', ')} }`,
    `flags: { ${flagProps.map((prop) => (control && prop.name === 'disabled' ? `disabled: ${flagProps.some((flag) => flag.name === 'loading') ? '!loading && ' : ''}(disabled || rest['aria-disabled'] === true || rest['aria-disabled'] === 'true')` : prop.name)).join(', ')} }`
  ].join(',\n      ')
  const disabled = flagProps.some((prop) => prop.name === 'disabled')
  const slotObject = slots.length > 0 ? `{ ${slots.map((slot) => slot.name).join(', ')} }` : '{}'

  const cases = spec.views.map((view) => {
    const states = [
      'base',
      ...(view.hover?.length ? ['hover'] : []),
      ...(view.pressed?.length ? ['pressed'] : [])
    ]
    return `  { key: ${quote(view.key)}, props: ${JSON.stringify(caseProps(view))}, states: ${JSON.stringify(states)} }`
  })

  const importLines = [
    ...(!control || reactNodeSlots.length > 0
      ? [
          `import type { ${control ? 'ReactNode' : `HTMLAttributes${reactNodeSlots.length > 0 ? ', ReactNode' : ''}`} } from 'react'`
        ]
      : []),
    `import styles from './${spec.name}.module.css'`,
    `import { FigmaView, registerComponent, type ForcedState, type ViewNode${control ? ', type ActionProps, type ControlContent, type ControlName, type IconOnlyContract' : ''} } from '../../../components/figma-module-runtime'`,
    "import type { MatrixCase } from '../../../components/StateMatrix'",
    ...refImports.map((name) => `import '../${folders.get(name) ?? pageFolder(spec.page)}/${name}'`)
  ]

  return [
    header().trimEnd(),
    `// Figma: ${commentText(spec.page)} / ${commentText(spec.figmaName)} (${commentText(spec.figmaId)})`,
    ...(spec.needsManual.length > 0
      ? [`// needs-manual: ${spec.needsManual.length} item(s), see componentManifest`]
      : []),
    ...importLines,
    '',
    `export type ${spec.name}Props = {`,
    ...typeLines,
    `}${contract ? ` & ${contract}` : ''}${countContract ? ` & ${countContract}` : ''} & ${control ? `ActionProps & ControlContent & IconOnlyContract${variantValues.length ? ` & ({ variant?: Exclude<${[...new Set([...variantValues, ...extraVariants])].map(quote).join(' | ')}, 'iconOnly'> } | ({ variant: 'iconOnly' } & ControlName))` : ''}` : "Omit<HTMLAttributes<HTMLElement>, 'children'>"}`,
    '',
    `export const ${varName}Views: Record<string, ViewNode> = {`,
    views.join(',\n'),
    '}',
    '',
    `export const ${varName}Cases: MatrixCase[] = [`,
    cases.join(',\n'),
    ']',
    '',
    // Text overrides the type forbids. They are a module constant, not a prop, so no caller can replace the list.
    ...(omittedTextSlots.length
      ? [
          `const ${varName}SuppressedProps = Object.freeze([${omittedTextSlots.map((slot) => quote(slot.name)).join(', ')}] as const)`,
          ''
        ]
      : []),
    `export const ${spec.name} = ({`,
    `  ${destructure.join(',\n  ')}`,
    `}: ${spec.name}Props) => (`,
    '  <FigmaView',
    `    as=${quote(spec.tag)}`,
    `    views={${varName}Views}`,
    `    component=${quote(spec.name)}`,
    '    select={{',
    `      ${selection}`,
    '    }}',
    `    slots={${slotObject}}`,
    ...(countRoles.length
      ? [`    counts={{ ${countRoles.map((role) => `${role.name}: ${role.label}`).join(', ')} }}`]
      : []),
    ...(semanticNames.length ? [`    semanticProps={{ ${semanticNames.join(', ')} }}`] : []),
    '    forceState={forceState}',
    ...(disabled ? ['    disabled={disabled}'] : []),
    ...(control && flagProps.some((prop) => prop.name === 'loading') ? ['    busy={loading}'] : []),
    // The caller's props go in as data, never spread over the generated configuration above.
    ...(control
      ? [
          `    action={{ ...rest, children, leadingIcon, trailingIcon, iconOnly, appearance: ${variantValues.length ? "variant === 'inverse' || variant === 'iconOnly' ? variant : undefined" : 'undefined'} }}`
        ]
      : ['    attributes={rest}']),
    ...(omittedTextSlots.length ? [`    suppressed={${varName}SuppressedProps}`] : []),
    '  />',
    ')',
    '',
    `registerComponent(${quote(spec.name)}, ${spec.name})`,
    ''
  ].join('\n')
}

function pageFolder(page: string): string {
  return kebabCase(page)
}

// Figma SVG exports of image fills embed their rasters, so inlining them as data URIs made the Review stylesheet
// exceed the 25 MiB Workers asset limit. Large ones become content-addressed files that bundlers fingerprint and
// deduplicate; small ones stay inline so the generated CSS remains readable and diffable.
const INLINE_SVG_LIMIT = 4096
const ASSET_DIRECTORY = 'components/assets'
function externalizeSvgs(css: string, assets: Map<string, string>): string {
  return css.replace(/url\("data:image\/svg\+xml,([^"]*)"\)/g, (match, encoded: string) => {
    const svg = decodeURIComponent(encoded)
    if (Buffer.byteLength(svg) <= INLINE_SVG_LIMIT) return match
    const unsafe = unsafeSvgReason(svg)
    if (unsafe) throw new Error(`Refusing to publish an SVG asset: ${unsafe}`)
    const name = `${createHash('sha256').update(svg).digest('hex').slice(0, 16)}.svg`
    assets.set(name, svg)
    return `url("../assets/${name}")`
  })
}

function generateComponents(specs: ComponentSpec[], context: GeneratorContext) {
  const files: GeneratedFile[] = []
  const manifest: ManifestEntry[] = []
  const assets = new Map<string, string>()
  const mode: ImageFills = context.imageFills ?? 'assets'
  const replaced: { component: string; count: number }[] = []
  const folders = new Map(specs.map((spec) => [spec.name, pageFolder(spec.page)]))
  for (const spec of specs) {
    const folder = pageFolder(spec.page)
    const cssRules: string[] = []
    files.push({
      path: `components/${folder}/${spec.name}.tsx`,
      content: emitComponent(spec, context, folders, cssRules)
    })
    let css = cssRules.join('\n\n') + '\n'
    if (mode === 'placeholder') {
      const result = replaceRasterFills(css)
      css = result.css
      if (result.replaced) replaced.push({ component: spec.name, count: result.replaced })
    }
    files.push({
      path: `components/${folder}/${spec.name}.module.css`,
      content: inLayer('dcl.components', externalizeSvgs(css, assets))
    })
    manifest.push({
      name: spec.name,
      page: spec.page,
      category: spec.category,
      figmaName: spec.figmaName,
      figmaId: spec.figmaId,
      file: `${folder}/${spec.name}`,
      props: spec.props.map((prop) => prop.name),
      views: spec.views.length,
      needsManual: spec.needsManual,
      notes: spec.notes
    })
  }
  for (const [name, svg] of [...assets].sort(([a], [b]) => a.localeCompare(b)))
    files.push({ path: `${ASSET_DIRECTORY}/${name}`, content: svg })
  files.push({
    path: 'components/manifest.ts',
    content: [
      header().trimEnd(),
      `export type ComponentManifestEntry = {
  name: string
  page: string
  category: string
  figmaName: string
  figmaId: string
  file: string
  props: string[]
  views: number
  needsManual: string[]
  notes: string[]
}
`,
      `export const componentManifest: ComponentManifestEntry[] = ${JSON.stringify(manifest, null, 2)}\n`
    ].join('\n')
  })
  files.push({
    path: 'components/index.ts',
    content: [
      header().trimEnd(),
      ...specs.map((spec) => `export * from './${pageFolder(spec.page)}/${spec.name}'`),
      "export * from './manifest'",
      ''
    ].join('\n')
  })
  return { files, manifest, imageFills: { mode, replaced } }
}

// Figma names are free text; a line terminator would end the `//` comment and turn the rest into code.
const commentText = (text: string) => text.replace(/[\r\n\u2028\u2029]/g, ' ')

export { externalizeSvgs, generateComponents, pageFolder }
export type { ManifestEntry }
