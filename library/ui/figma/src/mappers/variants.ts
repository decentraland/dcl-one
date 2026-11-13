import type { ExpandedVariant } from '../capture/expand-variants.js'
import { assertIdentifier, camelCase } from '../generators/naming.js'
import type { RawComponent } from '../ir/raw-components.js'

type Flags = { selected: boolean; disabled: boolean; loading: boolean }
type Interaction = 'base' | 'hover' | 'pressed'
type FlagName = keyof Flags | 'hover' | 'pressed'

type PropSpec = {
  name: string
  kind: 'enum' | 'boolean'
  values: string[]
  default: string | boolean
  source: string
}

type MappedVariant = {
  figmaName: string
  figmaId: string
  values: Record<string, string>
  booleans: Record<string, boolean>
  flags: Flags
  interaction: Interaction
  tree: ExpandedVariant['tree']
}

type VariantMapping = {
  props: PropSpec[]
  variants: MappedVariant[]
  notes: string[]
  issues: string[]
}

type VariantOptions = {
  wordAliases: Record<string, string>
  stateWords?: FlagName[]
  optionAliases?: Record<string, string>
  propNames?: Record<string, string>
  skipOptions?: string[]
}

const STATE_WORDS: Record<string, FlagName> = {
  hover: 'hover',
  pressed: 'pressed',
  press: 'pressed',
  pres: 'pressed',
  selected: 'selected',
  blocked: 'disabled',
  disabled: 'disabled',
  disable: 'disabled',
  loading: 'loading'
}
const BASE_WORDS = new Set(['idle', 'default', 'normal', 'unselected'])
const RESERVED = new Set(['switch', 'default', 'class', 'new', 'delete', 'in', 'if', 'for', 'var'])
const YES = new Set(['yes', 'true', 'on'])
const NO = new Set(['no', 'false', 'off'])

function splitWords(input: string): string[] {
  return input
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .split(/[^A-Za-z0-9]+/)
    .filter(Boolean)
    .map((word) => word.toLowerCase())
}

type ParsedOption = { flags: Set<FlagName>; remainder: string }

function parseOption(option: string, options: VariantOptions): ParsedOption {
  const aliased = options.optionAliases?.[option] ?? option
  const active = new Set<FlagName>(
    options.stateWords ?? ['hover', 'pressed', 'selected', 'disabled', 'loading']
  )
  const flags = new Set<FlagName>()
  const rest: string[] = []
  for (const word of splitWords(aliased)) {
    const state = STATE_WORDS[word]
    if (state && active.has(state)) flags.add(state)
    else if (BASE_WORDS.has(word)) continue
    else rest.push(options.wordAliases[word] ?? word)
  }
  return { flags, remainder: camelCase(rest.join(' ')) }
}

const isGenericName = (name: string) => /^Property \d+$/.test(name)

const camelWords = (input: string) => camelCase(splitWords(input).join(' '))

function interactionOf(flags: Set<FlagName>): Interaction {
  if (flags.has('pressed')) return 'pressed'
  if (flags.has('hover')) return 'hover'
  return 'base'
}

type Classified =
  | { kind: 'flag-property'; flag: FlagName }
  | { kind: 'state-only'; parsed: Map<string, ParsedOption> }
  | { kind: 'numbered'; parsed: Map<string, ParsedOption> }
  | { kind: 'enum'; parsed: Map<string, ParsedOption>; fallback: string }

function classify(
  name: string,
  definition: RawComponent['properties'][string],
  options: VariantOptions
): Classified {
  const values = definition.options ?? []
  const nameWord = STATE_WORDS[splitWords(name).join('')]
  const lowered = values.map((value) => value.toLowerCase())
  if (nameWord && lowered.length > 0 && lowered.every((value) => YES.has(value) || NO.has(value))) {
    return { kind: 'flag-property', flag: nameWord }
  }

  const parsed = new Map(values.map((value) => [value, parseOption(value, options)]))
  const remainders = [...parsed.values()].map((entry) => entry.remainder)
  if (remainders.every((remainder) => remainder === '')) return { kind: 'state-only', parsed }

  const numbered = remainders.filter((remainder) => remainder !== '')
  if (numbered.length > 0 && numbered.every((remainder) => /^variant\d+$/.test(remainder))) {
    return { kind: 'numbered', parsed }
  }

  const defaultOption = String(definition.default ?? values[0] ?? '')
  const defaultRemainder = parsed.get(defaultOption)?.remainder
  const hasExplicitBase = [...parsed.values()].some(
    (entry) => entry.remainder === '' && entry.flags.size === 0
  )
  return {
    kind: 'enum',
    parsed,
    fallback: hasExplicitBase ? 'default' : defaultRemainder || 'default'
  }
}

const NUMBERED_FLAGS: Record<number, FlagName[]> = {
  1: [],
  2: ['hover'],
  3: ['selected'],
  4: ['selected', 'hover']
}

function propName(figmaName: string, options: VariantOptions, used: Set<string>): string {
  // A name the configuration states is used as written, so it must already be a valid binding; a derived name is
  // adjusted below instead.
  const configured = options.propNames?.[figmaName]
  if (configured !== undefined) assertIdentifier(configured)
  let base = configured ?? (isGenericName(figmaName) ? 'variant' : camelWords(figmaName))
  if (!base) base = 'variant'
  if (RESERVED.has(base)) base = `${base}Value`
  let name = base
  for (let counter = 2; used.has(name); counter += 1) name = `${base}${counter}`
  used.add(name)
  return name
}

function mapVariants(
  component: RawComponent,
  variants: ExpandedVariant[],
  options: VariantOptions
): VariantMapping {
  const notes: string[] = []
  const issues: string[] = []
  const used = new Set<string>(['label', 'icon', 'className', 'forceState', 'children'])
  const classified = Object.entries(component.properties)
    .filter(([, definition]) => definition.type === 'VARIANT')
    .map(([name, original]) => {
      let definition = original
      if (component.hiddenVariants?.length) {
        const observed = new Set(variants.map((variant) => variant.props[name]))
        const available = (original.options ?? []).filter((value) => observed.has(value))
        const defaultValue = available.includes(String(original.default))
          ? original.default
          : available[0]
        definition = { ...original, options: available, default: defaultValue }
        if (!available.length) issues.push(`missing-visible-variant-property:${name}`)
        if (defaultValue !== original.default)
          notes.push(`visible-variant-default:${name}=${String(defaultValue)}`)
      }
      return { name, definition, result: classify(name, definition, options) }
    })

  const props: PropSpec[] = []
  const enumNames = new Map<string, { prop: string; fallback: string; boolean: boolean }>()

  for (const { name, definition, result } of classified) {
    if (result.kind !== 'enum') {
      if (result.kind === 'numbered') {
        notes.push(
          `numbered-variants: ${name} options read as default/hover/selected/selected+hover`
        )
      }
      continue
    }
    const values = [
      ...new Set([...result.parsed.values()].map((entry) => entry.remainder || result.fallback))
    ]
    const lowered = values.map((value) => value.toLowerCase())
    const isBoolean =
      values.length === 2 &&
      lowered.some((value) => YES.has(value)) &&
      lowered.some((value) => NO.has(value))
    const defaultOption = String(definition.default ?? '')
    const defaultValue = result.parsed.get(defaultOption)?.remainder || result.fallback
    const prop = propName(name, options, used)
    enumNames.set(name, { prop, fallback: result.fallback, boolean: isBoolean })
    if (isBoolean) {
      const onValue = values.find((value) => YES.has(value.toLowerCase()))!
      const configuredBoolean = options.propNames?.[name]
      if (configuredBoolean !== undefined) assertIdentifier(configuredBoolean)
      const named = configuredBoolean ?? (isGenericName(name) ? 'checked' : prop)
      enumNames.set(name, { prop: named, fallback: result.fallback, boolean: true })
      used.delete(prop)
      used.add(named)
      props.push({
        name: named,
        kind: 'boolean',
        values: ['false', 'true'],
        default: defaultValue === onValue,
        source: name
      })
    } else if (values.length > 1) {
      props.push({ name: prop, kind: 'enum', values, default: defaultValue, source: name })
    } else {
      used.delete(prop)
      enumNames.delete(name)
    }
  }

  const skipped = new Set(options.skipOptions ?? [])
  const kept = variants.filter((variant) => {
    const hit = Object.values(variant.props).find((value) => skipped.has(value))
    if (hit) notes.push(`skipped-variant: "${variant.name}" is excluded by configuration`)
    return !hit
  })

  const mapped: MappedVariant[] = kept.map((variant) => {
    const flags = new Set<FlagName>()
    const values: Record<string, string> = {}
    const booleans: Record<string, boolean> = {}
    for (const { name, result } of classified) {
      const chosen = variant.props[name]
      if (chosen === undefined) continue
      if (result.kind === 'flag-property') {
        if (YES.has(chosen.toLowerCase())) flags.add(result.flag)
        continue
      }
      if (result.kind === 'numbered') {
        const index = Number(/variant(\d+)/i.exec(chosen)?.[1] ?? 1)
        for (const flag of NUMBERED_FLAGS[index] ?? []) flags.add(flag)
        continue
      }
      const parsed = result.parsed.get(chosen) ?? parseOption(chosen, options)
      parsed.flags.forEach((flag) => flags.add(flag))
      if (result.kind === 'enum') {
        const target = enumNames.get(name)
        if (!target) continue
        const value = parsed.remainder || result.fallback
        if (target.boolean) booleans[target.prop] = YES.has(value.toLowerCase())
        else values[target.prop] = value
      }
    }
    return {
      figmaName: variant.name,
      figmaId: variant.id,
      values,
      booleans,
      flags: {
        selected: flags.has('selected'),
        disabled: flags.has('disabled'),
        loading: flags.has('loading')
      },
      interaction: interactionOf(flags),
      tree: variant.tree
    }
  })

  const present = (flag: keyof Flags) => mapped.some((variant) => variant.flags[flag])
  const flagProps: [keyof Flags, string][] = [
    ['selected', 'selected'],
    ['disabled', 'disabled'],
    ['loading', 'loading']
  ]
  for (const [flag, name] of flagProps) {
    if (present(flag)) {
      props.push({ name, kind: 'boolean', values: ['false', 'true'], default: false, source: flag })
    }
  }

  const seen = new Map<string, string>()
  for (const variant of mapped) {
    const key = `${viewKey(variant)}#${variant.interaction}`
    const previous = seen.get(key)
    if (previous)
      issues.push(
        `variant-collision: "${variant.figmaName}" maps to the same view as "${previous}"`
      )
    else seen.set(key, variant.figmaName)
  }

  return { props, variants: mapped, notes, issues }
}

function viewKey(variant: Pick<MappedVariant, 'values' | 'booleans' | 'flags'>): string {
  const parts = [
    ...Object.entries(variant.values).map(([name, value]) => `${name}:${value}`),
    ...Object.entries(variant.booleans)
      .filter(([, value]) => value)
      .map(([name]) => name),
    ...(Object.entries(variant.flags) as [string, boolean][])
      .filter(([, value]) => value)
      .map(([name]) => name)
  ]
  return parts.join('|') || 'default'
}

export { mapVariants, parseOption, splitWords, viewKey }
export type { Flags, Interaction, MappedVariant, PropSpec, VariantMapping, VariantOptions }
