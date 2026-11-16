import { describe, it, expect, vi } from 'vitest'

const { relay } = vi.hoisted(() => ({ relay: vi.fn() }))
vi.mock('../../bridge-scene/src/system-helpers', () => ({ relay }))
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: { getVoiceStream: vi.fn() } }))

const { registerVoice } = await import('../../bridge-scene/src/domains/voice')

type Voice = { sender_address: string; channel: string; active: boolean }
type Sent = { kind: string; address: string; active: boolean }

function engine(): { sent: Sent[]; voice: (v: Voice) => void; reopen: () => Promise<unknown> } {
  const sent: Sent[] = []
  relay.mockClear()
  registerVoice({ send: (msg) => sent.push(msg as never), on: () => undefined, push: () => undefined })
  const [, reopen, voice] = relay.mock.calls[0] as [string, () => Promise<unknown>, (v: Voice) => void]
  return { sent, voice, reopen }
}

const ADDR = '0x5854cce95d5e25817b41f4c41f06b695a83bc495'

describe('voice activity (bridge)', () => {
  it('tells the page who starts and stops talking, by lowercased address', () => {
    const { sent, voice } = engine()
    voice({ sender_address: ADDR.toUpperCase().replace('0X', '0x'), channel: 'Nearby', active: true })
    voice({ sender_address: ADDR, channel: 'Nearby', active: false })
    expect(sent).toEqual([
      { kind: 'voiceActivity', address: ADDR, active: true },
      { kind: 'voiceActivity', address: ADDR, active: false }
    ])
  })

  it('quiets whoever was talking when the stream reopens', async () => {
    const { sent, voice, reopen } = engine()
    voice({ sender_address: ADDR, channel: 'voice-chat', active: true })
    await reopen()
    expect(sent[1]).toEqual({ kind: 'voiceActivity', address: ADDR, active: false })
  })
})
