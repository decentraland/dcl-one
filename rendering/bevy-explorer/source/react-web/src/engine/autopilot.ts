// The benchmark autopilot's part of react-web's engine drivers: what the shared drivers leave out,
// because only this host runs the benchmark (src/host.ts, `unattendedRun`). App.tsx signs in through these.
import type { LoginDriver } from '@explorer-core/engine/driver'
import { BridgeClient } from '@explorer-core/engine/bridge'
import { EngineDriver } from '@explorer-core/engine/EngineDriver'
import type { StoredLogin } from '@explorer-core/features/auth/sso'
import { inShell, shellRequest } from '@explorer-core/lib/shell'

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

/** Why the autopilot's sign-in fails outside the shell (`loginLocalGuest`). */
export const NO_SHELL_FOR_LOCAL_GUEST = 'no shell to keep the local guest'

/** The web engine's driver, with the autopilot's sign-in and failure. */
export class AutopilotEngineDriver extends EngineDriver implements AutopilotDriver {
  // The shell makes the autopilot's guest, keeps its root key and holds the signed identity for this
  // page without storing it (a stored account stays as it is); the engine gets the public identity and
  // signs through the shell. Outside the shell (the app page opened on its own) there is nowhere to
  // keep it, and a throwaway guest would be a new address every run: the sign-in fails, and with it
  // the run (the session reports it with `autopilotFailed`).
  async loginLocalGuest(): Promise<void> {
    if (!inShell) throw new Error(NO_SHELL_FOR_LOCAL_GUEST)
    const login = await shellRequest<StoredLogin>('createLocalGuest')
    await this.loginWithIdentity(login.identity, false, true)
  }

  async autopilotFailed(reason: string): Promise<unknown> {
    return this.rpc.command(autopilotFailCommand(reason))
  }
}

/** The native and mock driver, with the autopilot's sign-in and failure. */
export class AutopilotBridgeClient extends BridgeClient implements AutopilotDriver {
  // Native: the engine keeps the autopilot's guest, root key included, in its config beside the
  // previous login, and signs the identity itself. The mock has no engine console, so it stays a
  // plain guest.
  async loginLocalGuest(): Promise<void> {
    if (!('engine_console_command' in window)) return this.loginGuest()
    await this.command('/login_local_guest')
  }

  // Native: the engine console, as for the identity above. The mock has none, and no run to end.
  async autopilotFailed(reason: string): Promise<unknown> {
    if ('engine_console_command' in window) return this.command(autopilotFailCommand(reason))
  }
}
