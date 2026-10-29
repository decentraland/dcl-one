import { describe, it, expect, vi, afterEach } from 'vitest'
import { BridgeClient } from '../engine/bridge'
import type { LoginDriver } from '../engine/driver'

type ConsoleWindow = Window & { engine_console_command?: (line: string) => Promise<string> }

afterEach(() => {
  delete (window as ConsoleWindow).engine_console_command
})

describe('BridgeClient (desktop)', () => {
  it('reset-profile jump-in asks the engine to fall back to a default profile', async () => {
    const run = vi.fn<(line: string) => Promise<string>>().mockResolvedValue('logged in')
    ;(window as ConsoleWindow).engine_console_command = run
    const client: LoginDriver = new BridgeClient('test-bridge-client')
    await client.jumpIn(true)
    expect(run).toHaveBeenCalledWith('/login_previous --default-on-error')
    client.dispose()
  })

  it('the autopilot\'s guest is the engine\'s own: one console command, no thirdweb calls', async () => {
    const run = vi.fn<(line: string) => Promise<string>>().mockResolvedValue('logged in as the local guest')
    ;(window as ConsoleWindow).engine_console_command = run
    const fetchSpy = vi.spyOn(globalThis, 'fetch')
    try {
      const client: LoginDriver = new BridgeClient('test-bridge-client')
      await client.loginLocalGuest?.()
      expect(run).toHaveBeenCalledExactlyOnceWith('/login_local_guest')
      expect(fetchSpy).not.toHaveBeenCalled()
      client.dispose()
    } finally {
      fetchSpy.mockRestore()
    }
  })

  it('the autopilot reports a failed sign-in as one plain-words console line', async () => {
    const run = vi.fn<(line: string) => Promise<string>>().mockResolvedValue('run ended')
    ;(window as ConsoleWindow).engine_console_command = run
    const client: LoginDriver = new BridgeClient('test-bridge-client')
    await client.autopilotFailed?.("sign-in failed: thirdweb 403:\n{\"error\": 'origin not allowed'}")
    expect(run).toHaveBeenCalledWith('/autopilot_fail sign-in failed: thirdweb 403: {error: origin not allowed}')
    client.dispose()
  })

  it('a previous-login query the engine never answers gives up, so the login screen can move on', async () => {
    vi.useFakeTimers()
    const client = new BridgeClient('test-bridge-client')
    const r = client.getPreviousLogin()
    const settled = expect(r).rejects.toThrow(/timed out/)
    await vi.advanceTimersByTimeAsync(15_000)
    await settled
    client.dispose()
    vi.useRealTimers()
  })

  it('disposing fails a pending sign-in instead of leaving it hanging', async () => {
    const client = new BridgeClient('test-bridge-client')
    const r = client.loginNew()
    client.dispose()
    await expect(r).rejects.toThrow(/closed/)
  })
})
