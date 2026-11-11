import { beforeEach, describe, it, expect, vi } from 'vitest'

type VoiceItem = { sender_address: string; channel: string; active: boolean }
const stream = vi.hoisted(() => ({ onItem: (_: { sender_address: string; channel: string; active: boolean }) => undefined as void }))
vi.mock('../../bridge-scene/src/system-helpers', () => ({
  relay: (_name: string, _open: unknown, onItem: (item: VoiceItem) => void) => {
    stream.onItem = onItem
  }
}))
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: { getVoiceStream: vi.fn() } }))

const { isSpeaking, registerVoice, setNearbyVoiceBlocked } = await import('../../bridge-scene/src/domains/voice')

const NEARBY = '0x00000000000000000000000000000000000000aa'
const CALLER = '0x00000000000000000000000000000000000000bb'

type Sent = { kind: string; address: string; active: boolean }
const sent: Sent[] = []
registerVoice({ send: (msg) => sent.push(msg as never), on: () => undefined, push: () => undefined })

const say = (sender_address: string, channel: string, active: boolean): void => stream.onItem({ sender_address, channel, active })

describe('speaking while the scene blocks nearby voice', () => {
  beforeEach(() => {
    setNearbyVoiceBlocked(false)
    say(NEARBY, 'Nearby', false)
    say(CALLER, 'voice-chat', false)
    sent.length = 0
  })

  it('shows a nearby speaker where the scene allows nearby voice', () => {
    say(NEARBY, 'Nearby', true)
    expect(isSpeaking(NEARBY)).toBe(true)
    expect(sent).toEqual([{ kind: 'voiceActivity', address: NEARBY, active: true }])
  })

  it('hides a nearby speaker who starts talking in a blocked scene', () => {
    setNearbyVoiceBlocked(true)
    say(NEARBY, 'Nearby', true)
    expect(isSpeaking(NEARBY)).toBe(false)
    expect(sent).toEqual([])
  })

  it('keeps showing a call speaker in a blocked scene', () => {
    setNearbyVoiceBlocked(true)
    say(CALLER, 'voice-chat', true)
    expect(isSpeaking(CALLER)).toBe(true)
    expect(sent).toEqual([{ kind: 'voiceActivity', address: CALLER, active: true }])
  })

  it('hides nearby speakers on entering a blocked scene and shows them again on leaving', () => {
    say(NEARBY, 'Nearby', true)
    say(CALLER, 'voice-chat', true)
    sent.length = 0
    setNearbyVoiceBlocked(true)
    expect(sent).toEqual([{ kind: 'voiceActivity', address: NEARBY, active: false }])
    setNearbyVoiceBlocked(false)
    expect(sent.slice(1)).toEqual([{ kind: 'voiceActivity', address: NEARBY, active: true }])
  })
})
