type GuardFinding = { check: 'url' | 'key' | 'lfs'; location: string }
type GuardOptions = {
  root: string
  range: string | null
  worktree: boolean
  paths: string[]
  key?: string
}
export declare const URL_PATTERN: RegExp
export declare function isLfsPointer(buffer: Buffer): boolean
export declare function scanBuffer(buffer: Buffer, key?: string, allowedIds?: Set<string>): string[]
export declare function runGuard(options: GuardOptions): {
  findings: GuardFinding[]
  lines: string[]
}
export declare function main(argv: string[]): number
