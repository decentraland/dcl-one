// The rule book call itself, shared by the worker and the inline fallback. Its input and output are plain,
// structured-clonable data so a run can cross the worker boundary.
import { type Finding, type Group } from '@dcl-regenesislabs/wearable-validator'

export type RuleBookJob =
  | { kind: 'model'; files: Map<string, Uint8Array>; metadata: unknown; category?: string }
  | { kind: 'thumbnail'; path: string; bytes: Uint8Array }

export type RuleBookFinding = Pick<Finding, 'check' | 'severity' | 'message' | 'where' | 'measured' | 'limit'> & {
  reason?: string
}

export type RuleBookCheck = { check: string; status: string; skipReason?: string }

export type RuleBookOutput = {
  findings: RuleBookFinding[]
  checks: RuleBookCheck[]
  /** Every check's title, keyed by check name. */
  titles: Record<string, string>
}

const MODEL_GROUPS: Group[] = ['model', 'emote']

// Loaded on first use, once: the rule book's parsers and decoders stay out of the route chunks.
let ruleBook: Promise<typeof import('@dcl-regenesislabs/wearable-validator')> | null = null

export async function runRuleBook(job: RuleBookJob, signal?: AbortSignal): Promise<RuleBookOutput> {
  // A failed chunk load is retried on the next run rather than failing every later check until reload.
  ruleBook ??= import('@dcl-regenesislabs/wearable-validator').catch((error: unknown) => {
    ruleBook = null
    throw error
  })
  const { validate, checks } = await ruleBook
  const result =
    job.kind === 'model'
      ? await validate(
          { files: job.files, metadata: job.metadata as never },
          { groups: MODEL_GROUPS, category: job.category, signal }
        )
      : await validate({ files: new Map([[job.path, job.bytes]]) }, { checks: ['thumbnail'], signal })
  const titles: Record<string, string> = {}
  for (const [check, definition] of Object.entries(checks as Record<string, { title?: string } | undefined>)) {
    if (definition?.title) titles[check] = definition.title
  }
  return {
    findings: result.findings.map(({ check, severity, message, where, measured, limit, data }) => ({
      check,
      severity,
      message,
      where,
      measured,
      limit,
      reason: typeof data?.reason === 'string' ? data.reason : undefined
    })),
    checks: result.checks.map(({ check, status, skipReason }) => ({ check, status, skipReason })),
    titles
  }
}
