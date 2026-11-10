import type { Buffer } from 'node:buffer'
export const WEB: string
export const MANIFEST: string
export function rootInventory(paths: Iterable<string>): string[]
export function readSharedPaths(root?: string): string[]
export function generateSharedInputs(options?: {
  root?: string
  filteredSource?: boolean
}): Promise<Buffer>
