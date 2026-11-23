// Runs the rule book off the main thread when the browser allows it, so parsing a model never freezes the page.
import { captureError } from '../monitoring'
import { runRuleBook, type RuleBookJob, type RuleBookOutput } from './ruleBook'

type Reply = { id: number; output?: RuleBookOutput; error?: string }
type Pending = { resolve: (output: RuleBookOutput) => void; reject: (error: Error) => void; timer: number }

/** A run still going after this is taken as hung: the worker is replaced so checks (and the publish modal) move on. */
export const WORKER_TIMEOUT_MS = 60_000

let worker: Promise<Worker | null> | null = null
const pending = new Map<number, Pending>()
let nextId = 0

async function spawn(): Promise<Worker | null> {
  if (typeof Worker === 'undefined') return null
  try {
    const { default: url } = await import('./ruleBook.worker?worker&url')
    // Production serves the bundle from the CDN, and a worker script must be same-origin: a same-origin
    // blob module importing the real one sidesteps that (the CDN already serves modules with CORS).
    const absolute = new URL(url, window.location.href).href
    const shim = URL.createObjectURL(new Blob([`import ${JSON.stringify(absolute)}`], { type: 'text/javascript' }))
    const instance = new Worker(shim, { type: 'module' })
    instance.onmessage = ({ data }: MessageEvent<Reply>) => {
      const job = pending.get(data.id)
      if (!job) return
      clearTimeout(job.timer)
      pending.delete(data.id)
      if (data.output) job.resolve(data.output)
      else job.reject(new Error(data.error ?? 'Validation worker failed'))
    }
    // Every later run goes inline, so this fires at most once per page load.
    instance.onerror = event => {
      event.preventDefault()
      const error = new Error(event.message || 'Validation worker crashed')
      captureError(error, { flow: 'validation_worker' })
      discard(instance, error)
      URL.revokeObjectURL(shim)
      worker = Promise.resolve(null)
    }
    return instance
  } catch {
    return null
  }
}

/** Stops `instance` and fails its in-flight runs; their bytes were transferred, so they can't be resent. */
function discard(instance: Worker, error: Error) {
  for (const job of pending.values()) {
    clearTimeout(job.timer)
    job.reject(error)
  }
  pending.clear()
  instance.terminate()
}

/**
 * Runs `job` in the worker, or inline without one. A worker run can't be interrupted, so it resolves even if
 * `signal` aborts mid-run; only the inline run honours it.
 */
export async function runOffMainThread(job: RuleBookJob, signal?: AbortSignal): Promise<RuleBookOutput> {
  worker ??= spawn()
  const instance = await worker
  if (!instance) return runRuleBook(job, signal)
  const id = nextId++
  return new Promise<RuleBookOutput>((resolve, reject) => {
    // A hung run is not a crash: a fresh worker is spawned for the next one rather than going inline.
    const timer = window.setTimeout(() => {
      const error = new Error('Validation worker timed out')
      captureError(error, { flow: 'validation_worker', step: 'timeout' })
      discard(instance, error)
      worker = null
    }, WORKER_TIMEOUT_MS)
    pending.set(id, { resolve, reject, timer })
    const buffers = job.kind === 'model' ? [...job.files.values()].map(bytes => bytes.buffer) : [job.bytes.buffer]
    instance.postMessage({ id, job }, [...new Set(buffers as ArrayBuffer[])])
  })
}
