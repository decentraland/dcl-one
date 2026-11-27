import * as readline from 'readline/promises'

type ConfirmQuestion = { type: 'confirm'; name: string; message: string; initial?: boolean }

type Options = { onCancel?: () => void }

export default async function prompts(
  question: ConfirmQuestion,
  options: Options = {}
): Promise<Record<string, boolean>> {
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout })
  // on Node <24 Ctrl+C only pauses stdin instead of rejecting rl.question; abort so onCancel runs
  const canceled = new AbortController()
  rl.on('SIGINT', () => canceled.abort())
  try {
    const suffix = question.initial ? ' [Y/n] ' : ' [y/N] '
    const answer = await rl.question(`${question.message}${suffix}`, { signal: canceled.signal })
    const value = answer === '' ? !!question.initial : /^y(es)?$/i.test(answer)
    return { [question.name]: value }
  } catch {
    options.onCancel?.()
    return {}
  } finally {
    rl.close()
  }
}
