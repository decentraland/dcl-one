/** A failed authored notification must not starve the monitor which can repair
 * the runtime that caused it. Preserve the failure: never fabricate a drag ACK. */
export async function runEditorTick(
  run: () => Promise<void>,
  notify: () => Promise<void> | void,
  sample: () => void
): Promise<void> {
  try {
    await run()
    await notify()
  } finally {
    sample()
  }
}

export interface EditorSchedulerDependencies<Message, Sample> {
  handle(message: Message): Promise<void>
  recover(sample: Sample): Promise<void>
  onMonitorError?(error: unknown): void
  intervalMs: number
}

export function createEditorScheduler<Message, Sample>(deps: EditorSchedulerDependencies<Message, Sample>) {
  const inbound: Message[] = []
  let pendingSample: Sample | undefined
  let monitorInFlight = false
  let lastMonitorAt = 0
  let monitorGeneration = 0
  let serial: Promise<void> | null = null

  const enqueue = (message: Message): void => { inbound.push(message) }

  const sample = (now: number, read: () => Promise<Sample>): void => {
    if (monitorInFlight || now - lastMonitorAt < deps.intervalMs) return
    lastMonitorAt = now
    monitorInFlight = true
    const generation = monitorGeneration
    let reading: Promise<Sample>
    try {
      reading = read()
    } catch (error) {
      deps.onMonitorError?.(error)
      monitorInFlight = false
      return
    }
    void reading.then((value) => {
      if (generation === monitorGeneration) pendingSample = value
    }).catch((error: unknown) => {
      deps.onMonitorError?.(error)
    }).finally(() => {
      monitorInFlight = false
    })
  }

  const invalidateMonitor = (): void => {
    monitorGeneration++
    pendingSample = undefined
    lastMonitorAt = 0
  }

  const run = (): Promise<void> => {
    if (serial) return serial
    const task = (async () => {
      while (inbound.length > 0) await deps.handle(inbound.shift() as Message)
      if (pendingSample !== undefined) {
        const current = pendingSample
        pendingSample = undefined
        await deps.recover(current)
      }
    })()
    serial = task
    return task.finally(() => {
      if (serial === task) serial = null
    })
  }

  return { enqueue, sample, invalidateMonitor, run }
}
