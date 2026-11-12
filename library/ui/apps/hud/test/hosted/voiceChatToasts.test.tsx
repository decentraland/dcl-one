import { describe, it, expect, vi, beforeEach } from 'vitest'
import { renderHook } from '@testing-library/react'
import type { CommunityVoiceUpdateMessage, VoiceChatState } from '@explorer-core/engine/protocol'

const { showToast, listeners } = vi.hoisted(() => ({
  showToast: vi.fn(),
  listeners: new Set<(e: CommunityVoiceUpdateMessage) => void>()
}))
vi.mock('../../design', () => ({ showToast }))
vi.mock('../../features/session/useEngineSession', () => ({
  onVoiceChatEvent: (listener: (e: CommunityVoiceUpdateMessage) => void) => {
    listeners.add(listener)
    return () => listeners.delete(listener)
  }
}))

const { useVoiceChatToasts } = await import('../../features/voiceChat/voiceChatToasts')

const idle: VoiceChatState = {
  status: 'disconnected',
  kind: 'none',
  callId: null,
  communityId: null,
  peer: null,
  connection: 'none',
  participants: [],
  micEnabled: false,
  localSpeaker: false,
  localRole: 'none',
  localRequestingToSpeak: false,
  error: null,
  errorIsNotice: false,
  errorSeq: 0
}

const started = (isMember: boolean): CommunityVoiceUpdateMessage => ({
  kind: 'communityVoiceUpdate',
  communityId: 'c1',
  status: 'started',
  createdAt: 0,
  endedAt: null,
  positions: [],
  worlds: [],
  isMember,
  communityName: 'Decentraland Foundation',
  communityImage: null
})

const emit = (e: CommunityVoiceUpdateMessage): void => {
  for (const listener of listeners) listener(e)
}

describe('voice chat toasts', () => {
  beforeEach(() => {
    showToast.mockReset()
    listeners.clear()
  })

  it("announces a stream's end to members only, and leaves its start to the notification", () => {
    renderHook(() => useVoiceChatToasts(idle))
    emit(started(true))
    expect(showToast).not.toHaveBeenCalled()
    emit({ ...started(false), status: 'ended' })
    expect(showToast).not.toHaveBeenCalled()
    emit({ ...started(true), status: 'ended' })
    expect(showToast).toHaveBeenCalledTimes(1)
  })

  it('shows how a call ended as a plain toast and a lost connection as an error', () => {
    const { rerender } = renderHook(({ state }) => useVoiceChatToasts(state), { initialProps: { state: idle } })
    rerender({ state: { ...idle, error: 'The stream ended.', errorIsNotice: true, errorSeq: 1 } })
    expect(showToast).toHaveBeenLastCalledWith('The stream ended.', { tone: 'default' })
    rerender({ state: { ...idle, error: "The call's connection was lost.", errorSeq: 2 } })
    expect(showToast).toHaveBeenLastCalledWith("The call's connection was lost.", { tone: 'error' })
  })

  it('shows the same error twice, and not an error from before it mounted', () => {
    const { rerender } = renderHook(({ state }) => useVoiceChatToasts(state), {
      initialProps: { state: { ...idle, kind: 'community', status: 'inCall', error: 'old', errorSeq: 3 } as VoiceChatState }
    })
    expect(showToast).not.toHaveBeenCalled()

    const inCall = { ...idle, kind: 'community', status: 'inCall' } as const
    rerender({ state: { ...inCall, error: 'not a moderator', errorSeq: 4 } })
    rerender({ state: { ...inCall, error: 'not a moderator', errorSeq: 4, micEnabled: true } })
    rerender({ state: { ...inCall, error: 'not a moderator', errorSeq: 5 } })
    expect(showToast).toHaveBeenCalledTimes(2)
    expect(showToast).toHaveBeenLastCalledWith('not a moderator', { tone: 'error' })
  })
})
