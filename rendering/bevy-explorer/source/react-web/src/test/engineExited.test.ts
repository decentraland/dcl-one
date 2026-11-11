// react-web's engine can end its own run (the benchmark autopilot): the bridge channel then takes the
// scene's silence for no fault (src/host.ts, `engineExited`). The channel's other readiness tests are
// library/ui/apps/hud/test/hosted/bridgeReadiness.test.ts.
import { describe, it, expect, vi, afterEach } from 'vitest'
import { BridgeChannel } from '@explorer-core/engine/bridgeChannel'

let open: Array<{ close: () => void }> = []
const track = <T extends { close: () => void }>(x: T): T => {
  open.push(x)
  return x
}
afterEach(() => {
  open.forEach((c) => c.close())
  open = []
  vi.useRealTimers()
})

let seq = 0
const channelName = (): string => `test-engine-exited-${seq++}`

describe('BridgeChannel when the engine ends its own run', () => {
  it('is no fault when the engine ended its own run before the scene answered', async () => {
    vi.useFakeTimers()
    const onUnavailable = vi.fn()
    const ch = track(new BridgeChannel(channelName(), () => {}, onUnavailable))
    ch.expectReady()
    ;(window as { __engineExited?: boolean }).__engineExited = true
    try {
      await vi.advanceTimersByTimeAsync(60_000)
      expect(onUnavailable).not.toHaveBeenCalled()
    } finally {
      delete (window as { __engineExited?: boolean }).__engineExited
    }
  })

})
