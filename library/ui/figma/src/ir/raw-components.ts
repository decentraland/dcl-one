import { z } from 'zod'

const paintSchema = z.object({
  type: z.string(),
  opacity: z.number().optional(),
  hex: z.string().optional(),
  token: z.string().optional(),
  stops: z
    .array(
      z.object({
        position: z.number(),
        hex: z.string(),
        alpha: z.number(),
        token: z.string().optional()
      })
    )
    .optional(),
  transform: z.array(z.array(z.number())).optional(),
  scaleMode: z.string().optional(),
  // IMAGE paints point at their bytes in the component's `images` table by Figma image hash.
  imageRef: z.string().optional()
})

const tintSideSchema = z.object({
  hex: z.string().optional(),
  token: z.string().optional(),
  opacity: z.number().optional(),
  weight: z.number().optional()
})

const tintSchema = z.object({ fill: tintSideSchema.optional(), stroke: tintSideSchema.optional() })

const effectSchema = z.object({
  type: z.string(),
  radius: z.number(),
  color: z.string().optional(),
  alpha: z.number().optional(),
  offset: z.tuple([z.number(), z.number()]).optional(),
  spread: z.number().optional()
})

const layoutSchema = z.object({
  mode: z.enum(['HORIZONTAL', 'VERTICAL']),
  pad: z.tuple([z.number(), z.number(), z.number(), z.number()]),
  gap: z.number(),
  main: z.string(),
  cross: z.string(),
  wrap: z.boolean().optional()
})

const lineHeightSchema = z.object({ unit: z.string(), value: z.number().optional() })

const textRunSchema = z.object({
  text: z.string(),
  font: z.string(),
  fontSize: z.number(),
  fills: z.union([z.literal('mixed'), z.array(paintSchema)]),
  textCase: z.string().optional(),
  lineHeight: lineHeightSchema.optional(),
  letterSpacing: z.object({ unit: z.string(), value: z.number() }).optional()
})

type RawNode = {
  type: string
  name?: string
  w: number
  h: number
  hidden?: boolean
  clipsContent?: boolean
  flip?: [boolean, boolean]
  isMask?: boolean
  maskType?: string
  mask?: { svg?: string; png?: string; sum?: string; heavy?: boolean }
  textRuns?: z.infer<typeof textRunSchema>[]
  textAlignHorizontal?: string
  textAlignVertical?: string
  lineHeight?: z.infer<typeof lineHeightSchema>
  paragraphSpacing?: number
  textTruncation?: string
  maxLines?: number
  opacity?: number
  layout?: z.infer<typeof layoutSchema>
  sizing?: [string, string]
  radius?: number | number[]
  fills?: 'mixed' | z.infer<typeof paintSchema>[]
  fillStyle?: string
  strokes?: z.infer<typeof paintSchema>[]
  strokeWeight?: number | 'mixed'
  strokeWeights?: [number, number, number, number]
  strokeAlign?: string
  // Absent means the capture did not record it, which is not the same as false.
  strokesIncludedInLayout?: boolean
  dash?: number[]
  strokeStyle?: string
  effects?: z.infer<typeof effectSchema>[]
  bound?: Record<string, string>
  text?: string
  textStyle?: string
  font?: string
  fontSize?: number
  letterSpacing?: { unit: string; value: number }
  textCase?: string
  component?: string
  componentId?: string
  variant?: string
  props?: Record<string, string | boolean | number>
  tint?: z.infer<typeof tintSchema>
  hiddenChildren?: string[]
  // Offset from the parent's top-left corner, present for children of frames without auto-layout and for
  // absolutely positioned children of auto-layout frames.
  x?: number
  y?: number
  absolute?: boolean
  // Exported artwork for DRAWING nodes: inline SVG, a base64 PNG, or `heavy` when neither fits a snapshot.
  svg?: string
  png?: string
  heavy?: boolean
  sum?: string
  children?: RawNode[]
}

const rawNodeSchema: z.ZodType<RawNode> = z.lazy(() =>
  z.object({
    type: z.string(),
    name: z.string().optional(),
    w: z.number(),
    h: z.number(),
    hidden: z.boolean().optional(),
    clipsContent: z.boolean().optional(),
    flip: z.tuple([z.boolean(), z.boolean()]).optional(),
    isMask: z.boolean().optional(),
    maskType: z.string().optional(),
    mask: z
      .object({
        svg: z.string().optional(),
        png: z.string().optional(),
        sum: z.string().optional(),
        heavy: z.boolean().optional()
      })
      .optional(),
    textRuns: z.array(textRunSchema).optional(),
    textAlignHorizontal: z.string().optional(),
    textAlignVertical: z.string().optional(),
    lineHeight: lineHeightSchema.optional(),
    paragraphSpacing: z.number().optional(),
    textTruncation: z.string().optional(),
    maxLines: z.number().optional(),
    opacity: z.number().optional(),
    layout: layoutSchema.optional(),
    sizing: z.tuple([z.string(), z.string()]).optional(),
    radius: z.union([z.number(), z.array(z.number())]).optional(),
    fills: z.union([z.literal('mixed'), z.array(paintSchema)]).optional(),
    fillStyle: z.string().optional(),
    strokes: z.array(paintSchema).optional(),
    strokeWeight: z.union([z.number(), z.literal('mixed')]).optional(),
    strokeWeights: z.tuple([z.number(), z.number(), z.number(), z.number()]).optional(),
    strokeAlign: z.string().optional(),
    strokesIncludedInLayout: z.boolean().optional(),
    dash: z.array(z.number()).optional(),
    strokeStyle: z.string().optional(),
    effects: z.array(effectSchema).optional(),
    bound: z.record(z.string(), z.string()).optional(),
    text: z.string().optional(),
    textStyle: z.string().optional(),
    font: z.string().optional(),
    fontSize: z.number().optional(),
    letterSpacing: z.object({ unit: z.string(), value: z.number() }).optional(),
    textCase: z.string().optional(),
    component: z.string().optional(),
    componentId: z.string().optional(),
    variant: z.string().optional(),
    props: z.record(z.string(), z.union([z.string(), z.boolean(), z.number()])).optional(),
    tint: tintSchema.optional(),
    hiddenChildren: z.array(z.string()).optional(),
    x: z.number().optional(),
    y: z.number().optional(),
    absolute: z.boolean().optional(),
    svg: z.string().optional(),
    png: z.string().optional(),
    heavy: z.boolean().optional(),
    sum: z.string().optional(),
    children: z.array(rawNodeSchema).optional()
  })
)

const patchSchema = z.object({
  at: z.array(z.number()),
  set: z.record(z.string(), z.unknown()).optional(),
  unset: z.array(z.string()).optional(),
  replace: rawNodeSchema.optional()
})

const variantPropsSchema = z.record(z.string(), z.string())

// A prototype "Change to" interaction on a variant, kept as Figma reports it (duration in seconds).
const interactionSchema = z.object({
  trigger: z.string(),
  // AFTER_TIMEOUT only: Figma's raw wait, kept as captured. Its unit is not established (documented as
  // milliseconds, observed as 0.001 for "immediately"), so it is never converted here.
  timeout: z.number().optional(),
  destinationId: z.string(),
  transition: z
    .object({
      type: z.string(),
      duration: z.number(),
      easing: z.string(),
      bezier: z.tuple([z.number(), z.number(), z.number(), z.number()]).optional()
    })
    .optional()
})

const rawComponentSchema = z.object({
  kind: z.enum(['set', 'component']),
  id: z.string(),
  name: z.string(),
  description: z.string().optional(),
  hiddenVariants: z.array(z.object({ id: z.string(), name: z.string() })).optional(),
  section: z.array(z.string()),
  x: z.number(),
  y: z.number(),
  properties: z.record(
    z.string(),
    z.object({
      type: z.string(),
      default: z.union([z.string(), z.boolean()]).optional(),
      // SLOT defaults carry structured Figma data, not a generated variant value.
      slotDefault: z.json().optional(),
      options: z.array(z.string()).optional()
    })
  ),
  base: z.object({
    id: z.string(),
    name: z.string(),
    props: variantPropsSchema,
    interactions: z.array(interactionSchema).optional(),
    tree: rawNodeSchema
  }),
  // Image fills used by this component, once per Figma image hash, as PNG data URIs (validatePage
  // decodes each one).
  images: z
    .record(
      z.string().min(1),
      z
        .string()
        .regex(/^data:image\/png;base64,[A-Za-z0-9+/]+={0,2}$/, 'image fill must be a PNG data URI')
    )
    .optional(),
  others: z.array(
    z.object({
      id: z.string(),
      name: z.string(),
      props: variantPropsSchema,
      interactions: z.array(interactionSchema).optional(),
      patches: z.array(patchSchema)
    })
  ),
  truncated: z.boolean().optional(),
  // Captured from outside the page (a library component, or one living on a page that is not generated).
  external: z.boolean().optional()
})

const rawComponentPageSchema = z.object({
  page: z.object({ id: z.string(), name: z.string() }),
  labels: z.array(z.object({ text: z.string(), x: z.number(), y: z.number() })).optional(),
  components: z.array(rawComponentSchema)
})

type RawPaint = z.infer<typeof paintSchema>
type RawPatch = z.infer<typeof patchSchema>
type RawInteraction = z.infer<typeof interactionSchema>
type RawComponent = z.infer<typeof rawComponentSchema>
type RawComponentPage = z.infer<typeof rawComponentPageSchema>

export { rawComponentPageSchema, rawComponentSchema, rawNodeSchema }
export type { RawComponent, RawComponentPage, RawInteraction, RawNode, RawPaint, RawPatch }
