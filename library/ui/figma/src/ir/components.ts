import { z } from 'zod'

const declarationSchema = z.tuple([z.string(), z.string()])

const cssBlockSchema = z.object({
  textStyle: z.string().optional(),
  decls: z.array(declarationSchema),
  // The ::after overlay that draws a stroke a box-shadow cannot (a gradient ring).
  after: z.array(declarationSchema).optional()
})

type CssBlock = z.infer<typeof cssBlockSchema>

type ViewNode = {
  kind: 'box' | 'text' | 'icon' | 'ref' | 'glyph'
  path: number[]
  name: string
  css: CssBlock
  children?: ViewNode[]
  text?: string
  textRuns?: { text: string; css: CssBlock }[]
  slot?: string
  // The Figma node id, when the capture records one. Text roles bind to it.
  figmaId?: string
  icon?: {
    component: string
    props: Record<string, string | boolean>
    size: number
    color?: string
  }
  ref?: { component: string; props: Record<string, string | boolean> }
  glyph?: { name: string; reason: string }
}

const propValueSchema = z.record(z.string(), z.union([z.string(), z.boolean()]))

// Opt-in accessible semantics for a box node, keyed by node name in figma.config.json
// (components.<Figma name>.semantics). `href` and `onActivate` name component props; `{slot}`
// stands for the slot of the node's first text, so repeated items get one prop each.
const semanticSchema = z.object({
  role: z.enum(['navigation', 'list', 'action']),
  label: z.string().min(1).optional(),
  href: z.string().min(1).optional(),
  onActivate: z.string().min(1).optional(),
  listItem: z.boolean().optional(),
  // An action with no href renders a button, or a plain box with fallback 'none'.
  fallback: z.enum(['button', 'none']).optional()
})

type Semantic = z.infer<typeof semanticSchema>

const viewNodeSchema: z.ZodType<ViewNode> = z.lazy(() =>
  z.object({
    kind: z.enum(['box', 'text', 'icon', 'ref', 'glyph']),
    path: z.array(z.number()),
    name: z.string(),
    css: cssBlockSchema,
    children: z.array(viewNodeSchema).optional(),
    text: z.string().optional(),
    textRuns: z.array(z.object({ text: z.string(), css: cssBlockSchema })).optional(),
    slot: z.string().optional(),
    figmaId: z.string().optional(),
    icon: z
      .object({
        component: z.string(),
        props: propValueSchema,
        size: z.number(),
        color: z.string().optional()
      })
      .optional(),
    ref: z.object({ component: z.string(), props: propValueSchema }).optional(),
    glyph: z.object({ name: z.string(), reason: z.string() }).optional()
  })
)

const stateRuleSchema = z.object({
  path: z.array(z.number()),
  decls: z.array(declarationSchema),
  after: z.array(declarationSchema).optional()
})

// How Figma animates into a state: milliseconds and a CSS timing function.
const motionSchema = z.object({ duration: z.number(), easing: z.string() })

// A timed two-variant loop (Figma AFTER_TIMEOUT + Change to, then an instant change back): the view plays
// it as a CSS animation that moves over `duration` milliseconds and restarts at once. Only immediate waits
// are generated, so the move fills the whole cycle.
const loopTrackSchema = z.object({
  path: z.array(z.number()),
  from: z.array(declarationSchema),
  to: z.array(declarationSchema)
})
const loopSchema = z.object({
  to: z.string(),
  duration: z.number(),
  easing: z.string(),
  tracks: z.array(loopTrackSchema)
})

const viewSpecSchema = z.object({
  key: z.string(),
  figmaName: z.string(),
  figmaId: z.string(),
  values: z.record(z.string(), z.string()),
  booleans: z.record(z.string(), z.boolean()),
  flags: z.object({ selected: z.boolean(), disabled: z.boolean(), loading: z.boolean() }),
  root: viewNodeSchema,
  hover: z.array(stateRuleSchema).optional(),
  pressed: z.array(stateRuleSchema).optional(),
  motion: z.object({ hover: motionSchema.optional(), pressed: motionSchema.optional() }).optional(),
  loop: loopSchema.optional()
})

const propSpecSchema = z.object({
  name: z.string(),
  kind: z.enum(['enum', 'boolean']),
  values: z.array(z.string()),
  default: z.union([z.string(), z.boolean()]),
  source: z.string()
})

const slotSpecSchema = z.object({
  name: z.string(),
  kind: z.enum(['text', 'icon']),
  default: z.string().optional()
})

// A text role is a named, typed public prop bound to captured text nodes by Figma node id
// (components.<Figma name>.textRoles in figma.config.json). Each node also states its expected path of layer names,
// from the variant root to the text, so a stale id fails generation instead of binding the wrong text. A count role
// names the string role that labels it; that label replaces the accessible name of the nearest semantic action. A role
// whose text sits under a semantic action with a link declares that link's prop name (href); it is never derived.
const textRoleTypeSchema = z.enum(['string', 'count'])
const textRoleSchema = z.object({
  type: textRoleTypeSchema,
  label: z.string().min(1).optional(),
  href: z.string().min(1).optional(),
  // Empty only for a label role, which carries the consumer's text and binds no captured layer.
  nodes: z
    .array(z.object({ nodeId: z.string().min(1), namePath: z.array(z.string()).min(1) }))
    .default([])
})
const textRoleSpecSchema = z.object({
  name: z.string(),
  type: textRoleTypeSchema,
  label: z.string().optional(),
  href: z.string().optional()
})

const componentSpecSchema = z.object({
  name: z.string(),
  figmaName: z.string(),
  figmaId: z.string(),
  page: z.string(),
  category: z.string(),
  kind: z.enum(['set', 'component']),
  description: z.string().optional(),
  tag: z.enum(['button', 'div']),
  props: z.array(propSpecSchema),
  slots: z.array(slotSpecSchema),
  exposeTextSlots: z.boolean().optional(),
  views: z.array(viewSpecSchema),
  defaultKey: z.string(),
  needsManual: z.array(z.string()),
  notes: z.array(z.string()),
  size: z.object({ w: z.number(), h: z.number() }),
  semantics: z.record(z.string(), semanticSchema).optional(),
  textRoles: z.array(textRoleSpecSchema).optional()
})

type StateRule = z.infer<typeof stateRuleSchema>
type Motion = z.infer<typeof motionSchema>
type Loop = z.infer<typeof loopSchema>
type ViewSpec = z.infer<typeof viewSpecSchema>
type SlotSpec = z.infer<typeof slotSpecSchema>
type ComponentSpec = z.infer<typeof componentSpecSchema>
type TextRole = z.infer<typeof textRoleSchema>
type TextRoleSpec = z.infer<typeof textRoleSpecSchema>

export {
  componentSpecSchema,
  cssBlockSchema,
  semanticSchema,
  textRoleSchema,
  viewNodeSchema,
  viewSpecSchema
}
export type {
  ComponentSpec,
  CssBlock,
  Loop,
  Motion,
  Semantic,
  SlotSpec,
  StateRule,
  TextRole,
  TextRoleSpec,
  ViewNode,
  ViewSpec
}
