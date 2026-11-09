import { z } from 'zod'

const colorValueSchema = z.object({ hex: z.string().regex(/^#[0-9a-f]{6}$/), alpha: z.number() })
const aliasValueSchema = z.object({ alias: z.string(), collection: z.string() })
const tokenValueSchema = z.union([colorValueSchema, aliasValueSchema, z.number()])

const tokenSchema = z.object({
  name: z.string(),
  type: z.enum(['COLOR', 'FLOAT']),
  values: z.record(z.string(), tokenValueSchema)
})

const tokenCollectionSchema = z.object({
  name: z.string(),
  modes: z.array(z.string()).min(1),
  variables: z.array(tokenSchema)
})

const textStyleSchema = z.object({
  name: z.string(),
  fontFamily: z.string(),
  fontStyle: z.string(),
  fontSize: z.number(),
  lineHeightPercent: z.number(),
  lineHeightAuto: z.boolean().optional(),
  letterSpacingPx: z.number(),
  textCase: z.enum(['ORIGINAL', 'UPPER', 'LOWER', 'TITLE'])
})

const gradientStopSchema = z.object({ position: z.number(), hex: z.string() })

const gradientStyleSchema = z.object({
  name: z.string(),
  type: z.enum(['linear', 'radial']),
  angleDeg: z.number().optional(),
  stops: z.array(gradientStopSchema).min(2)
})

const iconRecordSchema = z.object({
  name: z.string(),
  nodeId: z.string(),
  section: z.string(),
  category: z.string(),
  group: z.string(),
  variant: z.string().nullable(),
  width: z.number(),
  height: z.number(),
  file: z.string()
})

const iconSpecSchema = iconRecordSchema.extend({ svg: z.string() })

const pageSchema = z.object({ id: z.string(), name: z.string() })

type ColorValue = z.infer<typeof colorValueSchema>
type AliasValue = z.infer<typeof aliasValueSchema>
type TokenValue = z.infer<typeof tokenValueSchema>
type Token = z.infer<typeof tokenSchema>
type TokenCollection = z.infer<typeof tokenCollectionSchema>
type TextStyle = z.infer<typeof textStyleSchema>
type GradientStyle = z.infer<typeof gradientStyleSchema>
type IconRecord = z.infer<typeof iconRecordSchema>
type IconSpec = z.infer<typeof iconSpecSchema>
type FigmaPage = z.infer<typeof pageSchema>

export {
  gradientStyleSchema,
  iconRecordSchema,
  iconSpecSchema,
  pageSchema,
  textStyleSchema,
  tokenCollectionSchema,
  tokenSchema,
  tokenValueSchema
}
export type {
  AliasValue,
  ColorValue,
  FigmaPage,
  GradientStyle,
  IconRecord,
  IconSpec,
  TextStyle,
  Token,
  TokenCollection,
  TokenValue
}
