// The benchmark autopilot's part of react-web's drivers (engine/autopilot.ts); the shared drivers'
// own tests are library/explorer-core/test/bridgeClient.test.ts and engineDriver.test.ts.
import { describe, it, expect, vi, afterEach } from 'vitest'
import { EngineRpc } from '@explorer-core/engine/engineRpc'
import { AutopilotBridgeClient, AutopilotEngineDriver as EngineDriver, NO_SHELL_FOR_LOCAL_GUEST, type AutopilotDriver } from '../engine/autopilot'

type ConsoleWindow = Window & { engine_console_command?: (line: string) => Promise<string> }

afterEach(() => {
  delete (window as ConsoleWindow).engine_console_command
})

describe('AutopilotBridgeClient (desktop)', () => {
  it('the autopilot\'s guest is the engine\'s own: one console command, no thirdweb calls', async () => {
    const run = vi.fn<(line: string) => Promise<string>>().mockResolvedValue('logged in as the local guest')
    ;(window as ConsoleWindow).engine_console_command = run
    const fetchSpy = vi.spyOn(globalThis, 'fetch')
    try {
      const client: AutopilotDriver = new AutopilotBridgeClient('test-bridge-client')
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
    const client: AutopilotDriver = new AutopilotBridgeClient('test-bridge-client')
    await client.autopilotFailed?.("sign-in failed: thirdweb 403:\n{\"error\": 'origin not allowed'}")
    expect(run).toHaveBeenCalledWith('/autopilot_fail sign-in failed: thirdweb 403: {error: origin not allowed}')
    client.dispose()
  })

})

describe('AutopilotEngineDriver', () => {
  describe('loginLocalGuest', () => {
    afterEach(() => vi.doUnmock('@explorer-core/lib/shell'))

    it('asks the shell for the guest and signs in with its public identity', async () => {
      vi.resetModules()
      const identity = { ephemeralIdentity: { address: '0xe', publicKey: '0x04' }, expiration: '2099-01-01T00:00:00.000Z', authChain: [] }
      const shellRequest = vi.fn(async () => ({ address: '0xabc', identity, guest: true }))
      const actual = await vi.importActual<typeof import('@explorer-core/lib/shell')>('@explorer-core/lib/shell')
      vi.doMock('@explorer-core/lib/shell', () => ({ ...actual, inShell: true, shellRequest }))
      const { AutopilotEngineDriver: InShell } = await import('../engine/autopilot')
      const { EngineRpc: Rpc } = await import('@explorer-core/engine/engineRpc')
      const rpc = new Rpc()
      const command = vi.spyOn(rpc, 'command').mockResolvedValue('logged in with identity')
      const driver = new InShell(rpc)

      await driver.loginLocalGuest()
      expect(shellRequest).toHaveBeenCalledWith('createLocalGuest')
      expect(command).toHaveBeenCalledTimes(1)
      const line = (command.mock.calls[0] as [string])[0]
      expect(line).toMatch(/^\/login_identity \S+ --guest$/)
      expect(JSON.parse(atob(line.split(' ')[1]))).toEqual(identity)
      driver.dispose()
    })

    it('fails where there is no shell to keep the guest, rather than sign in a throwaway one', async () => {
      const rpc = new EngineRpc()
      const command = vi.spyOn(rpc, 'command').mockResolvedValue('ok')
      const driver = new EngineDriver(rpc)
      await expect(driver.loginLocalGuest()).rejects.toThrow(NO_SHELL_FOR_LOCAL_GUEST)
      // no guest of any kind signed in; the session reports the failure, and the engine ends the run
      expect(command).not.toHaveBeenCalled()
      await driver.autopilotFailed(`sign-in failed: ${NO_SHELL_FOR_LOCAL_GUEST}`)
      expect(command).toHaveBeenCalledExactlyOnceWith('/autopilot_fail sign-in failed: no shell to keep the local guest')
      driver.dispose()
    })
  })

})
