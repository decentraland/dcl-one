import { describe, it, expect, vi } from 'vitest'

const { relay } = vi.hoisted(() => ({ relay: vi.fn() }))
vi.mock('../../bridge-scene/src/system-helpers', () => ({ relay }))
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: { getVoiceStream: vi.fn() } }))

const { isSpeaking, isSpeakingInCall, registerVoice } = await import('../../bridge-scene/src/domains/voice')

type Voice = { sender_address: string; channel: string; active: boolean }

function engine(): (v: Voice) => void {
  relay.mockClear()
  registerVoice({ send: () => undefined, on: () => undefined, push: () => undefined })
  return relay.mock.calls[0][2] as (v: Voice) => void
}

const ADDR = '0x5854cce95d5e25817b41f4c41f06b695a83bc495'

describe('voice activity (bridge)', () => {
  it('tells nearby speech from call speech, so a hushed caller keeps the speaking badge', () => {
    const voice = engine()
    voice({ sender_address: ADDR, channel: 'voice-chat', active: true })
    expect(isSpeaking(ADDR)).toBe(true)
    expect(isSpeakingInCall(ADDR)).toBe(true)

    voice({ sender_address: ADDR, channel: 'voice-chat', active: false })
    expect(isSpeaking(ADDR)).toBe(false)
    expect(isSpeakingInCall(ADDR)).toBe(false)

    voice({ sender_address: ADDR, channel: 'Nearby', active: true })
    expect(isSpeaking(ADDR)).toBe(true)
    expect(isSpeakingInCall(ADDR)).toBe(false)
  })
})
