import type {
  ColorValue,
  GradientStyle,
  IconRecord,
  TextStyle,
  TokenCollection,
  TokenValue
} from '../ir/schemas.js'

type RestColor = { r: number; g: number; b: number; a?: number }

type RestVariablesMeta = {
  variableCollections: Record<
    string,
    { name: string; modes: { modeId: string; name: string }[]; variableIds: string[] }
  >
  variables: Record<
    string,
    {
      name: string
      resolvedType: string
      variableCollectionId: string
      valuesByMode: Record<string, unknown>
    }
  >
}

type RestNode = {
  id: string
  name: string
  type: string
  visible?: boolean
  children?: RestNode[]
  absoluteBoundingBox?: { x: number; y: number; width: number; height: number }
}

const toHex = (channel: number) =>
  Math.round(channel * 255)
    .toString(16)
    .padStart(2, '0')

function mapRestColor(color: RestColor): ColorValue {
  return {
    hex: `#${toHex(color.r)}${toHex(color.g)}${toHex(color.b)}`,
    alpha: Math.round((color.a ?? 1) * 1000) / 1000
  }
}

function isAlias(value: unknown): value is { type: 'VARIABLE_ALIAS'; id: string } {
  return (
    typeof value === 'object' &&
    value !== null &&
    (value as { type?: string }).type === 'VARIABLE_ALIAS'
  )
}

function mapRestVariables(meta: RestVariablesMeta): TokenCollection[] {
  const describe = (id: string) => {
    const variable = meta.variables[id]
    const collection = variable && meta.variableCollections[variable.variableCollectionId]
    return variable && collection ? { name: variable.name, collection: collection.name } : undefined
  }

  return Object.values(meta.variableCollections).map((collection) => ({
    name: collection.name,
    modes: collection.modes.map((mode) => mode.name),
    variables: collection.variableIds.flatMap((id) => {
      const variable = meta.variables[id]
      if (!variable || (variable.resolvedType !== 'COLOR' && variable.resolvedType !== 'FLOAT')) {
        return []
      }
      const values: Record<string, TokenValue> = {}
      for (const mode of collection.modes) {
        const raw = variable.valuesByMode[mode.modeId]
        if (isAlias(raw)) {
          const target = describe(raw.id)
          if (!target) continue
          values[mode.name] = { alias: target.name, collection: target.collection }
        } else if (typeof raw === 'number') {
          values[mode.name] = raw
        } else if (raw && typeof raw === 'object') {
          values[mode.name] = mapRestColor(raw as RestColor)
        }
      }
      return [{ name: variable.name, type: variable.resolvedType, values }]
    })
  }))
}

const WEIGHT_STYLES: Record<number, string> = {
  400: 'Regular',
  500: 'Medium',
  600: 'Semi Bold',
  700: 'Bold',
  800: 'Extra Bold'
}

const TEXT_CASES: Record<string, TextStyle['textCase']> = {
  UPPER: 'UPPER',
  LOWER: 'LOWER',
  TITLE: 'TITLE'
}

type RestTypeStyle = {
  fontFamily: string
  fontWeight: number
  fontSize: number
  lineHeightPercentFontSize?: number
  letterSpacing?: number
  textCase?: string
}

function mapRestTextStyle(name: string, style: RestTypeStyle): TextStyle {
  return {
    name,
    fontFamily: style.fontFamily,
    fontStyle: WEIGHT_STYLES[style.fontWeight] ?? 'Regular',
    fontSize: style.fontSize,
    lineHeightPercent: Math.round(style.lineHeightPercentFontSize ?? 100),
    letterSpacingPx: style.letterSpacing ?? 0,
    textCase: (style.textCase && TEXT_CASES[style.textCase]) || 'ORIGINAL'
  }
}

type RestGradientPaint = {
  type: string
  gradientHandlePositions?: { x: number; y: number }[]
  gradientStops: { position: number; color: RestColor }[]
}

function mapRestGradient(name: string, paint: RestGradientPaint): GradientStyle | undefined {
  const type =
    paint.type === 'GRADIENT_RADIAL'
      ? 'radial'
      : paint.type === 'GRADIENT_LINEAR'
        ? 'linear'
        : undefined
  if (!type) return undefined
  const stops = paint.gradientStops.map((stop) => ({
    position: Math.round(stop.position * 10000) / 10000,
    hex: mapRestColor(stop.color).hex
  }))
  const [start, end] = paint.gradientHandlePositions ?? []
  const angleDeg =
    type === 'linear' && start && end
      ? Math.round((Math.atan2(end.x - start.x, -(end.y - start.y)) * 180) / Math.PI + 360) % 360
      : undefined
  return { name, type, ...(angleDeg === undefined ? {} : { angleDeg }), stops }
}

type RestIconComponent = Pick<IconRecord, 'name' | 'nodeId' | 'variant' | 'width' | 'height'> & {
  path: string
}

function collectIconComponents(sectionFrame: RestNode): RestIconComponent[] {
  const found: RestIconComponent[] = []
  const add = (path: string, node: RestNode, wrapper?: string) => {
    const box = node.absoluteBoundingBox
    const isVariant = wrapper !== undefined
    found.push({
      path,
      name: isVariant ? wrapper : path,
      nodeId: node.id,
      variant: isVariant ? node.name : null,
      width: Math.round((box?.width ?? 24) * 100) / 100,
      height: Math.round((box?.height ?? 24) * 100) / 100
    })
  }

  for (const holder of sectionFrame.children ?? []) {
    for (const child of holder.children ?? []) {
      if (child.visible === false) continue
      if (child.type === 'COMPONENT') add(child.name, child)
      else
        for (const variant of child.children ?? []) {
          if (variant.visible !== false && variant.type === 'COMPONENT') {
            add(`${child.name}/${variant.name}`, variant, child.name)
          }
        }
    }
  }
  return found
}

export { collectIconComponents, mapRestColor, mapRestGradient, mapRestTextStyle, mapRestVariables }
export type { RestIconComponent, RestNode, RestVariablesMeta }
