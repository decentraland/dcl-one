import { afterEach, describe, expect, it, vi } from 'vitest'

let native = false
vi.mock('@hud-host', async (importOriginal) => ({ ...(await importOriginal<object>()), isNativeHud: () => native, exitUrl: () => 'https://example.com/exit' }))

const { exitApplication, setNativeQuit } = await import('../features/gate/exitApplication')

afterEach(() => {
  native = false
  setNativeQuit(null)
  vi.restoreAllMocks()
})

describe('exitApplication', () => {
  it('quits the engine on the desktop, and does not navigate', () => {
    native = true
    const quit = vi.fn()
    const close = vi.spyOn(window, 'close').mockImplementation(() => undefined)
    setNativeQuit(quit)
    exitApplication()
    expect(quit).toHaveBeenCalledOnce()
    expect(close).not.toHaveBeenCalled()
  })

  it('does nothing on the desktop before the session wired the engine in', () => {
    native = true
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined)
    const close = vi.spyOn(window, 'close').mockImplementation(() => undefined)
    exitApplication()
    expect(warn).toHaveBeenCalledOnce()
    expect(close).not.toHaveBeenCalled()
  })
})
