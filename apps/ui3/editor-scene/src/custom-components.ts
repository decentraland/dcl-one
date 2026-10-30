
import { Name, Tags, SyncComponents, NetworkEntity, NetworkParent } from '@dcl/sdk/ecs'
import { ReadWriteByteBuffer } from '@dcl/ecs/dist/serialization/ByteBuffer'
import { CUSTOM_REGISTRY_DEFS } from './custom-registry'
import { type Snapshot } from './state'

type CustomDef = {
  componentId: number
  componentName: string
  schema: {
    deserialize: (reader: ReadWriteByteBuffer) => unknown
    serialize: (value: unknown, builder: ReadWriteByteBuffer) => void
    create: () => unknown
  }
}

const CUSTOM_DEFS = [
  Name,
  Tags,
  SyncComponents,
  NetworkEntity,
  NetworkParent,
  ...CUSTOM_REGISTRY_DEFS
] as unknown as CustomDef[]

const BY_ID = new Map<number, CustomDef>()
const BY_NAME = new Map<string, CustomDef>()
for (const def of CUSTOM_DEFS) {
  BY_ID.set(def.componentId, def)
  BY_NAME.set(def.componentName, def)
}

export const NAME_COMPONENT = (Name as unknown as CustomDef).componentName

const timestamps = new Map<string, number>()

const NUMERIC = /^\d+$/

const B64_ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'
const B64_INV = ((): Int16Array => {
  const t = new Int16Array(256).fill(-1)
  for (let i = 0; i < B64_ALPHABET.length; i++) t[B64_ALPHABET.charCodeAt(i)] = i
  return t
})()

function base64ToBytes(b64: string): Uint8Array {
  let len = b64.length
  while (len > 0 && b64[len - 1] === '=') len--
  const out = new Uint8Array((len * 3) >> 2)
  let o = 0
  let acc = 0
  let bits = 0
  for (let i = 0; i < len; i++) {
    const v = B64_INV[b64.charCodeAt(i)]
    if (v < 0) continue
    acc = (acc << 6) | v
    bits += 6
    if (bits >= 8) {
      bits -= 8
      out[o++] = (acc >> bits) & 0xff
    }
  }
  return out
}

function bytesToBase64(bytes: Uint8Array): string {
  let out = ''
  for (let i = 0; i < bytes.length; i += 3) {
    const b0 = bytes[i]
    const b1 = i + 1 < bytes.length ? bytes[i + 1] : 0
    const b2 = i + 2 < bytes.length ? bytes[i + 2] : 0
    out += B64_ALPHABET[b0 >> 2]
    out += B64_ALPHABET[((b0 & 3) << 4) | (b1 >> 4)]
    out += i + 1 < bytes.length ? B64_ALPHABET[((b1 & 15) << 2) | (b2 >> 6)] : '='
    out += i + 2 < bytes.length ? B64_ALPHABET[b2 & 63] : '='
  }
  return out
}

function splitTsB64(s: string): { ts: number; b64: string } {
  const i = s.indexOf(':')
  if (i < 0) return { ts: 0, b64: s }
  return { ts: Number(s.slice(0, i)) || 0, b64: s.slice(i + 1) }
}

function decodeBytes(def: CustomDef, b64: string): unknown {
  return def.schema.deserialize(new ReadWriteByteBuffer(base64ToBytes(b64)))
}

export function decodeCustomComponents(snapshot: Snapshot): void {
  for (const [entityId, comps] of Object.entries(snapshot)) {
    for (const key of Object.keys(comps)) {
      if (!NUMERIC.test(key)) continue
      const def = BY_ID.get(Number(key))
      if (def === undefined) continue
      const raw = comps[key]
      try {
        let ts = 0
        let value: unknown
        if (Array.isArray(raw)) {
          value = raw.map((s) => {
            const { ts: t, b64 } = splitTsB64(s as string)
            ts = Math.max(ts, t)
            return decodeBytes(def, b64)
          })
        } else {
          const { ts: t, b64 } = splitTsB64(raw as string)
          ts = t
          value = decodeBytes(def, b64)
        }
        delete comps[key]
        comps[def.componentName] = JSON.parse(JSON.stringify(value))
        recordCustomTimestamp(entityId, def.componentName, ts)
      } catch {
      }
    }
  }
}

export function isCustomComponent(name: string): boolean {
  return BY_NAME.has(name)
}

export function customComponentId(name: string): number | undefined {
  return BY_NAME.get(name)?.componentId
}

export function createCustomDefault(name: string): unknown | undefined {
  const def = BY_NAME.get(name)
  if (def === undefined) return undefined
  return JSON.parse(JSON.stringify(def.schema.create() ?? {}))
}

export function recordCustomTimestamp(entityId: string, name: string, timestamp: number): void {
  timestamps.set(`${entityId}/${name}`, Math.max(customTimestamp(entityId, name), timestamp))
}

export function customTimestamp(entityId: string, name: string): number {
  return timestamps.get(`${entityId}/${name}`) ?? 0
}

function utf8Bytes(s: string): Uint8Array {
  const out: number[] = []
  for (let i = 0; i < s.length; i++) {
    let c = s.charCodeAt(i)
    if (c < 0x80) {
      out.push(c)
    } else if (c < 0x800) {
      out.push(0xc0 | (c >> 6), 0x80 | (c & 0x3f))
    } else if (c >= 0xd800 && c <= 0xdbff && i + 1 < s.length) {
      const c2 = s.charCodeAt(++i)
      c = 0x10000 + ((c & 0x3ff) << 10) + (c2 & 0x3ff)
      out.push(0xf0 | (c >> 18), 0x80 | ((c >> 12) & 0x3f), 0x80 | ((c >> 6) & 0x3f), 0x80 | (c & 0x3f))
    } else {
      out.push(0xe0 | (c >> 12), 0x80 | ((c >> 6) & 0x3f), 0x80 | (c & 0x3f))
    }
  }
  return new Uint8Array(out)
}

export function stringToBase64(s: string): string {
  return bytesToBase64(utf8Bytes(s))
}

export function customComponentDefs(): Array<{ componentName: string; jsonSchema: unknown }> {
  return CUSTOM_DEFS.map((d) => ({
    componentName: d.componentName,
    jsonSchema: (d.schema as { jsonSchema?: unknown }).jsonSchema
  }))
}

export function encodeCustomComponent(name: string, value: unknown): string | undefined {
  const def = BY_NAME.get(name)
  if (def === undefined) return undefined
  const buf = new ReadWriteByteBuffer()
  def.schema.serialize(value, buf)
  return bytesToBase64(buf.toBinary())
}

export function entityName(snapshot: Snapshot, id: string): string | undefined {
  const name = snapshot[id]?.[NAME_COMPONENT] as { value?: string } | undefined
  return typeof name?.value === 'string' && name.value !== '' ? name.value : undefined
}
