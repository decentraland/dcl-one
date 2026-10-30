import { describe, it, expect, vi, afterEach } from 'vitest'
import { EngineDriver, NO_SHELL_FOR_LOCAL_GUEST } from '../engine/EngineDriver'
import { EngineRpc } from '../engine/engineRpc'
import { sessionId } from '../lib/analytics'

describe('EngineDriver', () => {
  it('launches with the page analytics session id beside the host options', () => {
    const rpc = new EngineRpc()
    const launch = vi.spyOn(rpc, 'launch').mockImplementation(() => {})
    const driver = new EngineDriver(rpc)
    driver.launch('realm', '1,2', { holdWorld: true })
    driver.launch('realm')
    expect(launch).toHaveBeenNthCalledWith(1, 'realm', '1,2', { holdWorld: true, sessionId: sessionId() })
    expect(launch).toHaveBeenNthCalledWith(2, 'realm', undefined, { sessionId: sessionId() })
    expect(sessionId()).toMatch(/^[0-9a-f-]{36}$/)
    driver.dispose()
  })

  describe('loginLocalGuest', () => {
    afterEach(() => vi.doUnmock('../lib/shell'))

    it('asks the shell for the guest and signs in with its public identity', async () => {
      vi.resetModules()
      const identity = { ephemeralIdentity: { address: '0xe', publicKey: '0x04' }, expiration: '2099-01-01T00:00:00.000Z', authChain: [] }
      const shellRequest = vi.fn(async () => ({ address: '0xabc', identity, guest: true }))
      const actual = await vi.importActual<typeof import('../lib/shell')>('../lib/shell')
      vi.doMock('../lib/shell', () => ({ ...actual, inShell: true, shellRequest }))
      const { EngineDriver: InShell } = await import('../engine/EngineDriver')
      const { EngineRpc: Rpc } = await import('../engine/engineRpc')
      const rpc = new Rpc()
      const command = vi.spyOn(rpc, 'command').mockResolvedValue('logged in with identity')
      const driver = new InShell(rpc)

      await driver.loginLocalGuest()
      expect(shellRequest).toHaveBeenCalledWith('createLocalGuest')
      expect(command).toHaveBeenCalledTimes(1)
      const line = (command.mock.calls[0] as [string])[0]
      expect(line).toMatch(/^\/login_identity \S+ --guest --profiles /)
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

  it('the autopilot reports a failed sign-in to the engine as a console command', async () => {
    const rpc = new EngineRpc()
    const command = vi.spyOn(rpc, 'command').mockResolvedValue('run ended')
    const driver = new EngineDriver(rpc)
    await driver.autopilotFailed('no usable identity')
    expect(command).toHaveBeenCalledWith('/autopilot_fail no usable identity')
    driver.dispose()
  })
})
