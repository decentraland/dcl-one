import { runRuleBook, type RuleBookJob } from './ruleBook'

type Request = { id: number; job: RuleBookJob }

// The DOM lib types `self` as a Window, whose postMessage wants a target origin.
const scope = self as unknown as { postMessage(message: unknown): void; onmessage: unknown }

scope.onmessage = async ({ data }: MessageEvent<Request>) => {
  try {
    scope.postMessage({ id: data.id, output: await runRuleBook(data.job) })
  } catch (error) {
    scope.postMessage({ id: data.id, error: error instanceof Error ? error.message : String(error) })
  }
}
