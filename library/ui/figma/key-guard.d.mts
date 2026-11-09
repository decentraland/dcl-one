type GuardFinding = { check: 'url' | 'key' | 'lfs' | 'artifact'; location: string }
export type ArtifactLimits = { files: number; totalBytes: number; fileBytes: number; depth: number }
type GuardOptions = {
  root: string
  range: string | null
  worktree: boolean
  paths: string[]
  trees?: { rev: string; prefix: string }[]
  artifacts?: string[]
  artifactLimits?: ArtifactLimits
  allowlistRev?: string
  key?: string
}
export declare const URL_PATTERN: RegExp
export declare function redact(text: unknown, key?: string): string
export declare const ARTIFACT_LIMITS: ArtifactLimits
export declare function enumerateArtifact(
  directory: string,
  limits?: ArtifactLimits
): { files: { full: string; path: string }[]; problems: { path: string; reason: string }[] }
export declare function isLfsPointer(buffer: Buffer): boolean
export type LfsObject = { oid: string; size: number }
export declare function parseLfsPointer(buffer: Buffer): LfsObject | null
export declare function lfsStore(root: string): string
export declare function listLfsObjects(options: {
  root: string
  range: string | null
  trees?: { rev: string; prefix: string }[]
}): LfsObject[]
export declare function scanBuffer(buffer: Buffer, key?: string, allowedIds?: Set<string>): string[]
export declare function runGuard(options: GuardOptions): {
  findings: GuardFinding[]
  lines: string[]
}
export declare function main(argv: string[]): number
