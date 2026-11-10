// The benchmark autopilot's part of react-web's engine drivers: what the shared LoginDriver leaves out,
// because only this host runs the benchmark (src/host.ts, `unattendedRun`).
import type { LoginDriver } from '@explorer-core/engine/driver'

/** `/autopilot_fail` with the reason as one line of plain words: the console splits on spaces and
 *  takes quotes as syntax. */
export function autopilotFailCommand(reason: string): string {
  return `/autopilot_fail ${reason.replace(/["'\\]/g, '').replace(/\s+/g, ' ').trim().slice(0, 200)}`
}

/** A login driver of this host, with the autopilot's sign-in and failure. */
export interface AutopilotDriver extends LoginDriver {
  /** The only identity the autopilot signs in with: the local guest, the account kept on this machine
   *  or in this browser profile, made once without a network call. Each run signs a fresh identity for
   *  the same address, and no stored account is used or replaced. Rejects on web outside the shell,
   *  which is where the key is kept. Absent where there is no engine to take it (mock); `loginGuest`
   *  stays the throwaway guest. */
  loginLocalGuest?(): Promise<void>
  /** The autopilot's unattended sign-in failed: the engine ends the run with a failure. Absent
   *  where there is no engine (mock). */
  autopilotFailed?(reason: string): Promise<unknown>
}
