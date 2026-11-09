import type { LfsObject } from './key-guard.mjs'
export declare function allowedUrl(value: string): boolean
export declare function parseObjects(text: string): LfsObject[]
export declare function prefetch(options: {
  store: string
  endpoint: string
  authorization: string
  objects: LfsObject[]
  fetchFn?: typeof fetch
}): Promise<{
  local: number
  downloaded: number
  bytes: number
  failures: { oid: string; reason: string }[]
}>
export declare function main(
  argv: string[],
  env?: Record<string, string | undefined>
): Promise<number>
