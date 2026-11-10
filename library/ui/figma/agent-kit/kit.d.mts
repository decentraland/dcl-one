export type KitProblem = {
  skill: string
  reason: 'not-installed' | 'stale' | 'modified'
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
  agentsLink: 'created' | 'already-present' | 'skipped'
}>
export function checkKit(options: { target: string; kitRoot?: string }): Promise<{
  library: { name: string; version: string }
  ok: boolean
  problems: KitProblem[]
}>
