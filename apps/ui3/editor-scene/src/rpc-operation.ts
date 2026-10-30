export type RpcOutcome = { ok: true; result: unknown } | { ok: false; error: string }

/** A deadline reports failure; it cannot cancel writes already sent to Bevy.
 * Keep the scheduler's serial slot until those writes really finish, otherwise
 * a late restore/reset can overwrite the next edit after its successful reply. */
export async function runRpcOperation(
  invoke: () => unknown,
  deadline: (() => Promise<never>) | null,
  reply: (outcome: RpcOutcome) => void
): Promise<void> {
  let operation: Promise<unknown> | undefined
  try {
    operation = Promise.resolve(invoke())
    const result = await (deadline ? Promise.race([operation, deadline()]) : operation)
    reply({ ok: true, result })
  } catch (error) {
    reply({ ok: false, error: String(error) })
  } finally {
    await operation?.catch(() => {})
  }
}
