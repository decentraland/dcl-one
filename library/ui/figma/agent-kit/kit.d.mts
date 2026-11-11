export type KitProblem = {
  skill: string
  reason: 'not-installed' | 'stale' | 'modified' | 'agents-link-mismatch'
  message: string
}
export function installKit(options: {
  target: string
  force?: boolean
  linkAgents?: boolean
  kitRoot?: string
}): Promise<{
  library: { name: string; version: string }
  installed: string[]
  unchanged: string[]
  agentsLink: 'created' | 'already-linked' | 'replaced' | 'skipped'
}>
export function checkKit(options: { target: string; kitRoot?: string }): Promise<{
  library: { name: string; version: string }
  ok: boolean
  problems: KitProblem[]
}>
