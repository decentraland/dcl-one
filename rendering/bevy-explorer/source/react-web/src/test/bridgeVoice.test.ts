import { beforeEach, describe, it, expect, vi } from 'vitest'

type VoiceItem = { sender_address: string; channel: string; active: boolean }
const stream = vi.hoisted(() => ({
  onItem: (_: { sender_address: string; channel: string; active: boolean }) => undefined as void,
  open: async () => undefined as unknown
}))
vi.mock('../../bridge-scene/src/system-helpers', () => ({
  relay: (_name: string, open: () => Promise<unknown>, onItem: (item: VoiceItem) => void) => {
    stream.open = open
    stream.onItem = onItem
  }
}))
vi.mock('../../bridge-scene/src/bevy-api', () => ({ BevyApi: { getVoiceStream: vi.fn() } }))

const { isSpeaking, isSpeakingInCall, registerVoice, setNearbyVoiceBlocked } = await import('../../bridge-scene/src/domains/voice')

const NEARBY = '0x00000000000000000000000000000000000000aa'
const CALLER = '0x00000000000000000000000000000000000000bb'

type Sent = { kind: string; address: string; active: boolean }
const sent: Sent[] = []
registerVoice({ send: (msg) => sent.push(msg as never), on: () => undefined, push: () => undefined })

const say = (sender_address: string, channel: string, active: boolean): void => stream.onItem({ sender_address, channel, active })

describe('speaking while the scene blocks nearby voice', () => {
  beforeEach(async () => {
    setNearbyVoiceBlocked(false)
    await stream.open()
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
    expect(isSpeaking(NEARBY)).toBe(false)
    expect(isSpeaking(CALLER)).toBe(true)
    expect(sent).toEqual([{ kind: 'voiceActivity', address: NEARBY, active: false }])
    setNearbyVoiceBlocked(false)
    expect(isSpeaking(NEARBY)).toBe(true)
    expect(sent.slice(1)).toEqual([{ kind: 'voiceActivity', address: NEARBY, active: true }])
  })

  it('keeps a call speaker shown when their nearby room reports them silent in a blocked scene', () => {
    say(CALLER, 'Nearby', true)
    setNearbyVoiceBlocked(true)
    sent.length = 0
    say(CALLER, 'voice-chat', true)
    say(CALLER, 'Nearby', false)
    expect(isSpeaking(CALLER)).toBe(true)
    expect(sent).toEqual([{ kind: 'voiceActivity', address: CALLER, active: true }])
    say(CALLER, 'voice-chat', false)
    expect(isSpeaking(CALLER)).toBe(false)
    expect(sent.slice(1)).toEqual([{ kind: 'voiceActivity', address: CALLER, active: false }])
  })

  it('keeps a speaker shown while either room still reports them talking', () => {
    say(CALLER, 'Nearby', true)
    say(CALLER, 'voice-chat', true)
    say(CALLER, 'Nearby', false)
    expect(isSpeaking(CALLER)).toBe(true)
    expect(sent).toEqual([{ kind: 'voiceActivity', address: CALLER, active: true }])
  })

  it('reports the shown speakers stopped when the stream reopens in a blocked scene', async () => {
    say(NEARBY, 'Nearby', true)
    say(CALLER, 'voice-chat', true)
    setNearbyVoiceBlocked(true)
    sent.length = 0
    await stream.open()
    expect(sent).toEqual([{ kind: 'voiceActivity', address: CALLER, active: false }])
    expect(isSpeaking(CALLER)).toBe(false)
    setNearbyVoiceBlocked(false)
    expect(isSpeaking(NEARBY)).toBe(false)
    expect(sent).toHaveLength(1)
  })
})

describe('voice activity (bridge)', () => {
  beforeEach(async () => {
    setNearbyVoiceBlocked(false)
    await stream.open()
  })

  it('tells nearby speech from call speech, so a hushed caller keeps the speaking badge', () => {
    say(CALLER, 'voice-chat', true)
    expect(isSpeaking(CALLER)).toBe(true)
    expect(isSpeakingInCall(CALLER)).toBe(true)

    say(CALLER, 'voice-chat', false)
    expect(isSpeaking(CALLER)).toBe(false)
    expect(isSpeakingInCall(CALLER)).toBe(false)

    say(CALLER, 'Nearby', true)
    expect(isSpeaking(CALLER)).toBe(true)
    expect(isSpeakingInCall(CALLER)).toBe(false)
  })
})
